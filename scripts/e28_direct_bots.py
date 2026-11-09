"""Owns identity and explicitly untraced launches of E28 direct capacity bots."""
import json
from pathlib import Path

import e11_support as support
import goliath_bundle as bundle

ROOT=support.ROOT
STAMP_BUNDLE=ROOT/'.local/bin/goliath-e28/final-stamps'
PROOF=ROOT/'docs/experiments/data/E28/direct-bots-proof.json'
TRACE_ENV=('E25_LATENCY_DIR','E25_LATENCY_NAMES','E25_LATENCY_TILE')
SOURCE_FILES={f'crates/horizon-bots/src/{name}' for name in ['main.rs','measured.rs','latency_trace.rs']}


def selection():
    identity=bundle.validate(STAMP_BUNDLE,{'final-stamps'})
    proof=json.loads(PROOF.read_text())
    canonical=json.loads((STAMP_BUNDLE/'canonical.json').read_text())
    sources=canonical['projects']['horizon']['source_files']
    if proof.get('status')!='passed' or proof.get('bots_sha256')!=identity['binaries']['horizon-bots']:
        raise ValueError('direct bots need a passing proof for the selected stamped binary')
    if proof.get('checker_sha256')!=bundle.sha(ROOT/'scripts/e28-direct-bots-tests'):
        raise ValueError('direct-bot proof checker changed')
    if proof.get('launcher_sha256')!=bundle.sha(Path(__file__)):
        raise ValueError('direct-bot environment helper changed')
    if set(proof.get('source_sha256',{}))!=SOURCE_FILES or any(sources.get(name)!=digest for name,digest in proof['source_sha256'].items()):
        raise ValueError('direct-bot proof source differs from the frozen stamped sources')
    native=proof.get('native_probe',{})
    if native.get('disabled_clock_calls')!=0 or native.get('enabled_clock_calls')!=1:
        raise ValueError('direct-bot disabled clock path is unproven')
    return STAMP_BUNDLE,identity,bundle.sha(PROOF)


def environment(folder):
    result=support.private_env(Path(folder))
    for name in TRACE_ENV:result.pop(name,None)
    return result


def arguments(group):
    return ['--handshake-host',group['host']]
