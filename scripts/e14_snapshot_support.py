"""Validate and retain the executable contract of an E14 binary snapshot."""

import hashlib
import json
from pathlib import Path
import re
import shutil
import tempfile
import goliath_bundle

REQUIRED = {'horizon-client', 'meridian', 'meridian-sidecar', 'e4-client', 'goliath-store'}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def final_records(final):
    if final.get('schema_version') != 1 or set(final.get('binaries', {})) != REQUIRED:
        raise ValueError('FINAL.json does not identify every required executable')
    records = {}
    for name, record in final['binaries'].items():
        if Path(record['path']).name != name or not re.fullmatch(r'[0-9a-f]{64}', record['sha256']):
            raise ValueError('invalid executable identity in FINAL.json: ' + name)
        records[name] = record['sha256']
    if final.get('store_binary_sha256', records['goliath-store']) != records['goliath-store']:
        raise ValueError('FINAL.json store identities disagree')
    return records


def check_files(directory, records):
    for name, expected in records.items():
        path = directory / name
        if not path.is_file() or sha256(path) != expected:
            raise ValueError('snapshot executable hash mismatch: ' + name)


def validate_snapshot(directory):
    directory = Path(directory)
    if goliath_bundle.is_e28(directory):
        goliath_bundle.validate(directory)
    final_path, marker = directory / 'FINAL.json', directory / 'LEASES'
    if marker.exists() and not final_path.is_file():
        raise ValueError('leased snapshot requires FINAL.json')
    if directory.name.startswith('cluster-e12-final-') and not final_path.is_file():
        raise ValueError('final snapshot lost FINAL.json')
    records = json.loads((directory / 'manifest.json').read_text())
    if set(records) != REQUIRED:
        raise ValueError('snapshot manifest does not identify every required executable')
    final = json.loads(final_path.read_text()) if final_path.is_file() else None
    if final is not None:
        if records != final_records(final):
            raise ValueError('snapshot manifest differs from FINAL.json')
        leased = bool(final.get('launcher', {}).get('leased_marker'))
        if marker.is_file() != leased:
            raise ValueError('LEASES marker differs from FINAL.json launcher contract')
        launcher = directory / 'cluster-launcher.py'
        expected = final.get('launcher', {}).get('sha256')
        if not expected or not launcher.is_file() or sha256(launcher) != expected:
            raise ValueError('snapshot launcher hash differs from FINAL.json')
    check_files(directory, records)
    return {'binaries': records, 'leased': marker.is_file(), 'final': final}


def create_snapshot(source, destination, launcher):
    source, destination, launcher = Path(source), Path(destination), Path(launcher)
    ready = (source / 'READY').read_bytes()
    final_raw = (source / 'FINAL.json').read_bytes() if (source / 'FINAL.json').is_file() else None
    marker_raw = (source / 'LEASES').read_bytes() if (source / 'LEASES').is_file() else None
    ready_records = dict(re.findall(r'\.local/bin/goliath-e12/(\S+) sha256=([0-9a-f]{64})', ready.decode()))
    if final_raw is not None:
        final = json.loads(final_raw)
        records = final_records(final)
        if ready_records and ready_records != records:
            raise ValueError('READY hashes differ from FINAL.json')
        if bool(final.get('launcher', {}).get('leased_marker')) != (marker_raw is not None):
            raise ValueError('published LEASES marker differs from FINAL.json')
        if sha256(launcher) != final.get('launcher', {}).get('sha256'):
            raise ValueError('published launcher differs from FINAL.json')
    else:
        if marker_raw is not None:
            raise ValueError('leased snapshot requires FINAL.json')
        records = ready_records
    if set(records) != REQUIRED:
        raise ValueError('publication does not identify every required executable')
    check_files(source, records)
    identity_data = json.dumps(records, sort_keys=True).encode()
    if final_raw is not None:
        identity_data += b'\0' + final_raw + b'\0' + (marker_raw or b'')
    identity = hashlib.sha256(identity_data).hexdigest()[:12]
    prefix = 'cluster-e12-final-' if final_raw is not None else 'cluster-e12-'
    target = destination / (prefix + identity)
    destination.mkdir(parents=True, exist_ok=True)
    if target.exists():
        validate_snapshot(target)
        if final_raw is not None and (target / 'FINAL.json').read_bytes() != final_raw:
            raise ValueError('existing snapshot FINAL.json differs from publication')
        return target
    with tempfile.TemporaryDirectory(prefix='.e14-snapshot-', dir=destination) as temporary:
        staging = Path(temporary) / 'snapshot'
        staging.mkdir()
        for name in records:
            shutil.copy2(source / name, staging / name)
        shutil.copy2(launcher, staging / 'cluster-launcher.py')
        (staging / 'READY').write_bytes(ready)
        (staging / 'manifest.json').write_text(json.dumps(records, indent=2) + '\n')
        if final_raw is not None:
            (staging / 'FINAL.json').write_bytes(final_raw)
        if marker_raw is not None:
            (staging / 'LEASES').write_bytes(marker_raw)
        validate_snapshot(staging)
        if ready != (source / 'READY').read_bytes():
            raise ValueError('READY changed during snapshot')
        if final_raw is not None and final_raw != (source / 'FINAL.json').read_bytes():
            raise ValueError('FINAL.json changed during snapshot')
        if marker_raw is not None and marker_raw != (source / 'LEASES').read_bytes():
            raise ValueError('LEASES changed during snapshot')
        staging.rename(target)
    return target
