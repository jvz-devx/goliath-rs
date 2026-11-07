#!/usr/bin/env python3
"""Build the two E2 workspaces once each, retain binaries, and record time and disk.

Pins: Horizon fc2868b2b26d56ec76e17414909fe58a3b87bb92;
Meridian 902dcf195e0a8bbd3f39a5f560684af00e64bf5d.
No launcher is run and no Mojang files are downloaded or copied.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


PROJECTS = (
    ("horizon", "https://github.com/horizon-rust/horizon.git",
     "fc2868b2b26d56ec76e17414909fe58a3b87bb92", "e2-horizon-input-trace.patch",
     ("horizon-client", "horizon-bots", "horizon-client-lib", "horizon-server"),
     ("horizon-client", "horizon-bots", "horizon-ctl", "horizon-server")),
    ("meridian", "https://github.com/Akirabane/Meridian.git",
     "902dcf195e0a8bbd3f39a5f560684af00e64bf5d", "e2-meridian-client.patch",
     ("meridian-proxy", "meridian-sidecar", "meridian-bench", "meridian-testkit"),
     ("meridian", "meridian-sidecar", "meridian-bench", "e2-client")),
)


def prepare(root):
    source = root / ".local/src"
    source.mkdir(parents=True, exist_ok=True)
    evidence = root / "docs/experiments/data/E2"
    evidence.mkdir(parents=True, exist_ok=True)
    builds = []
    for name, url, commit, patch, packages, binaries in PROJECTS:
        workspace = source / name
        if not workspace.exists():
            subprocess.run(["git", "clone", "--filter=blob:none", "--no-checkout", url, workspace], check=True)
            subprocess.run(["git", "checkout", "--detach", commit], cwd=workspace, check=True)
        actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip()
        if actual != commit:
            raise RuntimeError(f"{workspace}: expected {commit}, found {actual}; not changing an existing checkout")
        patchpath = root / "patches" / patch
        applied = subprocess.run(["git", "apply", "--reverse", "--check", patchpath],
                                 cwd=workspace, capture_output=True).returncode == 0
        if not applied:
            subprocess.run(["git", "apply", "--check", patchpath], cwd=workspace, check=True)
            subprocess.run(["git", "apply", patchpath], cwd=workspace, check=True)
        environment = dict(os.environ)
        environment.pop("CI", None)
        environment.pop("CARGO_TARGET_DIR", None)
        environment["CARGO_INCREMENTAL"] = "0"
        environment.update(CARGO_PROFILE_FAST_INHERITS="release", CARGO_PROFILE_FAST_LTO="false",
                           CARGO_PROFILE_FAST_CODEGEN_UNITS="16")
        command = ["cargo", "build", "--locked", "--profile", "fast"]
        if name == "horizon":
            command.append("--no-default-features")
        for package, binary in zip(packages, binaries):
            command += ["-p", package, "--bin", binary]
        logpath = evidence / f"{name}-reproduce-build.log"
        print(f"E2 building {name}, one foreground command; output in {logpath}", flush=True)
        started = time.monotonic()
        with logpath.open("w") as log:
            subprocess.run(command, cwd=workspace, env=environment, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
        short = commit[:9] if name == "horizon" else commit[:7]
        installed = root / ".local/bin" / f"{name}-{short}"
        installed.mkdir(parents=True, exist_ok=True)
        for binary in binaries:
            shutil.copy2(workspace / "target/fast" / binary, installed / binary)
        builds.append({"workspace": name, "commit": commit, "command": command,
                       "incremental": False, "seconds": time.monotonic() - started,
                       "target_mib_after_build": int(subprocess.check_output(
                           ["du", "-sm", workspace / "target"], text=True).split()[0])})
        (evidence / "reproduce-builds.json").write_text(json.dumps(builds, indent=2) + "\n")
