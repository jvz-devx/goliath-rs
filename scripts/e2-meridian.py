#!/usr/bin/env python3
"""E2 real-server trials. Loaded by e2-real-transfer after its prerequisite check.

Pins: Horizon fc2868b2b26d56ec76e17414909fe58a3b87bb92;
Meridian 902dcf195e0a8bbd3f39a5f560684af00e64bf5d.
Requires patches/e2-horizon-input-trace.patch and patches/e2-meridian-client.patch.
"""
import base64
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import re
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import goliath_platform as platform


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Child:
    def __init__(self, command, cwd, log):
        self.log = log.open("w")
        self.process = subprocess.Popen(
            platform.launch_command([str(part) for part in command]), cwd=cwd, env=os.environ,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
            text=True, bufsize=1, start_new_session=True,
        )
        self.events = []
        self.incoming = queue.Queue()
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()

    def read(self):
        for line in self.process.stdout:
            self.log.write(line)
            self.log.flush()
            try:
                event = json.loads(line)
            except ValueError:
                continue
            self.events.append(event)
            self.incoming.put(event)

    def send(self, text):
        self.process.stdin.write(text + "\n")
        self.process.stdin.flush()

    def wait(self, predicate, timeout=25):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f"child exited {self.process.returncode}: {self.process.args[0]}")
            try:
                event = self.incoming.get(timeout=0.1)
            except queue.Empty:
                continue
            if predicate(event):
                return event
        raise RuntimeError("client event timed out")

    def stop(self):
        try:
            os.killpg(self.process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            self.process.wait(10)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait()
        self.reader.join(5)
        self.log.close()


def inputs(engine, after=0):
    readings = []
    for line in engine.server.since(0):
        if "E2_INPUT " in line:
            value = json.loads(line.split("E2_INPUT ", 1)[1])
            if value["ns"] > after:
                readings.append(value)
    return readings


def wait_input(engine, after, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        values = inputs(engine, after)
        if values:
            return values[0]
        time.sleep(0.01)
    raise RuntimeError("no server-accepted input on target")


def inventory(engine, uuid, upstream):
    reply = engine.ask(f"player-export {uuid}")
    if not reply.get("ok"):
        raise RuntimeError("inventory export was refused: " + str(reply.get("error")))
    record = reply["record"]
    player = upstream.read_nbt(base64.b64decode(record["player"]))
    stacks = player.get("Inventory", [])
    checksum = hashlib.sha256(json.dumps(stacks, sort_keys=True).encode()).hexdigest()
    return {"sha256": checksum, "count": sum(stack.get("count", 1) for stack in stacks),
            "velocity": record["velocity"], "position": record["position"]}


def percentiles(values):
    ordered = sorted(values)
    return {f"p{p}": ordered[max(0, math.ceil(len(ordered) * p / 100) - 1)]
            for p in (50, 95, 99)}


def run(root, resources, env_factory, stationary=50, walking=50):
    sys.path.insert(0, str(root / ".local/src/horizon/tools"))
    import server_transfer_e2e as upstream
    bins = root / ".local/bin/horizon-fc2868b2b"
    meridian = root / ".local/bin/meridian-902dcf1"
    output = root / "docs/experiments/data/E2"
    trials = []
    result = {"status": "running", "trials": trials, "settings": {
        "stationary": stationary, "walking": walking, "speed_blocks_per_second": 5.6,
        "view_distance": 3, "simulation_distance": 3, "level_type": "flat",
        "client": "kinematic absolute-position testkit client, not Minecraft physics",
        "gap": "first target accepted movement ns minus last source accepted movement ns",
        "total": "client command send to successful proxy chat receipt",
        "bytes": "compressed TCP payload bytes read by protocol connection, excludes TCP/IP headers",
    }}
    prior_env = dict(os.environ)
    with tempfile.TemporaryDirectory(prefix="E2-meridian-", dir=root / ".local/runs") as temporary:
        scratch = Path(temporary)
        os.environ.update(env_factory(scratch))
        os.environ["E2_TRACE_INPUTS"] = "1"
        engines, children = [], []
        try:
            # Use the upstream Engine's isolated startup and control socket path.
            for label in ("a", "b"):
                engine = upstream.Engine(bins, resources, scratch / label, scratch, label,
                                         **{"level-seed": "12345", "allow-flight": "true"})
                engines.append(engine)
                engine.ready()
            sideports = [free_port(), free_port()]
            sidekeys = [scratch / f"sidecar-{index}.key" for index in range(2)]
            for index, sidekey in enumerate(sidekeys):
                sidekey.write_text(f"e2-only-local-sidecar-secret-{index}-2026")
            for engine, port, sidekey in zip(engines, sideports, sidekeys):
                children.append(Child([
                    meridian / "meridian-sidecar", "--endpoint-file", engine.endpoint,
                    "--listen", f"127.0.0.1:{port}", "--secret-file", sidekey,
                ], scratch, scratch / f"sidecar-{port}.log"))
            proxyport = free_port()
            config = scratch / "meridian.toml"
            config.write_text(
                f'listen = "127.0.0.1:{proxyport}"\nauth-mode = "offline"\n'
                'connections-per-minute = 0\nmax-players = 2\n'
                f'forwarding-secret = "{upstream.SECRET.decode()}"\n'
                'switch-carry-position = true\ndefault-backend = "a"\n'
                + "".join(
                    f'[[backends]]\nname = "{engine.label}"\naddress = "127.0.0.1:{engine.port}"\n'
                    f'sidecar = "127.0.0.1:{port}"\nsidecar-secret-file = "{sidekey}"\n'
                    for engine, port, sidekey in zip(engines, sideports, sidekeys)
                )
            )
            subprocess.run([meridian / "meridian", "--check", config], cwd=scratch,
                           stdout=subprocess.DEVNULL, check=True)
            children.append(Child([meridian / "meridian", config], scratch, scratch / "proxy.log"))
            # Wait for TCP readiness without consuming a login or contacting any account service.
            deadline = time.monotonic() + 10
            while True:
                try:
                    with socket.create_connection(("127.0.0.1", proxyport), timeout=0.2):
                        break
                except OSError:
                    if time.monotonic() > deadline:
                        raise RuntimeError("proxy did not listen")
                    time.sleep(0.05)
            client = Child([meridian / "e2-client", proxyport], scratch, scratch / "client.log")
            children.append(client)
            client.wait(lambda event: event.get("kind") == "teleport")
            uuid = str(upstream.offline_uuid("E2Probe"))
            wait_input(engines[0], 0)
            for command in ("gamerule do_mob_spawning false", "gamerule do_weather_cycle false",
                            "gamerule random_tick_speed 0", "gamerule spawn_radius 0"):
                for engine in engines:
                    engine.server.console(command)
            engines[0].server.console("give E2Probe minecraft:diamond 5")
            engines[0].server.console("give E2Probe minecraft:oak_log 12")
            time.sleep(0.5)
            source = 0
            for index in range(stationary + walking):
                moving = index >= stationary
                client.send("speed 5.6" if moving else "speed 0")
                time.sleep(0.25)
                before = inventory(engines[source], uuid, upstream)
                if before["count"] != 17:
                    raise RuntimeError("known inventory is not 17 items before trial")
                start = time.time_ns()
                client.send(f"command server {engines[1-source].label}")
                sent = client.wait(lambda event: event.get("kind") == "command" and event["ns"] >= start)
                completed = client.wait(lambda event: event.get("kind") == "chat" and
                                        event.get("text") == f"You are now on {engines[1-source].label}.")
                first = wait_input(engines[1-source], sent["ns"])
                source_inputs = [value for value in inputs(engines[source]) if value["ns"] < first["ns"]]
                if not source_inputs:
                    raise RuntimeError("no last accepted source movement")
                last = source_inputs[-1]
                after = inventory(engines[1-source], uuid, upstream)
                teleports = [event for event in client.events if event.get("kind") == "teleport"
                             and sent["ns"] <= event["ns"] <= first["ns"] + 100_000_000]
                errors = [math.dist(event["before"], event["pos"]) for event in teleports]
                trial = {"index": index, "mode": "walking" if moving else "stationary",
                         "source": engines[source].label, "target": engines[1-source].label,
                         "total_ms": (completed["ns"] - sent["ns"]) / 1e6,
                         "input_gap_ms": (first["ns"] - last["ns"]) / 1e6,
                         "last_source_input": last, "first_target_input": first,
                         "client_bytes": completed["bytes"] - sent["bytes"],
                         "arrival_error_blocks": max(errors) if errors else None,
                         "arrival_teleports": [{key: value for key, value in event.items() if key != "kind"} for event in teleports],
                         "inventory_before": before["sha256"], "inventory_after": after["sha256"],
                         "inventory_count_before": before["count"], "inventory_count_after": after["count"],
                         "inventory_equal": before["sha256"] == after["sha256"],
                         "server_velocity_after": after["velocity"]}
                trials.append(trial)
                (output / "trials.json").write_text(json.dumps(result, indent=2) + "\n")
                print(f'E2 {index + 1}/{stationary+walking} {trial["mode"]}: '
                      f'total {trial["total_ms"]:.1f} ms, input gap {trial["input_gap_ms"]:.1f} ms', flush=True)
                if not trial["inventory_equal"]:
                    raise RuntimeError("inventory changed during transfer")
                source = 1 - source
            result["status"] = "complete"
            result["summary"] = {}
            for mode in ("stationary", "walking"):
                selected = [trial for trial in trials if trial["mode"] == mode]
                if selected:
                    result["summary"][mode] = {
                        metric: percentiles([trial[metric] for trial in selected])
                        for metric in ("total_ms", "input_gap_ms", "client_bytes", "arrival_error_blocks")
                    }
        except Exception as error:
            result["status"] = "failed"
            result["error"] = str(error)
            # Report an error live, but retain no game logs or player records.
            for log in sorted(scratch.glob("*.log")):
                print(f"E2 diagnostic {log.name}:\n" + "\n".join(log.read_text(errors="replace").splitlines()[-8:]), file=sys.stderr)
            raise
        finally:
            for child in reversed(children):
                child.stop()
            for engine in reversed(engines):
                engine.stop()
            result["run_disk_mib"] = int(subprocess.check_output(["du", "-sm", scratch], text=True).split()[0])
            (output / "trials.json").write_text(json.dumps(result, indent=2) + "\n")
            os.environ.clear()
            os.environ.update(prior_env)
    return result
