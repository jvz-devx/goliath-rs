"""Harness-only store traces and exact limiter signal-call brackets for frozen executables."""
import csv
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import signal
import threading
import time
import e11_support as support

ROOT = support.ROOT
DATA = ROOT/'docs/experiments/data/E22'
MAX_EVENTS = 100000


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def batches(path):
    text=path.read_text();complete=text[:text.rfind('\n')+1]
    rows=list(csv.DictReader(io.StringIO(complete)))
    required={'start_us','execute_ms','barrier_ms'}
    if not rows or not required.issubset(rows[0]):
        raise RuntimeError('store trace has no complete batch timing rows')
    rows=[{key:float(value) for key,value in row.items()} for row in rows]
    gaps=[]
    fields=['admission_max_ms','queue_max_ms','execute_ms','barrier_ms','maintain_ms','background_ms']
    for index,row in enumerate(rows):
        reasons=[field for field in fields if row.get(field,0)>200]
        spacing=(row['start_us']-rows[index-1]['start_us'])/1000 if index else None
        if spacing is not None and spacing>200:reasons.append('batch-start-spacing-ms')
        if reasons:gaps.append(dict(index=index,reasons=reasons,start_spacing_ms=spacing,
            context=rows[max(0,index-1):min(len(rows),index+2)]))
    # A store that answers renewals without a batch is idle between hand-overs, so spacing
    # alone is no longer rare. Past 100 such gaps they are counted, not listed.
    idle=[gap for gap in gaps if gap['reasons']==['batch-start-spacing-ms']]
    spacing=dict(count=len(idle),max_ms=max((gap['start_spacing_ms'] for gap in idle),default=None),listed=len(idle)<=100)
    if not spacing['listed']:gaps=[gap for gap in gaps if gap['reasons']!=['batch-start-spacing-ms']]
    return dict(rows=len(rows),time_scope='store-relative Instant microseconds; startup origin is separately bracketed, not exact',
        incomplete_tail_bytes=len(text)-len(complete),spacing_only_gaps_over_200ms=spacing,
        gap_threshold_ms=200,gap_note='batch spacing can be idle time; it is not by itself a stall',
        maxima_ms={field:max(row.get(field,0) for row in rows) for field in fields},gaps_over_200ms=gaps)


class Proxy:
    def __init__(self, original, **overrides):self.original,self.overrides=original,overrides
    def __getattr__(self,name):return self.overrides[name] if name in self.overrides else getattr(self.original,name)


class Availability:
    def __init__(self,name):
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]*',name):raise RuntimeError('use a simple availability name')
        self.name=name
        self.folder=ROOT/'.local/evidence/E22/availability'/name
        self.folder.mkdir(parents=True,exist_ok=False)
        self.store_trace=self.folder/'store-batches.csv'
        self.events=[];self.dropped=0;self.roles={};self.limiters={};self.lock=threading.Lock()
        self.origin={};self.origin_thread=None;self.origin_stop=threading.Event()
        self.expiry_events=[]
        self.original_os=os;self.os_proxy=Proxy(os,kill=self.kill)
        self.previous_store_trace=os.environ.get('GOLIATH_STORE_TRACE')
        self.previous_support_os=support.os
        os.environ['GOLIATH_STORE_TRACE']=str(self.store_trace)
        support.os=self.os_proxy

    def register(self,role,pid):
        if role=='store' or role.startswith('tile-'):
            with self.lock:self.roles[pid]=role

    def kill(self,pid,action):
        with self.lock:
            role=self.roles.get(pid);limiter=self.limiters.get(threading.get_ident())
        if role is None or limiter is None or action not in [signal.SIGSTOP,signal.SIGCONT]:
            return self.original_os.kill(pid,action)
        before=time.monotonic_ns();wall=time.time_ns();error=None
        try:return self.original_os.kill(pid,action)
        except BaseException as failure:error=type(failure).__name__;raise
        finally:
            row=dict(pid=pid,role=role,signal='STOP' if action==signal.SIGSTOP else 'CONT',
                monotonic_before_ns=before,monotonic_after_ns=time.monotonic_ns(),wall_before_ns=wall,
                budget_cores=limiter,error=error)
            with self.lock:
                if len(self.events)<MAX_EVENTS:self.events.append(row)
                else:self.dropped+=1

    def budget(self,base):
        trace=self
        class Budget(base):
            def run(self):
                with trace.lock:trace.limiters[threading.get_ident()]=self.cores
                try:return super().run()
                finally:
                    with trace.lock:trace.limiters.pop(threading.get_ident(),None)
        return Budget

    def sampler(self,base):
        trace=self
        class Sampler(base):
            def register(self,role,pid):
                trace.register(role,pid)
                return super().register(role,pid)
        return Sampler

    def store_popen(self,original,*args,**kwargs):
        command=args[0] if args else kwargs.get('args')
        from goliath_platform import original_command
        store=bool(command and isinstance(command,(list,tuple)) and Path(original_command(command)[0]).name=='goliath-store')
        before=time.monotonic_ns();wall=time.time_ns()
        child=original(*args,**kwargs)
        if store:
            self.register('store',child.pid)
            self.origin=dict(store_pid=child.pid,spawn_before_monotonic_ns=before,spawn_before_wall_ns=wall,
                popen_return_monotonic_ns=time.monotonic_ns())
            output=kwargs.get('stdout');filename=getattr(output,'name',None);log=Path(filename) if filename else None
            def ready():
                deadline=time.monotonic()+10
                while not self.origin_stop.wait(.005):
                    if log and log.exists() and any(line.startswith('LISTEN ') for line in log.read_text(errors='replace').splitlines()):
                        self.origin['listen_observed_monotonic_ns']=time.monotonic_ns()
                        self.origin['listen_observed_wall_ns']=time.time_ns()
                        self.origin['origin_bracket_width_ms']=(self.origin['listen_observed_monotonic_ns']-before)/1e6
                        break
                    if child.poll() is not None or time.monotonic()>deadline:break
            self.origin_thread=threading.Thread(target=ready,name='E22-store-startup-clock',daemon=True)
            self.origin_thread.start()
        return child

    def instrument_load(self,original):
        trace=self
        def load(name,path):
            module=original(name,path)
            if Path(path).name=='e18-proxy':
                module.os=trace.os_proxy
                module.TrackedBudget=trace.budget(module.TrackedBudget)
            if Path(path).name=='e17-m4':
                factory=module.cluster_module
                def cluster_module(bins):
                    result=factory(bins)
                    original_popen=result.subprocess.Popen
                    result.subprocess=Proxy(result.subprocess,Popen=lambda *args,**kwargs:trace.store_popen(original_popen,*args,**kwargs))
                    start=result.Tile.start
                    def tile_start(tile):
                        try:
                            returned=start(tile)
                            trace.register('tile-'+tile.label,tile.server.process.pid)
                            return returned
                        except BaseException:
                            # A constructor interrupted before registration still owns its server.
                            if hasattr(tile,'server'):tile.server.stop()
                            raise
                    result.Tile.start=tile_start
                    stop=result.Tile.stop
                    def tile_stop(tile,*args,**kwargs):
                        try:return stop(tile,*args,**kwargs)
                        except BaseException as failure:
                            if 'local cluster lease expired' in str(failure).lower():
                                trace.expiry_events.append(dict(role='tile-'+tile.label,scope='teardown',wall_ns=time.time_ns()))
                            if hasattr(tile,'server'):tile.server.stop()
                            raise
                    result.Tile.stop=tile_stop
                    return result
                module.cluster_module=cluster_module
            return module
        return load

    def finish(self,raw_path):
        self.origin_stop.set()
        if self.origin_thread:self.origin_thread.join(1)
        support.os=self.previous_support_os
        if self.previous_store_trace is None:os.environ.pop('GOLIATH_STORE_TRACE',None)
        else:os.environ['GOLIATH_STORE_TRACE']=self.previous_store_trace
        result=dict(kind='frozen-executable-harness-availability',store_trace_enabled=True,
            executables_changed=False,lease_rules_changed=False,per_renewal_tile_trace=False,
            per_renewal_note='not exposed by frozen engine; separate investigation if expiry recurs',
            origin_bracket=self.origin,events_dropped=self.dropped,
            collection_scope='trace harvest and signal serialization after the CPU span; limiter records were memory-only',
            overhead_scope='existing store tracing enabled identically in both arms; its work remains in measured store CPU',
            harness_sha256=sha(Path(__file__)))
        try:
            if self.store_trace.exists():
                result['store_batches']=batches(self.store_trace)
                target=DATA/(self.name+'-store-batches.csv.gz')
                target.write_bytes(gzip.compress(self.store_trace.read_bytes(),mtime=0))
                result['store_batches'].update(path=str(target.relative_to(ROOT)),sha256=sha(target))
                connections=self.store_trace.with_suffix('.connections.csv')
                if connections.exists():
                    target=DATA/(self.name+'-store-connections.csv.gz')
                    target.write_bytes(gzip.compress(connections.read_bytes(),mtime=0))
                    result['store_connections']=dict(path=str(target.relative_to(ROOT)),sha256=sha(target))
            else:result['error']='store trace missing'
        except BaseException as failure:result['error']=support.sanitize(str(failure))
        raw=json.loads(raw_path.read_text()) if raw_path.exists() else {}
        cell=raw.get('cells',{}).get('650',{})
        events=sorted(self.events,key=lambda row:row['monotonic_before_ns'])
        signals=DATA/(self.name+'-limiter-signals.json.gz')
        signals.write_bytes(gzip.compress(json.dumps(dict(roles=self.roles,events=events,dropped=self.dropped),indent=2).encode(),mtime=0))
        result['limiter_signals']=dict(path=str(signals.relative_to(ROOT)),sha256=sha(signals),count=len(events),
            roles=self.roles,scope='exact successful/failed per-PID kill-call brackets from limiter threads, including final limiter resumes')
        expected={'store','tile-a','tile-b','tile-c','tile-d'}
        result['complete']=not result.get('error') and not self.dropped and set(self.roles.values())==expected
        lines=list(cell.get('causal_warnings',[]))+list(cell.get('failure_log',[]))
        evidence=cell.get('failure_evidence',{}).get('path') or raw.get('failure_evidence')
        if evidence and (ROOT/evidence).exists():
            captured=json.loads(gzip.decompress((ROOT/evidence).read_bytes()))
            lines += [line for values in captured.get('lines',{}).values() for line in values]
        result['lease_expiry_observed']=bool(self.expiry_events) or any('local cluster lease expired' in line.lower() for line in lines)
        result['expiry_events']=self.expiry_events
        summary=DATA/(self.name+'-availability.json')
        support.save(summary,result)
        if raw:
            raw['lease_availability']=result
            for measured in raw.get('cells',{}).values():measured['lease_availability']=result
            support.save(raw_path,raw)
        return result
