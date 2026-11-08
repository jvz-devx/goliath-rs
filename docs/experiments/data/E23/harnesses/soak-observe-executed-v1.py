"""E23 read-only tick profiling and long-run evidence collection."""
from collections import Counter
import base64
import gzip
import json
from pathlib import Path
import subprocess
import threading
import time
import xml.etree.ElementTree as ET

from e23_support import DATA, EVIDENCE, ROOT, load, quiet, save, sha


class Profile:
    def __init__(self,name,ns,holder):
        self.name,self.ns,self.holder = name,ns,holder
        self.evidence = EVIDENCE/name
        self.evidence.mkdir(parents=True,exist_ok=False)
        self.result = dict(kind='E23-busiest-tile-profile',profiling_perturbs_timing=True,
            maximum_additions_tick_fraction=.05,errors=[])
        self.thread = None
        self.calls = 0

    def warmup(self,instance,seconds):
        quiet()
        time.sleep(seconds-5)
        counters = self.ns['e23_native_counter']
        before = {label:counters(tile.server.process.pid) for label,tile in instance.tiles.items()}
        time.sleep(5)
        after = {label:counters(tile.server.process.pid) for label,tile in instance.tiles.items()}
        cores = {label:(after[label]['cpu_s']-row['cpu_s'])/(after[label]['at']-row['at']) for label,row in before.items()}
        self.tile = instance.tiles[max(cores,key=cores.get)]
        self.result['selection'] = dict(method='native process CPU during last five warmup seconds',
            cores=cores,before=before,after=after,tile=self.tile.label,pid=self.tile.server.process.pid)

    def begin(self):
        if self.thread is None:
            self.thread = threading.Thread(target=self.collect,daemon=True)
            self.thread.start()

    def collect(self):
        try:
            pid = self.tile.server.process.pid
            trace = self.evidence/'tile.trace'
            command = ['xcrun','xctrace','record','--template','Time Profiler','--attach',str(pid),
                '--time-limit','10s','--no-prompt','--output',str(trace)]
            began = time.time_ns()
            with (self.evidence/'record-command.txt').open('w') as output:
                child = subprocess.Popen(command,stdout=output,stderr=subprocess.STDOUT)
                self.holder['monitor'].register('fixture-tile-profiler',child.pid)
                exit_code = child.wait(timeout=90)
            self.result['collection'] = dict(command=command,exit=exit_code,began_ns=began,ended_ns=time.time_ns())
            if exit_code:
                raise RuntimeError('native Running profile collection failed')
            toc = subprocess.check_output(['xcrun','xctrace','export','--input',str(trace),'--toc'])
            (self.evidence/'toc.xml').write_bytes(toc)
            schemas = [table.get('schema') for table in ET.fromstring(toc).findall('.//table')]
            self.result['schemas'] = schemas
            for schema in ['time-profile','cpu-profile']:
                if schema in schemas:
                    raw = subprocess.check_output(['xcrun','xctrace','export','--input',str(trace),
                        '--xpath',f'/trace-toc/run[@number="1"]/data/table[@schema="{schema}"]'])
                    path = self.evidence/(schema+'.xml.gz')
                    path.write_bytes(gzip.compress(raw,mtime=0))
            self.result['running'] = running(self.evidence/'time-profile.xml.gz')
        except BaseException as error:
            self.result['errors'].append(str(error))

    def finish(self):
        if self.thread:
            self.thread.join(timeout=110)
            if self.thread.is_alive():
                self.result['errors'].append('profiler did not finish')
        self.result['verdict'] = 'unproven'
        self.result['reason'] = 'sampled symbols cannot bound inlined ownership and reader work below 5% of tick execution; off-tick CPU is reported separately'
        save(DATA/(self.name+'-profile.json'),self.result)


def running(path):
    """Running weights only. Never treat waiting-stack occupancy as CPU cost."""
    root = ET.fromstring(gzip.decompress(path.read_bytes()))
    definitions = {node.get('id'):node for node in root.iter() if node.get('id')}
    def resolve(node):
        while node is not None and node.get('ref'):
            node = definitions[node.get('ref')]
        return node
    roles,leaves,inclusive,states = Counter(),Counter(),Counter(),Counter()
    categories = Counter()
    examples = {}
    main_total = total = 0
    for row in root.findall('.//row'):
        state = resolve(row.find('thread-state')).text
        weight = int(resolve(row.find('weight')).text)
        states[state] += weight
        if state != 'Running':
            continue
        total += weight
        thread = resolve(row.find('thread')).get('fmt','')
        roles[thread] += weight
        tagged = resolve(row.find('tagged-backtrace'))
        back = resolve(tagged.find('backtrace')) if tagged is not None else None
        frames = [resolve(frame).get('name','') for frame in back.findall('frame')] if back is not None else []
        leaf = frames[0] if frames else 'missing unwind'
        leaves[leaf] += weight
        for frame in set(frames):
            if any(term in frame for term in ['tile_','lease','e11_','track_entities','poll_server_control','wait_for_next_server_tick']):
                inclusive[frame] += weight
        is_main = 'main' in thread.lower() and 'horizon-net-' not in thread
        if is_main:
            main_total += weight
            group = 'unattributed main work'
            if any('wait_for_next_server_tick' in frame for frame in frames):
                group = 'between-tick pacing and control, includes inlined warming'
            elif any('lease' in frame for frame in frames):
                group = 'lease checks on main'
            elif any('e11_' in frame for frame in frames):
                group = 'digest or raw tick measurement'
            elif any('track_entities' in frame for frame in frames):
                group = 'entity tracking, mixed readers and real players'
            categories[group] += weight
            examples.setdefault(group,frames[:14])
    return dict(method='Time Profiler Running weights; no blocked occupancy in CPU denominator',
        source_sha256=sha(path),total_cpu_seconds=total/1e9,main_cpu_seconds=main_total/1e9,
        states_weight_ns=dict(states),thread_cpu_seconds={k:v/1e9 for k,v in roles.items()},
        main_categories={k:dict(cpu_seconds=v/1e9,percent_main=100*v/main_total if main_total else None) for k,v in categories.items()},
        top_leaves=[dict(frame=k,cpu_seconds=v/1e9,percent_target=100*v/total if total else None) for k,v in leaves.most_common(30)],
        addition_inclusive_frames=[dict(frame=k,cpu_seconds=v/1e9) for k,v in inclusive.most_common()],
        example_stacks=examples,reader_attribution='not separable from real-player tracking in optimized symbols',
        ownership_attribution='may be inlined; missing named symbols are not zero cost')


class Soak:
    """Drain all ticks, retain process RSS and audit crossing cargo without engine edits."""
    def __init__(self,name,ns,holder):
        self.name,self.ns,self.holder = name,ns,holder
        self.seeded = {}
        self.memory,self.audits = [],[]
        self.batches = {}
        self.origin = None
        self.last_drain = self.last_audit = 0
        self.recovery = None

    def cluster(self,module):
        ask,stop = module.Tile.ask,module.Cluster.stop
        soak = self
        self.module = module

        def asked(tile,command):
            result = ask(tile,command)
            if command.startswith('console tp E17BotX') and command.endswith(' 12 -58 12'):
                actor = command.split()[2]
                if actor not in soak.seeded:
                    marker = f'E23Cargo-{int(actor.removeprefix("E17BotX")):04d}'
                    given = ask(tile,f'console give {actor} minecraft:diamond[minecraft:custom_name="{marker}"] 1')
                    if not given.get('output') or any(word in str(given).lower() for word in ['incorrect','unknown','no entity']):
                        raise RuntimeError('soak cargo give refused')
                    soak.seeded[actor] = dict(marker=marker,uuid=str(module.upstream.offline_uuid(actor)),reply=given)
            if command=='e11-samples' and tile.label in soak.batches:
                prior = soak.batches.pop(tile.label)
                result = dict(result,ticks=prior['ticks']+result['ticks'],
                    digests=prior['digests']+result['digests'],dropped=prior['dropped']+result['dropped'])
            return result

        def stopped(instance):
            if soak.origin is not None and soak.recovery is None:
                instance.stop_proxy()
                for tile in instance.tiles.values():
                    tile.stop()
                for tile in instance.tiles.values():
                    tile.start()
                for tile in instance.tiles.values():
                    tile.stop()
                soak.recovery = soak.reconcile(instance)
            return stop(instance)

        module.Tile.ask,module.Cluster.stop = asked,stopped
        return module

    def times(self,instance,original):
        self.instance = instance
        if self.origin is None and len(self.seeded)!=65:
            raise RuntimeError('not all 65 crossing actors received unique soak cargo')
        return original(instance)

    def sample(self,counter):
        if self.origin is None:
            self.origin = counter['at']
        elapsed = counter['at']-self.origin
        instance = self.instance
        pids = {'proxy':instance.proxy.process.pid,'store':instance.store_process.pid,
            **{'tile-'+name:tile.server.process.pid for name,tile in instance.tiles.items()},
            **{'sidecar-'+str(i):child.process.pid for i,child in enumerate(instance.sidecars)}}
        helper_pids = {role:pid for role,pid in self.holder['monitor'].roles.items()
            if role.startswith(('bots-','shim-')) or role=='observer-and-driver'}
        self.memory.append(dict(elapsed_seconds=elapsed,processes={role:self.ns['e23_native_counter'](pid) for role,pid in pids.items()},
            helper_processes={role:self.ns['e23_native_counter'](pid) for role,pid in helper_pids.items()}))
        if elapsed-self.last_drain>=60:
            for name,tile in instance.tiles.items():
                # Call through the wrapper only while no prior batch is attached.
                prior = self.batches.pop(name,dict(ticks=[],digests=[],dropped=0))
                batch = tile.ask('e11-samples')
                self.batches[name] = dict(ticks=prior['ticks']+batch['ticks'],
                    digests=prior['digests']+batch['digests'],dropped=prior['dropped']+batch['dropped'])
            self.last_drain = elapsed
        if elapsed-self.last_audit>=60:
            self.audit(instance,elapsed)
            self.last_audit = elapsed

    def audit(self,instance,elapsed):
        before = {name:instance.player(row['uuid']) for name,row in self.seeded.items()}
        snapshots = {name:tile.ask('server-status') for name,tile in instance.tiles.items()}
        after = {name:instance.player(row['uuid']) for name,row in self.seeded.items()}
        violations,unstable = [],[]
        acting = {}
        for tile,status in snapshots.items():
            for actor in status['players']:
                if actor.get('terrain_reader') or not actor.get('name','').startswith('E17Bot'):
                    continue
                if actor.get('placed') and not any(actor.get(key) for key in ['held','frozen','released']):
                    acting.setdefault(actor['name'],[]).append(tile)
        for name,locations in acting.items():
            if name in before:
                a,b = before[name],after[name]
                if (a['generation'],a['owner']) != (b['generation'],b['owner']):
                    unstable.append(name)
                    continue
                expected = list(instance.tiles)[b['owner']]
            else:
                expected = name[6].lower()
            if len(locations)>1 or locations != [expected]:
                violations.append(dict(name=name,acting=locations,expected=expected))
        generations = {name:dict(before=before[name]['generation'],after=after[name]['generation'],owner=after[name]['owner']) for name in before}
        self.audits.append(dict(elapsed_seconds=elapsed,acting=acting,generations=generations,
            crossing_during_query=unstable,violations=violations,
            snapshots=snapshots,scope='sequential status bracketed by crossing-actor store generations; changed generations remain unresolved'))
        print(f'E23 soak {elapsed:.1f}s: {len(acting)} acting identities, {len(unstable)} crossing query brackets, {len(violations)} owner violations',flush=True)
        if violations:
            raise RuntimeError('soak owner violation observed')

    def reconcile(self,instance):
        import gzip
        module = self.module
        recovery = load('e23_soak_saved',ROOT/'scripts/m3_chaos_support.py')
        names = [row['marker'] for row in self.seeded.values()]
        totals = Counter()
        files = []
        records = {row['uuid']:instance.player(row['uuid']) for row in self.seeded.values()}
        stale = []
        for label,tile in instance.tiles.items():
            for path in (tile.folder/'world/players/data').glob('*.dat'):
                tag = module.upstream.read_nbt(gzip.decompress(path.read_bytes()))
                counts = recovery.tag_counts(tag.get('Inventory',[]),names)
                totals.update(counts)
                files.append(dict(tile=label,uuid=path.stem,generation=tag.get('goliath:generation'),counts=dict(counts)))
                record = records.get(path.stem)
                if record and (record['owner']!=tile.index or record['generation']!=tag.get('goliath:generation')):
                    stale.append(files[-1])
            for kind in ['region','entities']:
                for tag in recovery.region_tags(tile.folder/'world',kind):
                    for entry in tag.get('block_entities' if kind=='region' else 'Entities',[]):
                        totals.update(recovery.tag_counts(entry,names))
        duplicates = {name:totals[name]-1 for name in names if totals[name]>1}
        missing = [name for name in names if totals[name]!=1]
        return dict(totals={name:totals[name] for name in names},duplicates=duplicates,missing=missing,
            stale_files=stale,files=files,records={uuid:dict(owner=row['owner'],generation=row['generation']) for uuid,row in records.items()},
            store_anchors_counted_as_extra_copies=False,after_clean_checkpoint_and_all_tile_recovery=True,
            pass_line_met=not duplicates and not missing and not stale)

    def finish(self):
        result = dict(kind='E23-soak-evidence',seeded=self.seeded,memory=self.memory,
            ownership_audits=self.audits,recovery=self.recovery,minute5_to30={},verdict='failed',
            ownership_scope='observed acting-session checks and final persisted ownership; not continuous per-action tracing')
        if self.memory:
            a = min(self.memory,key=lambda row:abs(row['elapsed_seconds']-300))
            b = min(self.memory,key=lambda row:abs(row['elapsed_seconds']-1800))
            for role,left in a['processes'].items():
                right = b['processes'][role]
                growth = right['rss_mb']/left['rss_mb']-1
                result['minute5_to30'][role] = dict(before_seconds=a['elapsed_seconds'],after_seconds=b['elapsed_seconds'],
                    before_mb=left['rss_mb'],after_mb=right['rss_mb'],growth_fraction=growth,under_10_percent=growth<.10)
            coverage = abs(a['elapsed_seconds']-300)<2 and abs(b['elapsed_seconds']-1800)<2
            result['memory_endpoint_coverage'] = coverage
            result['helper_minute5_to30'] = {role:dict(before_mb=left['rss_mb'],after_mb=b['helper_processes'][role]['rss_mb'],
                growth_fraction=b['helper_processes'][role]['rss_mb']/left['rss_mb']-1)
                for role,left in a['helper_processes'].items()}
            result['verdict'] = 'passed' if coverage and self.recovery and self.recovery['pass_line_met'] and all(row['under_10_percent'] for row in result['minute5_to30'].values()) and self.audits and not any(row['violations'] for row in self.audits) else 'failed'
        save(DATA/(self.name+'-soak.json.gz'),result)
