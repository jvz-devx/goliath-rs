#!/usr/bin/env python3
"""E9 rerun of E5 crash, fence, torn-tail and barrier checks, stdlib only."""

import argparse
import concurrent.futures
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import selectors
import shutil
import socket
import struct
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parent.parent
RUNS = ROOT / ".local/runs"
U64 = struct.Struct("<Q")
U32 = struct.Struct("<I")
ID_LOCK = threading.Lock()
NEXT_ID = random.SystemRandom().randrange(1, 1 << 96)
PEAK_RUN_BYTES = 0


def request(operation, payload=b"", ident=None):
    global NEXT_ID
    if ident is None:
        with ID_LOCK:
            NEXT_ID += 1
            ident = NEXT_ID
    return b"\1" + ident.to_bytes(16, "little") + bytes([operation]) + payload


def u64(value):
    return U64.pack(value)


def blob(value):
    return U32.pack(1) + U32.pack(len(value)) + value


def contents(player, generation, length=256):
    head = struct.pack("<QQ", player, generation)
    return (head + hashlib.sha256(head).digest() * ((length + 31) // 32))[:length]


def commit(player, generation, source, target, length=256):
    return request(5, u64(player) + u64(generation) + source + target + blob(contents(player, generation + 1, length)))


def exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count - len(data))
        if not part:
            raise ConnectionError("EOF")
        data.extend(part)
    return bytes(data)


def receive(sock):
    count = U32.unpack(exact(sock, 4))[0]
    assert count <= 1024 * 1024, count
    return exact(sock, count)


def call(address, message):
    with socket.create_connection(address, timeout=5) as sock:
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        sock.sendall(U32.pack(len(message)) + message)
        return receive(sock)


def player_record(address, player):
    data = call(address, request(3, u64(player)))
    assert data[:3] == b"\1\3\1", data
    generation, owner, version, size = struct.unpack_from("<QQII", data, 3)
    payload = data[27:]
    assert version == 1 and len(payload) == size, data
    assert payload == contents(player, generation, size), (player, generation, payload.hex())
    assert owner == generation % 2, (player, generation, owner)
    return generation, owner, payload


def directory(address):
    data = call(address, request(2))
    assert data[:2] == b"\1\2", data
    count = U32.unpack_from(data, 2)[0]
    assert len(data) == 6 + count * 33
    entries = []
    for i in range(count):
        tile, process, epoch, active, heartbeat = struct.unpack_from("<QQQBQ", data, 6 + i * 33)
        entries.append((tile, process, epoch, active, heartbeat))
    assert len({entry[0] for entry in entries}) == count, entries
    assert len({entry[1] for entry in entries}) == count, entries
    return entries


def claim(address, tile):
    data = call(address, request(1, u64(tile)))
    assert data[:2] == b"\1\1" and len(data) == 26, data
    return data[2:]


def created(address, player, source, length=256):
    data = call(address, request(4, u64(player) + source + blob(contents(player, 0, length))))
    assert data == b"\1\4" + u64(0), data


def disk_bytes(path):
    total = 0
    for item in path.rglob("*"):
        try:
            if item.is_file():
                total += item.stat().st_blocks * 512
        except FileNotFoundError:
            pass
    return total


def measure_disk(path):
    global PEAK_RUN_BYTES
    PEAK_RUN_BYTES = max(PEAK_RUN_BYTES, disk_bytes(path))


class Server:
    def __init__(self, binary, path, mode="full", compact=64, extra=()):
        self.path = path
        self.process = subprocess.Popen(
            [str(binary), "--dir", str(path), "--sync", mode, "--compact-every", str(compact), *extra],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(self.process.stdout, selectors.EVENT_READ)
                assert selector.select(10), "store startup timed out"
            line = self.process.stdout.readline().strip()
            assert line.startswith("LISTEN "), (line, self.process.poll())
            host, port = line[7:].rsplit(":", 1)
            self.address = (host, int(port))
        except BaseException:
            self.kill()
            raise

    def kill(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdout.close()


def kill_run(binary, root, count, seed, clients):
    """The observer stays alive while only the store receives SIGKILL."""
    path = root / f"kill-{seed}"
    rng = random.Random(seed)
    server = Server(binary, path)
    fences = [claim(server.address, tile) for tile in range(2)]
    for player in range(clients):
        created(server.address, player, fences[0])
    acknowledged = [0] * clients
    observed = [0] * clients
    last_request = [None] * clients
    acknowledgements = 0
    uncertain = 0
    applied_uncertain = 0
    digest = hashlib.sha256()
    lock = threading.Lock()
    snapshots = set()
    errors = []
    try:
        for cut in range(count):
            for player in range(clients):
                generation, _, _ = player_record(server.address, player)
                assert generation >= acknowledged[player], ("lost acknowledgement", seed, cut, player, generation, acknowledged[player])
                assert generation >= observed[player], ("generation reversed", seed, cut, player, generation, observed[player])
                if last_request[player] is not None:
                    message, expected = last_request[player]
                    # Retry precisely the same command, even if it committed
                    # before SIGKILL but its response never reached the observer.
                    response = call(server.address, message)
                    assert response == b"\1\4" + u64(expected), response
                    applied_uncertain += int(generation < expected)
                    generation, _, _ = player_record(server.address, player)
                    assert generation >= expected
                    acknowledged[player] = max(acknowledged[player], expected)
                observed[player] = generation
            entries = directory(server.address)
            assert [tuple(struct.unpack("<QQQ", f)) for f in fences] == [e[:3] for e in entries]
            assert all(e[3] == 1 for e in entries)
            stop = threading.Event()
            barrier = threading.Barrier(clients + 1)

            def worker(player):
                nonlocal acknowledgements, uncertain
                try:
                    generation = observed[player]
                    barrier.wait(timeout=10)
                    while not stop.is_set():
                        owner = generation % 2
                        message = commit(player, generation, fences[owner], fences[1 - owner])
                        with lock:
                            last_request[player] = (message, generation + 1)
                        try:
                            response = call(server.address, message)
                        except OSError:
                            with lock:
                                uncertain += 1
                            return
                        assert response == b"\1\4" + u64(generation + 1), response
                        generation += 1
                        with lock:
                            acknowledged[player] = generation
                            observed[player] = generation
                            acknowledgements += 1
                            digest.update(struct.pack("<QQQ", cut, player, generation))
                except BaseException as error:
                    with lock:
                        errors.append(repr(error))

            with concurrent.futures.ThreadPoolExecutor(max_workers=clients) as pool:
                futures = [pool.submit(worker, player) for player in range(clients)]
                barrier.wait(timeout=10)
                # Deterministic seeded schedule, nondeterministic OS/IO overlap.
                time.sleep(rng.uniform(0.0001, 0.025))
                server.kill()
                stop.set()
                for future in futures:
                    future.result(timeout=10)
            assert server.process.returncode == -9, server.process.returncode
            assert not errors, errors
            snapshot = path / "snapshot"
            if snapshot.exists():
                snapshots.add(hashlib.sha256(snapshot.read_bytes()).hexdigest())
            measure_disk(path)
            server = Server(binary, path)
            if (cut + 1) % 100 == 0:
                print(f"kills seed={seed}: {cut + 1}/{count}, load acknowledgements={acknowledgements}", flush=True)
        for player in range(clients):
            generation, _, _ = player_record(server.address, player)
            assert generation >= max(acknowledged[player], observed[player])
        return dict(seed=seed, kills=count, clients=clients, load_acknowledgements=acknowledgements,
                    transport_unknown=uncertain, retries_completed_missing=applied_uncertain,
                    distinct_snapshots=len(snapshots), lost_acknowledgements=0,
                    backwards_generations=0, owner_violations=0, observation_sha256=digest.hexdigest())
    finally:
        server.kill()
        shutil.rmtree(path)


def fence_run(binary, root, disabled=False):
    path = root / ("fence-control" if disabled else "fence")
    server = Server(binary, path, compact=0, extra=("--fault-disable-fence",) if disabled else ())
    rejected = 0
    accepted_stale = 0
    try:
        fences = [claim(server.address, tile) for tile in range(2)]
        for case in range(32):
            created(server.address, case, fences[0], 32)
            message = commit(case, 0, fences[0], fences[1], 32)
            # Put the length and a partial body into a real connection before
            # replacement. Deliver its remaining bytes only after replacement.
            with socket.create_connection(server.address, timeout=5) as held:
                held.sendall(U32.pack(len(message)) + message[:17])
                tile = case % 2
                old = fences[tile]
                fences[tile] = claim(server.address, tile)
                assert struct.unpack("<QQQ", fences[tile])[2] > struct.unpack("<QQQ", old)[2]
                # Deliver a later fresh heartbeat first, then the delayed CAS.
                assert call(server.address, request(7, fences[tile] + u64(case))) == b"\1\0"
                held.sendall(message[17:])
                response = receive(held)
            if response == b"\1\5\1":
                rejected += 1
                assert call(server.address, message) == response
                assert player_record(server.address, case)[0] == 0
            elif response == b"\1\4" + u64(1):
                accepted_stale += 1
            else:
                raise AssertionError(response)
            # Non-CAS mutations are fenced too.
            for operation in [request(6, old), request(7, old + u64(99)), request(4, u64(1000 + case) + old + blob(b"x"))]:
                assert call(server.address, operation) == b"\1\5\1"
        checker_failure = None
        try:
            # Exactly the same safety assertion is run in both fence modes.
            assert rejected == 32 and accepted_stale == 0, f"stale CAS accepted: {accepted_stale}/32"
        except AssertionError as error:
            if not disabled:
                raise
            checker_failure = str(error)
        if disabled:
            assert checker_failure is not None, "fence mutation escaped the safety checker"
            assert accepted_stale == 32, accepted_stale
        return dict(delayed_cas=32, rejected=rejected, accepted_stale=accepted_stale,
                    checker_failure=checker_failure,
                    safety_check="FAIL as required" if accepted_stale else "PASS")
    finally:
        server.kill()
        shutil.rmtree(path)


def tail_run(binary, root):
    fixture = root / "tail-fixture"
    server = Server(binary, fixture, compact=0)
    try:
        fences = [claim(server.address, tile) for tile in range(2)]
        created(server.address, 0, fences[0], 32)
        assert call(server.address, request(8)) == b"\1\0"
        snapshot = (fixture / "snapshot").read_bytes()
        message = commit(0, 0, fences[0], fences[1], 32)
        assert call(server.address, message) == b"\1\4" + u64(1)
    finally:
        server.kill()
    last = (fixture / "journal").read_bytes()
    assert last[:8] == b"GSTLOG01"
    trials = []
    try:
        for kind in ("truncate", "corrupt"):
            for offset in range(len(last)):
                path = root / "tail-case"
                path.mkdir()
                (path / "snapshot").write_bytes(snapshot)
                damaged = last[:offset] if kind == "truncate" else last[:offset] + bytes([last[offset] ^ 1]) + last[offset + 1:]
                (path / "journal").write_bytes(damaged)
                recovered = Server(binary, path, compact=0)
                try:
                    assert player_record(recovered.address, 0)[0] == 0, (kind, offset)
                    assert (path / "journal").stat().st_size == 0, (kind, offset)
                    assert call(recovered.address, message) == b"\1\4" + u64(1)
                finally:
                    recovered.kill()
                recovered = Server(binary, path, compact=0)
                try:
                    assert player_record(recovered.address, 0)[0] == 1
                    assert call(recovered.address, message) == b"\1\4" + u64(1)
                finally:
                    recovered.kill()
                trials.append((kind, offset, "PASS"))
                shutil.rmtree(path)
        # A complete valid final record must not be mistaken for a torn one.
        recovered = Server(binary, fixture, compact=0)
        try:
            assert player_record(recovered.address, 0)[0] == 1
        finally:
            recovered.kill()
        return dict(record_bytes=len(last), truncations=len(last), corruptions=len(last),
                    restarts=4 * len(last) + 1, failures=0), trials
    finally:
        shutil.rmtree(fixture)


def latency_run(binary, root, mode, clients, samples, rows, driver=None):
    load_before = os.getloadavg()
    path = root / f"latency-{mode}-{clients}"
    server = Server(binary, path, mode=mode, compact=256)
    try:
        fences = [claim(server.address, tile) for tile in range(2)]
        for player in range(clients):
            created(server.address, player, fences[0])
        barrier = threading.Barrier(clients)

        def worker(player):
            measured = []
            # Warm one full round trip per client before the timed set.
            message = commit(player, 0, fences[0], fences[1])
            assert call(server.address, message) == b"\1\4" + u64(1)
            barrier.wait(timeout=30)
            for index in range(samples):
                generation = index + 1
                source = generation % 2
                message = commit(player, generation, fences[source], fences[1 - source])
                begin = time.perf_counter_ns()
                response = call(server.address, message)
                elapsed = (time.perf_counter_ns() - begin) / 1_000_000
                assert response == b"\1\4" + u64(generation + 1), response
                measured.append(elapsed)
            return measured

        begin = time.perf_counter()
        if driver is None:
            with concurrent.futures.ThreadPoolExecutor(max_workers=clients) as pool:
                values = list(pool.map(worker, range(clients)))
        else:
            output = subprocess.run([str(driver), ":".join(map(str, server.address)), str(clients), str(samples)],
                                    check=True, capture_output=True, text=True, timeout=60)
            values = [[] for _ in range(clients)]
            for row in csv.DictReader(output.stdout.splitlines()):
                player = int(row["player"])
                assert int(row["index"]) == len(values[player]), row
                values[player].append(float(row["milliseconds"]))
            assert all(len(group) == samples for group in values)
        elapsed = time.perf_counter() - begin
        flat = sorted(value for group in values for value in group)
        for player, group in enumerate(values):
            rows.extend((mode, clients, player, index, round(value, 6)) for index, value in enumerate(group))
        measure_disk(path)
        percentile = lambda p: round(flat[math.ceil(p * len(flat)) - 1], 6)
        result = dict(mode=mode, clients=clients, samples=len(flat), blob_bytes=256,
                      driver="python" if driver is None else "rust",
                      p50_ms=percentile(.50), p95_ms=percentile(.95), p99_ms=percentile(.99),
                      max_ms=round(flat[-1], 6), wall_seconds=round(elapsed, 3))
        result.update(load_average_before=load_before, load_average_after=os.getloadavg())
        print(f"latency {result}", flush=True)
        return result
    finally:
        server.kill()
        shutil.rmtree(path)


def write_csv(path, header, rows):
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(header)
        writer.writerows(rows)


def barrier_control(binary, root, early=False):
    path = root / ("early-ack" if early else "barrier")
    server = Server(binary, path, compact=0,
                    extra=("--test-power-loss-layer", "--fault-ack-before-barrier") if early
                    else ("--test-power-loss-layer",))
    try:
        fences = [claim(server.address, tile) for tile in range(2)]
        created(server.address, 0, fences[0])
        assert call(server.address, commit(0, 0, *fences)) == b"\1\4" + u64(1)
        acknowledged = 1
        assert player_record(server.address, 0)[0] == acknowledged
    finally:
        server.kill()
    server = Server(binary, path, compact=0)
    try:
        recovered = player_record(server.address, 0)[0]
        failure = None
        try:
            # The same observer assertion as the kill experiment. The injected
            # file layer drops unflushed bytes; SIGKILL alone would not do that.
            assert recovered >= acknowledged, "lost acknowledgement after unflushed cache loss"
        except AssertionError as error:
            if not early:
                raise
            failure = str(error)
        assert (failure is not None) == early, "early-ack mutation escaped checker"
        return dict(acknowledged=acknowledged, recovered=recovered, checker_failure=failure,
                    file_layer="volatile writes dropped before barrier" if early
                    else "same volatile file cache, persisted by real barrier")
    finally:
        server.kill()
        shutil.rmtree(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/goliath-store")
    parser.add_argument("--kills", type=int, default=1000)
    parser.add_argument("--latency-driver", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "docs/experiments/data/E9")
    parser.add_argument("--only", choices=["all", "kills", "fence", "tail", "latency", "barrier"], default="all")
    args = parser.parse_args()
    assert args.kills >= 0
    RUNS.mkdir(parents=True, exist_ok=True)
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="e5-", dir=RUNS))
    begin = time.perf_counter()
    sampling_done = threading.Event()

    def sample_disk():
        while not sampling_done.wait(.01):
            measure_disk(root)

    sampler = threading.Thread(target=sample_disk)
    sampler.start()
    result = dict(platform=platform.platform(), machine=platform.machine(),
                  binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  durability="F_FULLFSYNC" if platform.system() == "Darwin" else "sync_all",
                  command=" ".join(os.sys.argv).replace(str(ROOT), "$REPO").replace(str(Path.home()), "$HOME"),
                  load_average_before=os.getloadavg())
    try:
        if args.only in ("all", "fence"):
            result["fence"] = fence_run(args.binary, root)
            result["fence_control"] = fence_run(args.binary, root, disabled=True)
            print(f"fence {result['fence']}, control {result['fence_control']}", flush=True)
        if args.only in ("all", "tail"):
            result["tail"], trials = tail_run(args.binary, root)
            write_csv(args.output / "tail.csv", ["mutation", "offset", "result"], trials)
            print(f"tail {result['tail']}", flush=True)
        if args.only in ("all", "barrier"):
            result["barrier"] = barrier_control(args.binary, root)
            result["early_ack_control"] = barrier_control(args.binary, root, early=True)
        if args.only in ("all", "kills"):
            first = args.kills // 2
            result["kills"] = [kill_run(args.binary, root, first, 5001, 8),
                               kill_run(args.binary, root, args.kills - first, 5002, 8)]
        if args.only in ("all", "latency"):
            rows = []
            result["latency"] = [latency_run(args.binary, root, mode, clients, samples, rows, args.latency_driver)
                                 for mode in ("fsync", "full")
                                 for clients, samples in ((1, 256), (8, 64), (64, 16), (256, 8))]
            write_csv(args.output / "latency.csv", ["mode", "clients", "player", "index", "milliseconds"], rows)
        result["wall_seconds"] = round(time.perf_counter() - begin, 3)
        result["peak_sampled_run_allocated_bytes"] = PEAK_RUN_BYTES
        result["target_allocated_bytes_at_end"] = disk_bytes(ROOT / "target")
        result["load_average_after"] = os.getloadavg()
        (args.output / f"{args.only}-results.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2), flush=True)
    finally:
        sampling_done.set()
        sampler.join()
        shutil.rmtree(root)


if __name__ == "__main__":
    main()
