"""Owns strict E28 bundle identities, separate from frozen Darwin identities."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / 'docs/experiments/data/E28/build-manifest.json'
BUNDLES = ROOT / '.local/bin/goliath-e28'



DISPOSITION = ROOT / 'docs/experiments/data/E28/gate-disposition.json'


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
    return Path(directory).resolve().parent == BUNDLES.resolve()


def validate(directory, expected=None):
    directory = Path(directory).resolve()
    if not is_e28(directory):
        raise ValueError('E28 bundle must be an immediate child of .local/bin/goliath-e28')
    manifest = json.loads(MANIFEST.read_text())
    if manifest.get('schema_version') != 1 or manifest.get('platform') != 'linux':
        raise ValueError('invalid E28 build manifest schema/platform')
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
    if name != 'plain-09':
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
    expected_binaries = {'horizon-client', 'horizon-bots'} if name == 'plain-09' else {
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
SERVER_STACKS = ('final', 'final-r2')


def validate_server_stack(directory):
    """Validate a measured server bundle; final-r2 may differ from final in the tile only."""
    identity = validate(directory, set(SERVER_STACKS))
    if identity['bundle'] != 'final':
        base = validate(BUNDLES / 'final', {'final'})['binaries']
        changed = {name for name in set(base) | set(identity['binaries'])
                   if base.get(name) != identity['binaries'].get(name)}
        if changed != {'horizon-client'}:
            raise ValueError('E28 final-r2 must differ from final in horizon-client only')
    return identity
