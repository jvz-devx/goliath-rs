"""Owns strict Linux bundle identities, separate from frozen Darwin identities.

Each Linux host has its own trust anchor, because the same sources compile to
different bytes on each machine. The workstation's is E28's build manifest and
is the default. `GOLIATH_HOST_PLATFORM=vm` selects the VM host's E32 manifest.
A bundle validates only against the manifest of the host it was built on.
"""
import hashlib
import importlib
import json
from pathlib import Path

import goliath_platform as platform

ROOT = Path(__file__).resolve().parent.parent
HOST = platform.host_platform()
EXPERIMENT, FOLDER = dict(workstation=('E28', 'goliath-e28'), vm=('E32', 'goliath-e32'))[HOST]
MANIFEST = ROOT / 'docs/experiments/data' / EXPERIMENT / 'build-manifest.json'
BUNDLES = ROOT / '.local/bin' / FOLDER
LINUX_FOLDERS = ('goliath-e28', 'goliath-e32')
# The VM host builds only the bots of the stamped stack, as a bundle of its own.
BOTS_ONLY = {'vm': 'stamped-bots'}.get(HOST)
# The complete server bundle that fixes and fixtures are compared with.
BASE_STACK = dict(workstation='final', vm='final-r2')[HOST]

DISPOSITION = ROOT / 'docs/experiments/data' / EXPERIMENT / 'gate-disposition.json'


def disposed_gates(bundle):
    """Gates the reviewer accepted for this bundle; any other failure blocks."""
    if not DISPOSITION.exists():
        return set()
    record = json.loads(DISPOSITION.read_text())
    return {gate for gate, entry in record.get('gates', {}).items()
            if entry.get('accepted') is True and entry.get('class') == 'toolchain-lint'
            and bundle in entry.get('bundles', [])}

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def is_e28(directory):
    """True for a bundle of either Linux host, so the other host's bundle is refused, not skipped."""
    parent = Path(directory).resolve().parent
    return parent == BUNDLES.resolve() or (parent.name in LINUX_FOLDERS and parent.parent == (ROOT/'.local/bin').resolve())


def direct_bots():
    """The direct-bot identity module for the selected host."""
    return importlib.import_module('e32_direct_bots' if HOST == 'vm' else 'e28_direct_bots')


def validate(directory, expected=None):
    directory = Path(directory).resolve()
    if directory.parent != BUNDLES.resolve():
        raise ValueError('E28 bundle must be an immediate child of .local/bin/'+BUNDLES.name)
    manifest = json.loads(MANIFEST.read_text())
    if manifest.get('schema_version') != 1 or manifest.get('platform') != 'linux':
        raise ValueError('invalid E28 build manifest schema/platform')
    if manifest.get('host', 'workstation') != HOST:
        raise ValueError('E28 build manifest belongs to another host platform')
    if HOST == 'vm' and platform.virtualization() in ('none', 'undetected'):
        raise ValueError('the VM host manifest was selected on a physical host')
    name = directory.name
    if expected and name not in expected:
        raise ValueError('wrong E28 bundle for this runner: '+name)
    record = manifest['bundles'][name]
    if record['path'] != str(directory.relative_to(ROOT)):
        raise ValueError('E28 bundle path differs from build manifest')
    files = record['files']
    if any(not path.is_file() or path.is_symlink() for path in directory.iterdir()):
        raise ValueError('E28 bundle must contain only regular files')
    if set(files) != {path.name for path in directory.iterdir()}:
        raise ValueError('E28 bundle has missing or unlisted files')
    required = {'horizon-client', 'horizon-bots', 'build.json', 'canonical.json'}
    if name == BOTS_ONLY:
        required -= {'horizon-client'}
    elif name != 'plain-09':
        required |= {'meridian', 'meridian-sidecar', 'goliath-store', 'e4-client',
            'manifest.json', 'candidate.json', 'FINAL.json', 'LEASES', 'cluster-launcher.py'}
    if name == 'final-lite':
        required |= {'source-proof.json'}
    if not required <= set(files):
        raise ValueError('E28 bundle lacks required files')
    for filename, digest in files.items():
        if Path(filename).name != filename or len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
            raise ValueError('invalid E28 file identity')
        if sha(directory/filename) != digest:
            raise ValueError('E28 frozen file changed: '+filename)
    binaries = record['binaries']
    expected_binaries = {'horizon-bots'} if name == BOTS_ONLY else {'horizon-client', 'horizon-bots'} if name == 'plain-09' else {
        'horizon-client', 'horizon-bots', 'meridian', 'meridian-sidecar', 'goliath-store', 'e4-client'}
    if not expected_binaries <= set(binaries):
        raise ValueError('E28 executable identities incomplete')
    for filename, digest in binaries.items():
        if files.get(filename) != digest or not (directory/filename).stat().st_mode & 0o111:
            raise ValueError('E28 executable identity or permission mismatch: '+filename)
    canonical = json.loads((directory/'canonical.json').read_text())
    build = json.loads((directory/'build.json').read_text())
    if build.get('status') != 'complete' or build.get('profile') != 'release':
        raise ValueError('E28 build is not a complete release')
    if canonical.get('source_equality') != 'passed':
        raise ValueError('E28 source equality did not pass')
    # A failed gate blocks unless the reviewer's disposition accepts that exact
    # gate for this bundle as toolchain lint drift (no behaviour change).
    accepted = disposed_gates(name)
    blocking = [gate for gate in build.get('failed_gates', []) if gate not in accepted]
    if blocking:
        raise ValueError('E28 build gates did not pass: '+', '.join(blocking))
    if build.get('verification_status', 'passed') != 'passed' and not build.get('failed_gates'):
        raise ValueError('E28 build verification did not pass')
    if canonical.get('host', 'workstation') != HOST:
        raise ValueError('E28 bundle was built on another host platform')
    for metadata in [canonical, build]:
        if any(metadata['binaries'].get(key) != digest for key, digest in binaries.items()):
            raise ValueError('E28 binary mapping differs between build records')
    for project in canonical.get('projects', {}).values():
        for patch in project.get('patches', []):
            if sha(ROOT/patch['path']) != patch['sha256']:
                raise ValueError('E28 source patch changed: '+patch['path'])
    return dict(bundle=name, manifest_sha256=sha(MANIFEST), binaries=binaries, files=files)


# The measured server stacks. final-r2 is final with the two soak fixes in the
# tile; every other executable is final's bytes.
# Each derived stack, with the only executables in which it may differ from the
# base. On the VM host final-r2-e29 is final-r2 with the E29 store.
STACK_DIFFERENCES = dict(workstation={'final-r2': {'horizon-client'}},
                         vm={'final-r2-e29': {'goliath-store'}})[HOST]
SERVER_STACKS = (BASE_STACK, *STACK_DIFFERENCES)


def validate_server_stack(directory):
    """Validate a measured server bundle; a derived stack may differ from its base in the named executables only."""
    identity = validate(directory, set(SERVER_STACKS))
    allowed = STACK_DIFFERENCES.get(identity['bundle'])
    if allowed is not None:
        base = validate(BUNDLES / BASE_STACK, {BASE_STACK})['binaries']
        changed = {name for name in set(base) | set(identity['binaries'])
                   if base.get(name) != identity['binaries'].get(name)}
        if changed != allowed:
            raise ValueError(f"E28 {identity['bundle']} must differ from {BASE_STACK} in {' and '.join(sorted(allowed))} only")
    return identity
