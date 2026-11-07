#!/usr/bin/env python3
"""Reproduce E16 store takeover timing and parent-authorized E15 quiet checks."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("e5", ROOT / "scripts/e5-store.py")
e5 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e5)


def claim(server, tile, length):
    reply = e5.call(server.address, e5.request(15, e5.u64(tile) + e5.u64(length)))
    assert reply[:2] == b"\1\10" and len(reply) == 42, reply
    return reply[2:26]


def takeover(args):
    rows = []
    for length in (500, 1000, 2000):
        for restart in (False, True):
            for trial in range(args.trials):
                folder = Path(tempfile.mkdtemp(prefix="e16-takeover-", dir=ROOT / ".local/runs"))
                server = e5.Server(args.binary, folder, compact=4)
                try:
                    fence = claim(server, 0, length)
                    sent = time.monotonic()
                    renew = e5.request(16, fence + e5.u64(1))
                    reply = e5.call(server.address, renew)
                    assert reply[:2] == b"\1\10", reply
                    received = time.monotonic()
                    # A failure just after renewal has the longest wait. Other
                    # trials fail one heartbeat interval later.
                    phase = 0 if trial % 2 == 0 else length / 4
                    time.sleep(phase / 1000)
                    failed = time.monotonic()
                    assert e5.call(server.address, e5.request(17, fence)) == b"\1\0"
                    recovered = None
                    if restart:
                        server.kill()
                        server = e5.Server(args.binary, folder, compact=4)
                        recovered = time.monotonic()
                    busy = 0
                    while True:
                        reply = e5.call(server.address, e5.request(15, e5.u64(0) + e5.u64(length)))
                        if reply[:2] == b"\1\10":
                            break
                        assert reply == b"\1\5\7", reply
                        busy += 1
                        time.sleep(.005)
                        assert time.monotonic() - failed < length / 1000 + 5
                    completed = time.monotonic()
                    assert completed - sent >= length / 1000
                    if recovered is not None:
                        # Recovery has finished before LISTEN. It can finish a
                        # few milliseconds before the parent observes that line.
                        assert completed - recovered >= length / 1000 - .020
                    rows.append(dict(lease_ms=length, restart=restart, trial=trial,
                        failure_phase_ms=phase, busy_claims=busy,
                        renewal_rtt_ms=round((received - sent) * 1000, 3),
                        wait_from_failure_ms=round((completed - failed) * 1000, 3),
                        wait_from_renew_send_ms=round((completed - sent) * 1000, 3),
                        restart_wait_ms=None if recovered is None else round((completed - recovered) * 1000, 3)))
                finally:
                    server.kill()
                    shutil.rmtree(folder)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "takeover.json").write_text(json.dumps(dict(
        binary_sha256=e5.hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        durability="F_FULLFSYNC" if e5.platform.system() == "Darwin" else "sync_all",
        trials=rows), indent=2) + "\n")
    for length in (500, 1000, 2000):
        values = [row["wait_from_failure_ms"] for row in rows if row["lease_ms"] == length and not row["restart"]]
        print(f"L={length} ms wait from failure {min(values):.3f}..{max(values):.3f} ms", flush=True)


def quiet_latency(args):
    # This command is authorized by the parent for E16. Do not use it for
    # unrelated work or create a second overlapping request.
    files = [ROOT / ".local/quiet-request", *(ROOT / f".local/quiet-ack-{agent}" for agent in args.quiet_agents)]
    if any(path.exists() for path in files):
        raise RuntimeError("quiet markers already exist; coordinate before retrying")
    started = datetime.now(timezone.utc).isoformat()
    with files[0].open("x") as file:
        file.write(started + f"\nE16 store lease latency, waiting for {','.join(args.quiet_agents)}, at most 30 minutes.\n")
    e5.QUIET_FILES = files
    e5.QUIET_ACK_DEADLINE = time.monotonic() + 1800
    e5.QUIET_DEADLINE = e5.QUIET_ACK_DEADLINE
    outcomes = []
    original_claim = e5.claim
    original_latency = e5.latency_run

    def leased_latency(*positional, **keywords):
        done = threading.Event()
        workers = []
        errors = []

        def leased_claim(address, tile):
            reply = e5.call(address, e5.request(15, e5.u64(tile) + e5.u64(1000)))
            assert reply[:2] == b"\1\10" and len(reply) == 42, reply
            fence = reply[2:26]

            def renew():
                counter = 0
                while not done.wait(.250):
                    counter += 1
                    try:
                        reply = e5.call(address, e5.request(16, fence + e5.u64(counter)))
                        assert reply[:2] == b"\1\10", reply
                    except BaseException as error:
                        errors.append(str(error))
                        return

            worker = threading.Thread(target=renew)
            workers.append(worker)
            worker.start()
            return fence

        e5.claim = leased_claim
        try:
            result = original_latency(*positional, **keywords)
            if errors:
                raise RuntimeError(f"lease renewal failed: {errors}")
            result.update(lease_ms=1000, renew_interval_ms=250)
            return result
        finally:
            done.set()
            for worker in workers:
                worker.join()
            e5.claim = original_claim

    try:
        for leased in (False, True):
            e5.latency_run = leased_latency if leased else original_latency
            for trial in range(args.trials):
                output = args.output / f"{'leased' if leased else 'legacy'}-{trial + 1}"
                sys.argv = ["scripts/e5-store.py", "--only", "latency", "--latency-modes", "full",
                    "--binary", str(args.binary), "--latency-driver", str(args.driver),
                    "--latency-clients", "64", "--latency-samples", "128", "--output", str(output)]
                e5.main()
                result = json.loads((output / "latency-results.json").read_text())
                row = result["latency"][0]
                row.update(trial=trial + 1, leased=leased)
                outcomes.append(row)
                if row["disturbed"]:
                    raise RuntimeError("quiet latency row disturbed; retain it and coordinate a fresh window")
                if row["p95_ms"] >= 20:
                    raise RuntimeError("64-client commit P95 missed the 20 ms limit")
        summary = dict(started=started, finished=datetime.now(timezone.utc).isoformat(), rows=outcomes)
        (args.output / "quiet-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    finally:
        e5.claim = original_claim
        e5.latency_run = original_latency
        for file in files:
            file.unlink(missing_ok=True)
        print("E16 quiet request and required acknowledgements released", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("takeover", "quiet-latency"))
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/goliath-store")
    parser.add_argument("--driver", type=Path, default=ROOT / "target/release/goliath-store-load")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/experiments/data/E16")
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--quiet-agents", nargs="+", choices=("E12", "E14"), default=["E12", "E14"])
    args = parser.parse_args()
    assert args.trials > 0
    (ROOT / ".local/runs").mkdir(parents=True, exist_ok=True)
    args.output.mkdir(parents=True, exist_ok=True)
    (takeover if args.mode == "takeover" else quiet_latency)(args)


if __name__ == "__main__":
    main()
