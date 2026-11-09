"""Owns the VM host's direct-bot identity: its own stamped bots and disabled-trace proof.

The launch environment and arguments are the workstation module's, unchanged.
Only the identity differs, because bots compiled on the VM host have their own
hash and need their own proof.
"""
import json
from pathlib import Path

import e28_direct_bots as workstation
import goliath_bundle as bundle

ROOT=bundle.ROOT
STAMP_BUNDLE=ROOT/'.local/bin/goliath-e32/stamped-bots'
PROOF=ROOT/'docs/experiments/data/E32/direct-bots-proof.json'
CHECKER=ROOT/'scripts/e32-direct-bots-proof'
TRACE_ENV=workstation.TRACE_ENV
SOURCE_FILES=workstation.SOURCE_FILES
environment=workstation.environment
arguments=workstation.arguments


def launcher_hashes():
    return {path.name:bundle.sha(path) for path in [Path(__file__),Path(workstation.__file__)]}


def selection():
    identity=bundle.validate(STAMP_BUNDLE,{'stamped-bots'})
    proof=json.loads(PROOF.read_text())
    canonical=json.loads((STAMP_BUNDLE/'canonical.json').read_text())
    sources=canonical['projects']['horizon']['source_files']
    if proof.get('status')!='passed' or proof.get('bots_sha256')!=identity['binaries']['horizon-bots']:
        raise ValueError('direct bots need a passing proof for the selected stamped binary')
    if proof.get('manifest_sha256')!=identity['manifest_sha256']:
        raise ValueError('direct-bot proof belongs to another build manifest')
    if proof.get('checker_sha256')!=bundle.sha(CHECKER):
        raise ValueError('direct-bot proof checker changed')
    if proof.get('launcher_sha256')!=launcher_hashes():
        raise ValueError('direct-bot environment helper changed')
    if set(proof.get('source_sha256',{}))!=SOURCE_FILES or any(sources.get(name)!=digest for name,digest in proof['source_sha256'].items()):
        raise ValueError('direct-bot proof source differs from the frozen stamped sources')
    native=proof.get('native_probe',{})
    if native.get('disabled_clock_calls')!=0 or native.get('enabled_clock_calls')!=1:
        raise ValueError('direct-bot disabled clock path is unproven')
    return STAMP_BUNDLE,identity,bundle.sha(PROOF)
