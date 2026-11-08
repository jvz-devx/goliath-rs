"""E23 frozen identities, result ownership and harness-only availability capture."""
import ast
from datetime import datetime, timezone
import gzip
import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import re
import time

import e11_support as support
from e14_snapshot_support import validate_snapshot

ROOT = support.ROOT
DATA = ROOT / 'docs/experiments/data/E23'
EVIDENCE = ROOT / '.local/evidence/E23'
BINS = ROOT / '.local/bin/meridian-e18/reactor-1'
FROZEN = {
    'horizon-client': 'b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d',
    'meridian': '29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b',
    'goliath-store': '6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af',
    'meridian-sidecar': 'c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b',
    'horizon-bots': 'bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d',
    'e4-client': 'e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025',
}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def timestamp():
    return datetime.now(timezone.utc).isoformat()


def name_check(name):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]*', name):
        raise ValueError('use a simple fresh result name')


def save(path, value):
    """Sanitize all paths; caller retains distinct attempts and output identities."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = support.sanitize(json.dumps(value, indent=2, allow_nan=False)) + '\n'
    if path.suffix == '.gz':
        path.write_bytes(gzip.compress(raw.encode(), mtime=0))
    else:
        path.write_text(raw)


def pack(path):
    path = Path(path)
    if path.exists() and path.suffix == '.json' and path.stat().st_size > 1_000_000:
        target = path.with_suffix('.json.gz')
        if target.exists():
            raise RuntimeError('compressed evidence already exists')
        raw = path.read_bytes()
        target.write_bytes(gzip.compress(raw, mtime=0))
        if gzip.decompress(target.read_bytes()) != raw:
            raise RuntimeError('compression round-trip differs')
        path.unlink()
        return target
    return path


def frozen():
    actual = {name: sha(BINS / name) for name in FROZEN}
    if actual != FROZEN:
        raise RuntimeError('final-stack executable differs from E20 frozen identity')
    validate_snapshot(BINS)
    return dict(binaries=actual, manifests={name: sha(BINS / name) for name in
        ['manifest.json', 'candidate.json', 'canonical.json', 'FINAL.json', 'LEASES', 'cluster-launcher.py']})


def quiet():
    request = ROOT / '.local/quiet-request'
    if not request.exists() or request.read_text() != os.environ.get('E23_QUIET_IDENTITY'):
        raise RuntimeError('run through e23-window after explicit E22 host release')


class Ownership(ast.NodeTransformer):
    """Retarget only E22 result names and quiet identity in imported fixture code."""
    def visit_Constant(self, node):
        if isinstance(node.value, str):
            value = node.value.replace('E22', 'E23').replace('e22_', 'e23_')
            return ast.copy_location(ast.Constant(value), node)
        return node


def owned_fixture(path, name):
    """Preserve the exact executed adapter source beside its hash for inspection."""
    tree = ast.fix_missing_locations(Ownership().visit(ast.parse(Path(path).read_text())))
    source = ast.unparse(tree) + '\n'
    destination = DATA / 'harnesses' / (name + '.py')
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() and destination.read_text() != source:
        raise RuntimeError('executed adapter source changed; use a new attempt name')
    destination.write_text(source)
    module = type(os)('e23_' + name)
    module.__file__ = str(path)
    exec(compile(tree, str(path), 'exec'), module.__dict__)
    return module


def availability(name):
    """Use E22's signal capture unchanged, with E23-owned paths."""
    name_check(name)
    module = owned_fixture(ROOT / 'scripts/e22_availability.py', name + '-availability')
    module.MAX_EVENTS = 2_000_000
    return module.Availability(name)


def instrument_cluster(trace, factory, tiles=None):
    """Catch lease refusals and register exact owned PIDs without engine changes."""
    def cluster(bins):
        result = factory(bins)
        if tiles is not None:
            result.e4.TILES = dict(tiles)
        popen = result.subprocess.Popen
        result.subprocess = type(os)('e23_subprocess')
        for key in dir(__import__('subprocess')):
            setattr(result.subprocess, key, getattr(__import__('subprocess'), key))
        result.subprocess.Popen = lambda *a, **kw: trace.store_popen(popen, *a, **kw)
        start, ask, stop = result.Tile.start, result.Tile.ask, result.Tile.stop

        def tile_start(tile):
            try:
                returned = start(tile)
                trace.register('tile-' + tile.label, tile.server.process.pid)
                return returned
            except BaseException:
                if hasattr(tile, 'server'):
                    tile.server.stop()
                raise

        def tile_ask(tile, command):
            reply = ask(tile, command)
            text = json.dumps(reply).lower()
            if 'lease expired' in text or command == 'cluster-authority' and reply.get('authorized') is False:
                trace.expiry_events.append(dict(role='tile-' + tile.label, scope='control', wall_ns=time.time_ns()))
                raise RuntimeError('local cluster lease expired; stop this plan')
            return reply

        def tile_stop(tile, *args, **kwargs):
            try:
                return stop(tile, *args, **kwargs)
            finally:
                if hasattr(tile, 'server'):
                    tile.server.stop()

        result.Tile.start, result.Tile.ask, result.Tile.stop = tile_start, tile_ask, tile_stop
        return result
    return cluster


def finish_availability(trace, target, expected_tiles=('a', 'b', 'c', 'd')):
    result = trace.finish(target)
    expected = {'store', *('tile-' + label for label in expected_tiles)}
    result['complete'] = bool(not result.get('error') and not result['events_dropped']
        and set(trace.roles.values()) == expected)
    result['expected_roles'] = sorted(expected)
    save(DATA / (trace.name + '-availability.json'), result)
    raw = json.loads(Path(target).read_text()) if Path(target).exists() else {}
    raw['lease_availability'] = result
    for cell in raw.get('cells', {}).values():
        cell['lease_availability'] = result
    save(target, raw)
    if result['lease_expiry_observed']:
        save(DATA / (trace.name + '-availability-event.json'), result)
    return result
