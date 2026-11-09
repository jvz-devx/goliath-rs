"""Owns the D9-matched plain fixture and native five-cohort measurement."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import goliath_platform as platform
from e11_support import ROOT,RUNS,Engine,CpuBudget,usage,private_env,save,sanitize,wait,complete_json_lines
from e18_process import ProcessSampler
from e18_d9_layout import plan
from e17_flat09_metrics import thread_ids,thread_usage,outside_validity


def cohort_config(group):
    return {'speed':4,'path':group['path']} if group['kind']=='crossing' else {'speed':4,'spread':group['spread']}


def combined_telemetry(groups,rows):
    count=sum(group['count'] for group in groups)
    fields=['online','joined','failed','late_rounds','waited_ticks','distance','bytes_in','bytes_out']
    result={key:sum(rows[group['label']].get(key,0) for group in groups) for key in fields}
    result['elapsed']=sum(rows[group['label']].get('elapsed',0)*group['count'] for group in groups)/count
    return result


def movement_contract(bench,groups,first,last,wall):
    expected=0.
    records=[]
    reasons=[]
    for group in groups:
        name=group['label'];a,b=first[name],last[name]
        elapsed=b.get('elapsed',0)-a.get('elapsed',0)
        distance=b.get('distance',0)-a.get('distance',0)
        offered=bench.steady_route_speed(cohort_config(group),group['count'])*group['count']*elapsed
        fraction=distance/max(offered,1e-9)
        if elapsed<wall-2: reasons.append(name+' stale cohort telemetry')
        if fraction<.95: reasons.append(name+' route progress below 95%')
        if b.get('late_rounds',0)!=a.get('late_rounds',0): reasons.append(name+' cohort falling behind')
        if b.get('waited_ticks',0)-a.get('waited_ticks',0)>group['count']*wall*.2:
            reasons.append(name+' cohort waiting for chunks over 1%')
        expected+=offered
        records.append({'label':name,'players':group['count'],'expected_distance':offered,
            'distance':distance,'movement_fraction':fraction,'telemetry_seconds':elapsed})
    return expected,records,reasons


def cell(args,base):
    build=base.provenance()
    request=base.REQUEST
    if not request.exists() or json.loads(request.read_text()).get('owner')!='E17' or not base.ACK.exists():
        raise RuntimeError('missing D9 quiet request or E18 acknowledgement')
    until=time.monotonic()+300
    while os.getloadavg()[0]>4:
        if time.monotonic()>until: raise RuntimeError('D9 start could not settle to load <=4')
        time.sleep(1)
    start_load=os.getloadavg()
    bench=base.load_bench()
    layout=plan(args.cell,bench.route_anchor,seed=18)
    layout_hash=hashlib.sha256(json.dumps(layout,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    monitor=ProcessSampler().start()
    row={'kind':'E17-D9-plain09-cell','bots':args.cell,'repeat':args.repeat,'pass':False,'valid':False,
        'reasons':[],'build':build,'layout':layout,'layout_sha256':layout_hash,'start_load':start_load,
        'bot_commands':[],'placement_commands':[],'pig_commands':[],'samples':[],
        'validity_policy':{'start_load_max':4,'outside_process_average_core_max':.5,
            'outside_phases':['steady','whole_cell'],'fixture_load_does_not_void':True},
        'client_cpu_budget':'five native cohort processes share one aggregate core',
        'join_pacing':{'native_cadence_seconds':.25,'incremental_teleport_batch_max':16,
            'teleports_finish_before_warmup':True},'placement_attempts':[],
        'main_thread_identification':'earliest surviving native ID, with native name retained'}
    engine=None;clients=[];streams=[];limits=[]
    RUNS.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='E17-D9-plain-',dir=RUNS) as temporary:
        folder=Path(temporary)
        try:
            config={'seed':12345,'view_distance':3,'mobs':256,'speed':4,'groups':layout['groups'],
                'servers':[{'command':[str(base.BINS/'horizon-client'),'--resources','{resources}',
                    '--control-socket','{endpoint}','--dedicated','--nogui'],
                    'properties':{'difficulty':'peaceful','pause-when-empty-seconds':-1}}]}
            row['config']=config
            engine=Engine(folder/'engine',config)
            monitor.register('engine-0',engine.pid)
            main_id=engine.pid if platform.LINUX else min(thread_ids(engine.pid))
            limits.append(CpuBudget([engine.pid],4).start())
            for command in ['gamerule spawn_mobs false','gamerule advance_weather false','gamerule advance_time false',
                    'gamerule random_tick_speed 3','gamerule respawn_radius 0','time set day','kill @e[type=!minecraft:player]']:
                engine.console(command)
            for group in layout['groups']:
                command=[str(base.BINS/'horizon-bots'),'--event-loop','--server',f'127.0.0.1:{engine.port}',
                    '--bots',str(group['count']),'--behaviour','route','--seconds','7200','--join-every','.25',
                    '--view-distance','3','--seed','12345','--speed','4','--height','2','--name',group['prefix']]
                command+=['--spread',','.join(map(str,group['spread']))] if group['kind']=='interior' else ['--path',group['path']]
                log=folder/(group['label']+'.jsonl')
                stream=log.open('w');streams.append(stream)
                process=subprocess.Popen(platform.launch_command(command, 'bots'),cwd=folder,env=private_env(folder/(group['label']+'-home')),
                    stdout=stream,stderr=subprocess.STDOUT)
                clients.append((group,process,log));monitor.register('bots-'+group['label'],process.pid)
                row['bot_commands'].append(command)
            limits.append(CpuBudget([process.pid for _,process,_ in clients],1).start())
            def telemetry():
                return {group['label']:bench.latest(log) for group,_,log in clients}
            targets={name:(group,point) for group in layout['groups'][:4] for name,point in group['starts'].items()}
            preambles=set();requested=set()
            def inside(player,group):
                lo_x,lo_z,hi_x,hi_z=group['spread'];x,_,z=player['position']
                return lo_x<=x<=hi_x and lo_z<=z<=hi_z
            def joined():
                states=telemetry()
                if any(process.poll() is not None for _,process,_ in clients) or any(value.get('failed') for value in states.values()):
                    raise RuntimeError('D9 native client join failure: '+str(states))
                for group,_,log in clients[:4]:
                    if group['label'] in preambles: continue
                    rows=[value['routes'] for value in complete_json_lines(log) if 'routes' in value]
                    if not rows: continue
                    for route in rows[0]:
                        expected=group['starts'][route['name']]
                        if any(abs(a-b)>1e-8 for a,b in zip(route['start'],expected)):
                            raise RuntimeError('D9 native route preamble differs from shared plan')
                    preambles.add(group['label'])
                players=engine.ask('server-status')['players'];sent=0
                for player in players:
                    name=player['name']
                    if name not in targets: continue
                    group,expected=targets[name]
                    if group['label'] not in preambles or (name in requested and inside(player,group)): continue
                    command=f'tp {name} {expected[0]} -58 {expected[1]}'
                    engine.console(command);row['placement_attempts'].append(command)
                    if name not in requested: row['placement_commands'].append(command)
                    requested.add(name);sent+=1
                    if sent>=16: break
                actual={p['name']:p for p in engine.ask('server-status')['players']}
                return (all(states[group['label']].get('online')==group['count'] for group in layout['groups'])
                    and len(actual)==args.cell and len(preambles)==4
                    and all(name in actual and name in requested and inside(actual[name],group) for name,(group,_) in targets.items()))
            wait(joined,180,'D9 clients did not join')
            monitor.capture('joined')
            row['join_population']=engine.ask('server-status')
            for group in layout['groups'][:4]:
                cx,cz=group['center']
                for index in range(64):
                    command=f'summon minecraft:pig {cx+index%8-4} -60 {cz+index//8-4} {{Invulnerable:1b,PersistenceRequired:1b}}'
                    row['pig_commands'].append(command);engine.console(command)
            print(f'D9_CELL {args.cell} repeat={args.repeat}: warmup 30s',flush=True)
            until=time.monotonic()+30
            while time.monotonic()<until:
                if not request.exists(): raise RuntimeError('D9 quiet request vanished')
                time.sleep(min(.2,max(0,until-time.monotonic())))
            engine.ask('e11-samples')
            def snapshot():
                stamp=bench.game_time(engine)
                status=engine.ask('server-status');players=status['players']
                status['player_positions']=players
                status['players']=len(players)
                states=telemetry()
                if any(process.poll() is not None for _,process,_ in clients):
                    raise RuntimeError('D9 native client exited during measurement: '+str(states))
                client_usage={group['label']:usage(process.pid) for group,process,_ in clients}
                bot_usage={'cpu_s':sum(value['cpu_s'] for value in client_usage.values()),
                    'threads':sum(value['threads'] for value in client_usage.values()),
                    'rss_mb':sum(value['rss_mb'] for value in client_usage.values())}
                return {'at':time.monotonic(),'time':[stamp],'tiles':[status],'server_usage':[usage(engine.pid)],
                    'bot_usage':bot_usage,'bot_process_usage':client_usage,'bots':combined_telemetry(layout['groups'],states),
                    'cohorts':states,'main_threads':[thread_usage(engine.pid,main_id)],'load':os.getloadavg()}
            limiter_before=[limit.report() for limit in limits]
            samples=row['samples'];samples.append(snapshot());monitor.capture('steady-first');monitor.capture('steady-last')
            end=time.monotonic()+60
            while time.monotonic()<end:
                if not request.exists(): raise RuntimeError('D9 quiet request vanished')
                time.sleep(min(2,max(0,end-time.monotonic())))
                samples.append(snapshot());monitor.capture('steady-last')
            raw=[engine.ask('e11-samples')]
            wall=samples[-1]['at']-samples[0]['at']
            expected,cohorts,reasons=movement_contract(bench,layout['groups'],samples[0]['cohorts'],samples[-1]['cohorts'],wall)
            elapsed=samples[-1]['bots']['elapsed']-samples[0]['bots']['elapsed']
            # E11's 95% rule uses the exact sum of all five native route speeds.
            bench.steady_route_speed=lambda config,count:expected/(count*elapsed)
            evaluated=bench.evaluate(samples,raw,args.cell,4,1,float('inf'),256,config)
            row.update(evaluated,raw_ticks=raw,cohort_movement=cohorts,cpu_limits=[limit.report() for limit in limits],
                cpu_limits_measured=[dict(limit.report(),stops=limit.stops-before['stops']) for limit,before in zip(limits,limiter_before)])
            for sample in samples:
                for group in layout['groups']:
                    state=sample['cohorts'][group['label']]
                    if state.get('online')!=group['count'] or state.get('failed'):
                        reasons.append(group['label']+' cohort offline or failed')
            if reasons:
                row.update({'pass':False,'valid':False});row['reasons']+=sorted(set(reasons))
            a,b=samples[0]['main_threads'][0],samples[-1]['main_threads'][0]
            row['main_thread_cpu_cores']=[(b['cpu_s']-a['cpu_s'])/(b['at']-a['at'])]
            row['process_cpu_cores']={'engine':row['server_cpu_cores'],**{group['label']:
                (samples[-1]['bot_process_usage'][group['label']]['cpu_s']-samples[0]['bot_process_usage'][group['label']]['cpu_s'])/wall
                for group in layout['groups']}}
            if any(limit.error for limit in limits):
                row.update({'pass':False,'valid':False});row['reasons'].append('CPU limiter failed')
            if base.provenance()!=build:
                row.update({'pass':False,'valid':False});row['reasons'].append('D9 snapshot or instrumentation changed')
        except BaseException as error:
            row.update({'pass':False,'valid':False});row['reasons'].append(sanitize(error))
        finally:
            if not row.get('valid'):
                row['bot_tails']={group['label']:complete_json_lines(log)[-3:] for group,_,log in clients}
            for limit in limits: limit.stop()
            for _,process,_ in clients:
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(10)
                    except subprocess.TimeoutExpired: process.kill();process.wait()
            for stream in streams: stream.close()
            if engine: engine.stop()
            monitor.stop()
            process_report=monitor.report()
            for phase in process_report['phases'].values():
                if phase: phase['server_cores']=phase['fixture'].get('engine-0',{}).get('cores',0)
            row['outside_process_cpu']=process_report
            valid,reasons=outside_validity(process_report)
            if not valid:
                row.update({'pass':False,'valid':False});row['reasons']+=reasons
            save(args.out,row)
    print('D9_CELL_RESULT '+json.dumps({key:row.get(key) for key in ['bots','repeat','pass','valid','reasons',
        'tps','tick_p99_ms','server_cpu_cores','bot_cpu_cores','main_thread_cpu_cores','cpu_limits_measured']}),flush=True)
    return row
