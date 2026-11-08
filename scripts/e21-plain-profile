#!/usr/bin/env python3
"""E21's uncapped single-square plain-09 cost reference, not a D9 baseline."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from types import SimpleNamespace
import e11_support as support
import importlib.machinery
import importlib.util

ROOT=Path(__file__).resolve().parent.parent
DATA=ROOT/'docs/experiments/data/E21'


def load(name,path):
    loader=importlib.machinery.SourceFileLoader(name,str(path))
    spec=importlib.util.spec_from_loader(name,loader)
    module=importlib.util.module_from_spec(spec);loader.exec_module(module);return module


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--players',type=int,default=163)
    parser.add_argument('--name',required=True)
    parser.add_argument('--seconds',type=float,default=30)
    args=parser.parse_args()
    profile=load('e21_plain_profile',ROOT/'scripts/e21-tile-profile')
    bench=load('e21_plain_bench',ROOT/'scripts/capacity-bench')
    bins=ROOT/'.local/bin/meridian-e18/flat09-plain'
    canonical=json.loads((bins/'canonical.json').read_text())
    for name,digest in canonical['binaries'].items():
        if hashlib.sha256((bins/name).read_bytes()).hexdigest()!=digest:raise RuntimeError('plain-09 binary identity changed')
    target=DATA/(args.name+'.json')
    if target.exists():raise RuntimeError('result already exists')
    request=ROOT/'.local/quiet-request';ack=ROOT/'.local/quiet-ack-E20'
    if request.exists() or ack.exists():raise RuntimeError('quiet state exists')
    identity=json.dumps(dict(owner='E21',name=args.name,pid=os.getpid()))+'\n'
    began=time.monotonic();result=dict(kind='E21-plain09-per-tile-reference',status='waiting',acceptance=False,
        spatial_caveat='one 128-square, unlike D9 interior plus separate crossing cohort',canonical=canonical)
    with request.open('x') as stream:stream.write(identity)
    monitor=None
    try:
        while not ack.exists() or os.getloadavg()[0]>4:
            if time.monotonic()-began>600:raise TimeoutError('plain quiet acquisition timeout')
            time.sleep(.5)
        result.update(load_start=os.getloadavg(),ack_E20=ack.read_text())
        monitor=profile.ProcessSampler(observer=True).start()
        original_engine=bench.Engine;original_snapshot=bench.snapshot
        class Engine(original_engine):
            def __init__(self,*values,**keywords):
                super().__init__(*values,**keywords)
                monitor.register('plain',self.pid)
        bench.Engine=Engine
        def snapshot(engines,pids,bot_pid,bot_log):
            monitor.register('bots',bot_pid)
            value=original_snapshot(engines,pids,bot_pid,bot_log)
            value['native_threads']={'plain':profile.native_threads(engines[0].pid)}
            if 'steady-first' not in monitor.marks:monitor.capture('steady-first')
            monitor.capture('steady-last')
            return value
        bench.snapshot=snapshot
        config=dict(seed=12345,view_distance=3,speed=4,mobs=64,spread=[-64,-64,64,64],
            bot_binary=str(bins/'horizon-bots'),servers=[dict(command=[str(bins/'horizon-client'),
                '--resources','{resources}','--control-socket','{endpoint}','--dedicated','--nogui'],
                properties={'difficulty':'peaceful','pause-when-empty-seconds':-1},env={})])
        options=SimpleNamespace(servers=1,quiet_marker=str(request),uncapped_servers=True,
            bot_cores=1,server_cores=4,join_every=.25,warmup=30,seconds=args.seconds,sample_every=1,
            bot_only=False,max_load=float('inf'))
        row=bench.cell(config,args.players,0,options)
        row['thread_cpu']=profile.thread_deltas(row['samples'][0]['native_threads'],row['samples'][-1]['native_threads'])
        result.update(status='complete',cell=row)
    except BaseException as error:
        result.update(status='failed',error=repr(error));raise
    finally:
        if monitor:
            monitor.stop();result['processes']=monitor.report()
        if request.exists() and request.read_text()==identity:request.unlink();ack.unlink(missing_ok=True)
        result.update(seconds=time.monotonic()-began,load_end=os.getloadavg())
        support.save(target,result)
        print(json.dumps(dict(status=result['status'],cores=result.get('cell',{}).get('server_cpu_cores'),
            tps=result.get('cell',{}).get('tps')),indent=2),flush=True)


if __name__=='__main__':main()
