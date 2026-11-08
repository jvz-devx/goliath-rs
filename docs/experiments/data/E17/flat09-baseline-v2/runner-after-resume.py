#!/usr/bin/env python3
"""Run W1's unchanged workload on plain-09, releasing quiet windows per load step."""
import argparse
import contextlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from datetime import datetime,timezone
from types import SimpleNamespace
from e11_support import ROOT,save,sanitize
from e18_process import ProcessSampler
from e17_flat09_metrics import thread_ids,thread_usage,outside_validity
import hashlib

DATA=ROOT/'docs/experiments/data/E17/flat09-baseline'
BINS=ROOT/'.local/bin/meridian-e18/flat09-plain'
REQUEST=ROOT/'.local/quiet-request'
ACK=ROOT/'.local/quiet-ack-E18'


def load_bench():
    loader=importlib.machinery.SourceFileLoader('e17_flat09_bench',str(ROOT/'scripts/capacity-bench'))
    spec=importlib.util.spec_from_loader(loader.name,loader)
    module=importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def provenance():
    canonical=json.loads((BINS/'canonical.json').read_text())
    build=json.loads((BINS/'build.json').read_text())
    if build['profile']!='release' or canonical['source_equality']!='passed':
        raise RuntimeError('plain-09 is not a verified release snapshot')
    for name,value in canonical['binaries'].items():
        if digest(BINS/name)!=value: raise RuntimeError('plain-09 binary changed: '+name)
    for patch in canonical['projects']['horizon']['patches']:
        if digest(ROOT/patch['path'])!=patch['sha256']:
            raise RuntimeError('canonical patch changed: '+patch['path'])
    return {'canonical':canonical,'canonical_sha256':digest(BINS/'canonical.json'),
        'build_sha256':digest(BINS/'build.json'),'profile':build['profile'],
        'harness_sha256':digest(ROOT/'scripts/capacity-bench'),
        'limiter_sha256':digest(ROOT/'scripts/e11_support.py'),
        'process_sampler_sha256':digest(ROOT/'scripts/e18_process.py'),
        'runner_sha256':digest(Path(__file__).resolve()),
        'thread_metrics_sha256':digest(ROOT/'scripts/e17_flat09_metrics.py')}


@contextlib.contextmanager
def quiet(count):
    if REQUEST.exists() or ACK.exists(): raise RuntimeError('another request or stale E18 ack exists')
    began=time.monotonic()
    identity=json.dumps({'owner':'E17','name':f'plain09-baseline-{count}','pid':os.getpid(),
        'time':datetime.now(timezone.utc).isoformat(),'max_seconds':1560})+'\n'
    with REQUEST.open('x') as stream: stream.write(identity)
    metadata={'request':json.loads(identity),'status':'waiting'}
    try:
        notice=0.
        while not ACK.exists() or os.getloadavg()[0]>4:
            if not REQUEST.exists() or REQUEST.read_text()!=identity: raise RuntimeError('quiet request changed')
            if time.monotonic()-began>900: raise RuntimeError('quiet acknowledgement/load wait exceeded 15 minutes')
            if time.monotonic()>=notice:
                print('QUIET_WAIT '+json.dumps({'bots':count,'E18_ack':ACK.exists(),'load':os.getloadavg()}),flush=True)
                notice=time.monotonic()+30
            time.sleep(1)
        metadata.update(status='acquired',ack=ACK.read_text(),load_start=os.getloadavg(),wait_seconds=time.monotonic()-began)
        save(DATA/f'{count}-quiet.json',metadata)
        yield began+1560
        metadata['status']='complete'
    finally:
        if REQUEST.exists() and REQUEST.read_text()==identity:
            REQUEST.unlink()
            ACK.unlink(missing_ok=True)
        metadata.update(seconds=time.monotonic()-began,load_end=os.getloadavg())
        save(DATA/f'{count}-quiet.json',metadata)
        print('QUIET_RELEASE '+str(count),flush=True)


def cell(args):
    build=provenance()
    if not REQUEST.exists() or json.loads(REQUEST.read_text()).get('owner')!='E17' or not ACK.exists():
        raise RuntimeError('missing E17 quiet request or E18 acknowledgement')
    until=time.monotonic()+300
    while os.getloadavg()[0]>4:
        if time.monotonic()>until: raise RuntimeError('cell could not settle to start load <=4')
        time.sleep(1)
    start_load=os.getloadavg()
    bench=load_bench()
    sampler=ProcessSampler().start()
    engines=[]
    main_ids={}
    original_engine=bench.Engine
    original_snapshot=bench.snapshot
    original_wait=bench.wait
    # Keep Engine a class: capacity-bench uses isinstance for parallel setup.
    class MonitoredEngine(original_engine):
        def __init__(self,*values,**keywords):
            super().__init__(*values,**keywords)
            try:
                main_ids[self.pid]=min(thread_ids(self.pid))
                engines.append(self)
                sampler.register('engine-'+str(len(engines)-1),self.pid)
            except BaseException:
                self.stop()
                raise
    bench.Engine=MonitoredEngine
    def popen(command,**keywords):
        process=subprocess.Popen(command,**keywords)
        sampler.register('bots',process.pid)
        return process
    bench.subprocess=SimpleNamespace(**vars(subprocess))
    bench.subprocess.Popen=popen
    def joined(*values,**keywords):
        result=original_wait(*values,**keywords)
        sampler.capture('joined')
        return result
    bench.wait=joined
    def snapshot(*values,**keywords):
        value=original_snapshot(*values,**keywords)
        value['main_threads']=[thread_usage(instance.pid,main_ids[instance.pid]) for instance in engines]
        if 'steady-first' not in sampler.marks: sampler.capture('steady-first')
        sampler.capture('steady-last')
        return value
    bench.snapshot=snapshot
    config={'seed':12345,'view_distance':3,'speed':4,'mobs':64,'spread':[-64,-64,64,64],
        'bot_binary':str(BINS/'horizon-bots'),'servers':[{'command':[str(BINS/'horizon-client'),
            '--resources','{resources}','--control-socket','{endpoint}','--dedicated','--nogui']}]}
    options=SimpleNamespace(quiet_marker=str(REQUEST),servers=1,uncapped_servers=False,server_cores=4,
        bot_cores=1,join_every=.01,warmup=30,seconds=60,sample_every=2,bot_only=False,max_load=float('inf'))
    row={'bots':args.cell,'repeat':args.repeat,'pass':False,'valid':False,'reasons':[]}
    try:
        row=bench.cell(config,args.cell,args.repeat,options)
        sampler.stop()
        trace=sampler.report()
        valid,reasons=outside_validity(trace)
        if not valid:
            row['pass']=False
            row['valid']=False
            row['reasons']+=reasons
        first,last=row['samples'][0],row['samples'][-1]
        row['main_thread_cpu_cores']=[(b['cpu_s']-a['cpu_s'])/(b['at']-a['at'])
            for a,b in zip(first['main_threads'],last['main_threads'])]
        row['main_thread_identification']='earliest surviving native thread ID at engine startup, retained throughout cell'
        row['process_cpu_cores']={'engine':row['server_cpu_cores'],'bots':row['bot_cpu_cores']}
        row['outside_process_cpu']=trace
        row['start_load']=start_load
        row['validity_policy']={'start_load_max':4,'outside_process_average_core_max':.5,
            'outside_phases':['steady','whole_cell'],'fixture_load_does_not_void':True}
        if provenance()!=build:
            row.update({'pass':False,'valid':False})
            row['reasons'].append('snapshot or measurement code changed during cell')
    except BaseException as error:
        sampler.stop()
        row.update({'pass':False,'valid':False})
        row['reasons'].append(sanitize(error))
        row['outside_process_cpu']=sampler.report()
    finally:
        sampler.stop()
        row['build']=build
        save(args.out,row)
    print('CELL_RESULT '+json.dumps({key:row.get(key) for key in ['bots','repeat','pass','valid','reasons','tps',
        'tick_p99_ms','server_cpu_cores','bot_cpu_cores','main_thread_cpu_cores','cpu_limits_measured']}),flush=True)
    return row


def stop(process):
    if process.poll() is not None: return
    os.killpg(process.pid,signal.SIGCONT)
    os.killpg(process.pid,signal.SIGINT)
    try: process.wait(30)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid,signal.SIGTERM)
        try: process.wait(10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            process.wait()


def run(args):
    current=provenance()
    DATA.mkdir(parents=True,exist_ok=True)
    summary_path=DATA/'staircase.json'
    if summary_path.exists():
        if not args.resume: raise RuntimeError('staircase already exists; use explicit --resume for an unstarted step')
        summary=json.loads(summary_path.read_text())
        if summary['status']!='running': raise RuntimeError('only a between-step checkpoint may resume')
        rows=summary['rows']
        lower=summary.get('frontier_pass',0)
        upper=summary.get('frontier_fail')
        count=summary['active_count']
        if any(row['bots']==count for row in rows): raise RuntimeError('resume cannot overwrite or skip a partial step')
        for reference in rows:
            previous=json.loads((ROOT/reference['path']).read_text())
            for key in ['canonical_sha256','build_sha256','harness_sha256','limiter_sha256','process_sampler_sha256','thread_metrics_sha256']:
                if previous['build'][key]!=current[key]: raise RuntimeError('checkpoint instrumentation or snapshot changed: '+key)
        summary.setdefault('resumptions',[]).append({'count':count,'time':datetime.now(timezone.utc).isoformat(),
            'reason':'released between-step slot for E18 focused unit build; workload/measurement unchanged',
            'runner_sha256':current['runner_sha256']})
    else:
        if args.resume: raise RuntimeError('no checkpoint to resume')
        rows=[]
        summary={'kind':'E17-plain09-baseline','old_frontier_not_carried_forward':True,'rows':rows,
            'workload':{'seed':12345,'spread':[-64,-64,64,64],'view_distance':3,'speed':4,'pigs':64,
                'warmup_seconds':30,'measurement_seconds':60,'server_cpu_cap':4,'bot_cpu_cap':1},'status':'running'}
        lower=0
        upper=None
        count=200
    while True:
        summary['active_count']=count
        save(summary_path,summary)
        results=[]
        with quiet(count) as deadline:
            for repeat in range(2):
                for attempt in range(2):
                    path=DATA/f'{count}-r{repeat}-a{attempt}.json'
                    command=[sys.executable,'-B',str(Path(__file__).resolve()),'--cell',str(count),
                        '--repeat',str(repeat),'--out',str(path),'--data-dir',str(DATA)]
                    process=subprocess.Popen(command,cwd=ROOT,start_new_session=True)
                    try: process.wait(timeout=max(1,deadline-time.monotonic()-90))
                    except BaseException:
                        stop(process)
                        raise
                    if not path.exists(): raise RuntimeError('cell returned no retained result')
                    row=json.loads(path.read_text())
                    rows.append({'bots':count,'repeat':repeat,'attempt':attempt,'path':str(path.relative_to(ROOT)),
                        **{key:row.get(key) for key in ['pass','valid','reasons','tps','tick_p99_ms','server_cpu_cores',
                            'bot_cpu_cores','main_thread_cpu_cores','cpu_limits_measured']}})
                    save(summary_path,summary)
                    if row.get('valid') or not row.get('tps'): break
                results.append(row)
                if not row.get('tps'): break
        if any(not row.get('valid') for row in results):
            summary.update(status='incomplete-invalid-cell',frontier_pass=lower,frontier_fail=upper)
            break
        if all(row['pass'] for row in results): lower=max(lower,count)
        elif all(not row['pass'] for row in results): upper=count if upper is None else min(upper,count)
        else:
            summary.update(status='incomplete-mixed-repeat',frontier_pass=lower,frontier_fail=upper,mixed_load=count)
            break
        summary.update(frontier_pass=lower,frontier_fail=upper)
        save(summary_path,summary)
        if lower and upper and upper-lower<=25:
            summary.update(status='complete',tested_three_times_target=3*lower,
                conservative_three_times_target=3*upper)
            break
        if not lower and upper is not None and upper<=25:
            summary['status']='incomplete-no-passing-load'
            break
        count=2*count if upper is None else max(25,((lower+upper)//50)*25)
        if count==lower: count=lower+25
    save(summary_path,summary)
    print('FRONTIER '+json.dumps(summary),flush=True)


def main():
    global DATA
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cell',type=int)
    parser.add_argument('--repeat',type=int,default=0)
    parser.add_argument('--out',type=Path)
    parser.add_argument('--data-dir',type=Path,default=DATA)
    parser.add_argument('--resume',action='store_true',help='resume only a checkpoint between completed load steps')
    args=parser.parse_args()
    DATA=args.data_dir.resolve()
    if not DATA.is_relative_to(ROOT/'docs/experiments/data/E17'):
        parser.error('--data-dir must stay under E17 evidence')
    if args.cell:
        if not args.out: parser.error('--cell requires --out')
        cell(args)
    else: run(args)


if __name__=='__main__': main()
