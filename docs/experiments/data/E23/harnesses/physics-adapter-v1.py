"""Run unchanged E14 native physics against E23 bytes, with local-only image captures."""
import hashlib
import argparse
import gzip
import importlib.machinery
import importlib.util
import os
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import time
from e22_availability import Availability
ROOT = Path(__file__).resolve().parent.parent

def presence_probe(module, args):
    module.DATA.mkdir(parents=True, exist_ok=True)
    module.EVIDENCE.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='E23-presence-', dir=ROOT / '.local/runs') as folder:
        run = module.Run(Path(folder), False, variant='e12', snapshot=args.snapshot, name=args.name)
        try:
            run.start()
            run.leg('a', 'b', 'walk')
            run.reset([-80, -60, 64], 0)
            run.leg('b', 'a', 'walk')
            run.reset([64, -60, 64], 0)
            loaded = set()
            center = (0, 0)
            for row in run.observer.rows:
                if row['kind'] == 'chunk_center':
                    center = tuple(row['chunk'])
                    loaded = {key for key in loaded if max(abs(key[0] - center[0]), abs(key[1] - center[1])) <= 6}
                elif row['kind'] == 'chunk':
                    key = tuple(row['chunk'])
                    if max(abs(key[0] - center[0]), abs(key[1] - center[1])) <= 6:
                        loaded.add(key)
                elif row['kind'] == 'forget_chunk':
                    loaded.discard(tuple(row['chunk']))
            if (4, 4) not in loaded:
                raise RuntimeError('returning client is missing its underfoot column (4,4)')
            began = time.time_ns()
            for action in ('down', 'up', 'down', 'up'):
                run.control.ask('key space ' + action)
                time.sleep(0.08)
            time.sleep(0.2)
            abilities = [row for row in run.observer.rows if row['kind'] == 'client_abilities' and row['ns'] >= began]
            if not abilities or not abilities[-1]['flags'] & 2:
                raise RuntimeError('unchanged real-key flight sequence did not activate')
            run.result.update(status='complete', collection_complete=True, presence_probe=dict(underfoot_column=[4, 4], present=True, client_abilities=abilities[-1], exact_original_key_sequence=True))
        except BaseException as error:
            run.result.update(status='failed', error=str(error))
            raise
        finally:
            run.stop()
            module.save(args.name + '.json.gz', run.result)
        if not run.result['lease_availability']['complete'] or run.result['lease_availability']['lease_expiry_observed']:
            raise RuntimeError('native presence probe availability capture failed or lease expired')
    return 0

def main():
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--name')
    parser.add_argument('--snapshot', type=Path)
    parser.add_argument('--presence-probe', action='store_true')
    args, _ = parser.parse_known_args()
    availability = Availability(args.name) if args.name else None
    loader = importlib.machinery.SourceFileLoader('e23_physics_e14', str(ROOT / 'scripts/e14-physics'))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    module.DATA = ROOT / 'docs/experiments/data/E23/physics'
    module.EVIDENCE = ROOT / '.local/evidence/E23/physics'

    def quiet():
        request = ROOT / '.local/quiet-request'
        if not request.exists() or request.read_text() != os.environ.get('E23_QUIET_IDENTITY'):
            raise module.Quiet('E23 physics requires its own bounded quiet window')
    module.quiet = quiet
    leg, stop, start = (module.Run.leg, module.Run.stop, module.Run.start)

    def recorded_start(self):
        returned = start(self)
        if availability:

            def pid(port):
                values = {int(value) for value in subprocess.check_output(['lsof', '-nP', '-t', '-a', '-iTCP:' + str(port), '-sTCP:LISTEN'], text=True).split()}
                if len(values) != 1:
                    raise RuntimeError('fixture listener does not identify one owned process')
                return values.pop()
            availability.register('store', pid(self.address[1]))
            for label, tile in self.state['tiles'].items():
                endpoint = json.loads(Path(tile['endpoint']).read_text())
                availability.register('tile-' + label, pid(endpoint['port']))
        return returned

    def recorded_leg(self, *args, **kwargs):
        row = leg(self, *args, **kwargs)
        if row.get('screenshot'):
            row['screenshot'] = str((module.EVIDENCE / Path(row['screenshot']).name).relative_to(ROOT))
        return row

    def recorded_stop(self):
        lines = {}

        def scan():
            paths = [self.folder / 'cluster.log']
            if hasattr(self, 'cluster_root') and self.cluster_root.exists():
                paths += list(self.cluster_root.glob('*.log'))
            for path in paths:
                if path.exists():
                    observed = [line for line in module.complete_lines(path) if 'local cluster lease expired' in line.lower()]
                    if observed:
                        lines[path.name] = observed
        scan()
        stop(self)
        scan()
        self.result['acknowledgement_waiver'] = os.environ.get('E23_QUIET_WAIVER') or None
        self.result['e23_adapter'] = dict(native_physics_unchanged=True, physics_harness_sha256=hashlib.sha256((ROOT / 'scripts/e14-physics').read_bytes()).hexdigest(), adapter_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
        if availability:
            failure = availability.folder / 'lease-lines.json.gz'
            failure.write_bytes(gzip.compress(json.dumps(dict(lines=lines)).encode(), mtime=0))
            meta = availability.folder / 'physics-meta.json'
            meta.write_text(json.dumps(dict(action='uncapped-physics', failure_evidence=str(failure.relative_to(ROOT)))))
            self.result['lease_availability'] = availability.finish(meta)
    module.Run.leg, module.Run.stop, module.Run.start = (recorded_leg, recorded_stop, recorded_start)
    if args.presence_probe:
        return presence_probe(module, args)
    return module.main()
if __name__ == '__main__':
    raise SystemExit(main())
