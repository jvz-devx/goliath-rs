"""Packet and persisted-group acceptance fixtures for E12. No launcher management."""
import base64
import gzip
import json
import math
import os
import subprocess
import struct
import time
import zlib
import socket
import concurrent.futures


def cpu_seconds(pid):
    text = subprocess.check_output(['ps','-p',str(pid),'-o','time='],text=True).strip()
    parts = [float(p) for p in text.split(':')]
    return sum(p*60**i for i,p in enumerate(reversed(parts)))


def audit(module, client, token, start=False):
    began = time.time_ns()
    client.send(f'{"audit-start" if start else "audit"} {token}')
    return module.e8.wait_event(client,lambda e:e.get('kind')=='terrain_audit' and e.get('token')==token,began)


def ask_fresh(tile,command):
    endpoint=json.loads(tile.endpoint.read_text())
    with socket.create_connection(('127.0.0.1',endpoint['port']),timeout=60) as sock:
        reader=sock.makefile('r',encoding='utf-8')
        sock.sendall(f'auth {endpoint["token"]}\n'.encode())
        if json.loads(reader.readline()).get('ok') is not True:
            raise RuntimeError('fresh control authentication refused')
        sock.sendall((command+'\n').encode())
        line=reader.readline()
        if not line:
            raise RuntimeError('fresh control closed before reply')
        return json.loads(line)


def admission(module,cluster,args):
    result=dict(status='running',limit=cluster.max_expected,load_start=os.getloadavg())
    client=None
    try:
        with module.environment(module.env(cluster.root/'admission-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'admission-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        source,target=cluster.tiles['a'],cluster.tiles['b']
        # Do not mistake an observer's still-live startup expectation for our
        # explicit overload reservation. Wait for the fixture's table to drain.
        until=time.monotonic()+20
        while target.ask('transfer-status')['expected']:
            if time.monotonic()>until: raise RuntimeError('initial expected table did not drain')
            time.sleep(.05)
        dummy='00000000000000000000000000e12001'
        result['fill']=target.ask(f'player-expect {dummy} 20')
        if result['fill'].get('ok') is not True: raise RuntimeError('explicit admission overload reservation failed')
        uuid=str(module.upstream.offline_uuid('E4Probe'))
        began=time.time_ns()
        result['refused']=target.ask(f'player-expect {uuid} 20')
        time.sleep(.15)
        result['source']=source.ask('transfer-status')
        result['source_inputs']=len([value for value in module.e2.inputs(source) if value['ns']>began])
        if 'too many players are expected at once' not in json.dumps(result['refused']) or result['source']['frozen'] or not result['source_inputs']:
            raise RuntimeError('expected-limit refusal affected source authority or input')
        result['drop']=target.ask(f'player-drop {dummy}')
        result['retry']=target.ask(f'player-expect {uuid} 20')
        if result['retry'].get('ok') is not True: raise RuntimeError('admission could not retry after releasing reservation')
        target.ask(f'player-drop {uuid}')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        raise
    finally:
        if client: client.stop()
        module.save(args.name+'.json',result)


def radius(module,cluster,args):
    result=dict(status='running',load_start=os.getloadavg(),binary_sha256=module.e8.prereq.sha256(module.BINS/'horizon-client'))
    client=None
    try:
        with module.environment(module.env(cluster.root/'radius-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'radius-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        tile=cluster.tiles['a']
        deadline=time.monotonic()+20
        while True:
            status=tile.ask('server-status')
            readers=[p for p in status['players'] if p.get('terrain_reader')]
            if readers: break
            if time.monotonic()>deadline: raise RuntimeError('reader did not appear')
            time.sleep(.05)
        uuid=readers[0]['uuid']
        result['before']=readers[0]
        began=time.time_ns()
        tile.ask('console tp E4Probe 128.0 -60.0 128.0')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport' and e['pos']==[128,-60,128],began)
        request=json.dumps(dict(position=[128,-60,128],radius=8))
        result['request']=tile.ask(f'player-observe {uuid} {request}')
        expected={(0,8),(16,8),(8,0),(8,16)}
        deadline=time.monotonic()+20
        while True:
            observed=audit(module,client,'radius-8')
            status=tile.ask('server-status')
            reader=next(p for p in status['players'] if p['uuid']==uuid)
            if reader['view_distance']==8 and expected.issubset(set(map(tuple,observed['columns']))): break
            if time.monotonic()>deadline: raise RuntimeError('radius 8 tracker did not send its outer columns')
            time.sleep(.05)
        result.update(after=reader,outer_columns=sorted(expected),audit=observed,real_players=[p for p in status['players'] if p['placed'] and not p['terrain_reader']])
        if reader['placed'] or not reader['held'] or result['real_players'][0]['view_distance']!=3:
            raise RuntimeError('reader became placed or changed the real-player radius')
        result['invalid_radius']=tile.ask(f'player-observe {uuid} '+json.dumps(dict(position=[128,-60,128],radius=33)))
        result['import_refused']=tile.ask(f'player-import {uuid} '+json.dumps(dict(record={})))
        if result['invalid_radius'].get('ok') is True or result['import_refused'].get('ok') is True:
            raise RuntimeError('invalid radius or reader placement was accepted')
        tile.ask(f'player-observe {uuid} [128,-60,128]')
        result['legacy_refresh']=next(p for p in tile.ask('server-status')['players'] if p['uuid']==uuid)
        if result['legacy_refresh']['view_distance']!=8: raise RuntimeError('legacy refresh lost the explicit radius')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        raise
    finally:
        if client: client.stop()
        module.save(args.name+'.json',result)


def queue(module,cluster,args):
    result=dict(status='running',clients=args.clients,limit=8,seconds=args.seconds,load_start=os.getloadavg(),binary_sha256=module.e8.prereq.sha256(module.BINS/'horizon-client'))
    clients=[]
    tile=cluster.tiles['a']
    gate=tile.cut/'gate-pack-sync'
    try:
        for index in range(args.clients):
            with module.environment(module.env(cluster.root/f'queue-private-{index}')):
                child=module.e2.Child([module.BINS/'e4-client',cluster.port,f'E12Queue{index}'],cluster.root,cluster.root/f'queue-{index}.log')
            clients.append(child)
            module.e8.wait_event(child,lambda e:e.get('kind')=='teleport')
        uuid=str(module.upstream.offline_uuid('E12Queue0'))
        tile.ask(f'tile-cut {tile.token}')
        gate.touch()
        with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
            pending=[pool.submit(ask_fresh,tile,f'tile-cut {tile.token}') for _ in range(16)]
            deadline=time.monotonic()+20
            while not any(f.done() and 'cut queue is full' in json.dumps(f.result()) for f in pending):
                if time.monotonic()>deadline: raise RuntimeError('queue saturation control did not reject')
                time.sleep(.005)
            began=time.time_ns()
            result['refused_departure']=tile.ask(f'tile-cut {tile.token} {uuid}')
            result['refused_status']=tile.ask('transfer-status')
            time.sleep(.15)
            result['inputs_after_refusal']=len([p for p in module.e2.inputs(tile) if p['ns']>began])
            if 'cut queue is full' not in json.dumps(result['refused_departure']) or result['refused_status']['frozen'] or not result['inputs_after_refusal']:
                raise RuntimeError('full queue froze or stopped the source player')
            gate.unlink()
            result['saturation_replies']=[f.result() for f in pending]
        retry=tile.ask(f'tile-cut {tile.token} {uuid}')
        result['retry']=retry
        if retry.get('ok') is not True: raise RuntimeError('source could not retry after queue drained')
        result['retry_release']=tile.ask(f'player-release {uuid} {retry["export"]["record"]["handover"]} abort')
        result['before']=tile.ask('server-status')
        started=time.monotonic()
        until=started+args.seconds
        def drive():
            samples=[]
            while time.monotonic()<until:
                began=time.monotonic()
                reply=ask_fresh(tile,f'tile-cut {tile.token}')
                samples.append(dict(reply=reply,rtt_ms=(time.monotonic()-began)*1000,load=os.getloadavg()))
            return samples
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            groups=list(pool.map(lambda _:drive(),range(8)))
        elapsed=time.monotonic()-started
        samples=[sample for group in groups for sample in group]
        accepted=[s for s in samples if s['reply'].get('ok') is True]
        refused=[s for s in samples if s['reply'].get('ok') is not True]
        flushes=sum(1/s['reply']['group_size'] for s in accepted)
        result.update(samples=samples,elapsed=elapsed,accepted=len(accepted),refused=len(refused),durable_cuts_per_second=len(accepted)/elapsed,
            full_flushes=round(flushes),full_flushes_per_second=flushes/elapsed,group_sizes=sorted(set(s['reply']['group_size'] for s in accepted)),after=tile.ask('server-status'))
        if not accepted or any('cut queue is full' not in json.dumps(s['reply']) for s in refused):
            raise RuntimeError('throughput run had a non-overload failure')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        raise
    finally:
        gate.unlink(missing_ok=True)
        for child in clients: child.stop()
        module.save(args.name+'.json',result)


def live(module,cluster,args):
    result=dict(status='running',trials=[],requested=args.crossings,load_start=os.getloadavg(),
        binary_sha256={p:module.e8.prereq.sha256(module.BINS/p) for p in ['horizon-client','meridian','e4-client']})
    client=None
    def state(token):
        began=time.time_ns()
        client.send(f'state {token}')
        return module.e8.wait_event(client,lambda e:e.get('kind')=='live_state' and e['token']==token,began)
    def attrs(snapshot):
        return {a['attribute']:dict(base=a['base'],modifiers=sorted(a['modifiers'],key=lambda m:m['id'])) for a in snapshot['attributes']}
    try:
        with module.environment(module.env(cluster.root/'live-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'live-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        began=time.time_ns()
        client.send('sprint on')
        module.e8.wait_event(client,lambda e:e.get('kind')=='attributes' and any(m['id']=='minecraft:sprinting' for v in e['values'] for m in v['modifiers']),began)
        result['effect_command']=cluster.tiles['a'].ask('console effect give E4Probe minecraft:speed 120 1 true')
        time.sleep(.3)
        source='a'
        for index in range(args.crossings):
            if index==args.crossings//2:
                client.send('sprint off')
                client.send('fly on')
                time.sleep(.2)
            before=state(f'before-{index}')
            exported=cluster.tiles[source].ask(f'player-export {module.upstream.offline_uuid("E4Probe")}')
            if exported.get('ok') is not True:
                raise RuntimeError(f'source live snapshot refused: {exported}')
            authoritative=exported['record']['goliath_live']
            ability_bytes=bytes(authoritative['abilities'])
            _,offset=module.upstream.parse_varint(ability_bytes)
            flags,fly_speed,walk_speed=struct.unpack_from('>bff',ability_bytes,offset)
            expected_abilities=dict(flags=flags,flying_speed=fly_speed,walking_speed=walk_speed)
            metadata=bytes(authoritative['metadata'])
            _,offset=module.upstream.parse_varint(metadata)
            _,offset=module.upstream.parse_varint(metadata,offset)
            if metadata[offset]!=0:
                raise RuntimeError('source metadata does not start with shared flags')
            serializer,offset=module.upstream.parse_varint(metadata,offset+1)
            if serializer!=0:
                raise RuntimeError('source shared flags are not a byte')
            expected_flags=struct.unpack_from('b',metadata,offset)[0]
            expected=attrs(before)
            sprinting=index<args.crossings//2
            if not any(any(m['id']=='minecraft:sprinting' for m in a['modifiers']) for a in expected.values())==sprinting:
                raise RuntimeError('source sprint setup differs')
            target='b' if source=='a' else 'a'
            began=time.time_ns()
            client.send(f'walk live-{index} {-3 if target=="b" else 3} 1')
            module.e8.chat(client,f'E4 wall arrived {target}',began,30)
            module.e8.wait_event(client,lambda e:e.get('kind')=='reached' and e.get('token')==f'live-{index}',began)
            time.sleep(.2)
            after=state(f'after-{index}')
            observed=attrs(after)
            if any(observed.get(key)!=value for key,value in expected.items()):
                raise RuntimeError('live movement attribute or modifier changed across the swap')
            if expected_flags!=after['flags'] or expected_abilities!=after['abilities']:
                raise RuntimeError('live flags or abilities changed across the swap')
            packets=[e for e in client.events if e['ns']>=began and e.get('kind') in ['attributes','flags','abilities']]
            if sprinting and any(e.get('kind')=='attributes' and any(v['attribute'] in expected and
                    any(m['id']=='minecraft:sprinting' for m in expected[v['attribute']]['modifiers']) and
                    not any(m['id']=='minecraft:sprinting' for m in v['modifiers']) for v in e['values']) for e in packets):
                raise RuntimeError('swap briefly sent an empty sprint modifier list')
            result['trials'].append(dict(index=index,source=source,target=target,before=before,source_flags=expected_flags,source_abilities=expected_abilities,after=after,packets=packets,load=os.getloadavg()))
            source=target
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        if client:
            result['client_tail']=client.events[-20:]
        result['proxy_tail']=(cluster.root/'proxy.log').read_text().rpartition('\n')[0].splitlines()[-20:]
        raise
    finally:
        if client:
            client.stop()
        module.save(args.name+'.json',result)


def teleport(module,cluster,args):
    result=dict(status='running',placements=[],load_start=os.getloadavg())
    client=None
    try:
        with module.environment(module.env(cluster.root/'teleport-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'teleport-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        client.send('hold on')
        held=module.e8.wait_event(client,lambda e:e.get('kind')=='hold' and e['active'])
        for index,position in enumerate([[70,-60,70],[6,-60,6]]):
            began=time.time_ns()
            cluster.tiles['a'].ask('console tp E4Probe '+' '.join(f'{float(v):.1f}' for v in position))
            placed=module.e8.wait_event(client,lambda e:e.get('kind')=='teleport' and e['pos']==position,began)
            deadline=time.monotonic()+30
            while True:
                observed=audit(module,client,f'placement-{index}')
                result['last_audit']=observed
                if not observed['missing']:
                    break
                if time.monotonic()>deadline:
                    raise RuntimeError('server placement did not load its complete view without client inputs')
                time.sleep(.05)
            result['placements'].append(dict(position=position,audit=observed,ready_ms=(observed['ns']-placed['ns'])/1e6,load=os.getloadavg()))
        result['moves_while_held']=[e for e in client.events if e.get('kind')=='move' and e['ns']>held['ns']]
        if result['moves_while_held']:
            raise RuntimeError('no-input teleport control sent movement')
        client.send('hold off')
        began=time.time_ns()
        client.send('walk crossing -3 6')
        module.e8.chat(client,'E4 wall arrived b',began,30)
        result['first_target_input']=module.e4.wait_position(cluster.tiles['b'],began,'b')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        result['server_status']={label:tile.ask('server-status') for label,tile in cluster.tiles.items()}
        if client:
            result['client_packets']=[e for e in client.events if e.get('kind') in ['teleport','chunk','forget','hold']]
        result['proxy_tail']=(cluster.root/'proxy.log').read_text().rpartition('\n')[0].splitlines()[-40:]
        raise
    finally:
        if client:
            client.stop()
        module.save(args.name+'.json',result)


def borders(module,cluster,args):
    """All sixteen tile walls. Expected owned columns come from geometry, not packets."""
    result=dict(status='running',view_distance=3,edge=cluster.edge,walls=[],load_start=os.getloadavg(),
        binary_sha256={p:module.e8.prereq.sha256(module.BINS/p) for p in ['horizon-client','meridian','e4-client']})
    client=None
    try:
        with module.environment(module.env(cluster.root/'borders-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'borders-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        for label in ['a','b','c','d']:
            ix,iz=module.e4.TILES[label]
            lowx,lowz=ix*cluster.edge,iz*cluster.edge
            highx,highz=lowx+cluster.edge,lowz+cluster.edge
            for wall,position in [('west',[lowx+6,-60,lowz+38]),('east',[highx-6,-60,lowz+38]),
                    ('north',[lowx+38,-60,lowz+6]),('south',[lowx+38,-60,highz-6])]:
                began=time.time_ns()
                client.send('hold on')
                module.e8.wait_event(client,lambda e:e.get('kind')=='hold' and e['active'],began)
                cluster.tiles[label].ask('console tp E4Probe '+' '.join(f'{float(v):.1f}' for v in position))
                placed=module.e8.wait_event(client,lambda e:e.get('kind')=='teleport' and e['pos']==position,began)
                cx,cz=math.floor(position[0]/16),math.floor(position[2]/16)
                expected={(x,z) for x in range(cx-3,cx+4) for z in range(cz-3,cz+4)
                    if lowx//16<=x<highx//16 and lowz//16<=z<highz//16}
                deadline=time.monotonic()+30
                while True:
                    observed=audit(module,client,f'{label}-{wall}')
                    missing=expected-{tuple(p) for p in observed['columns']}
                    if not missing or time.monotonic()>deadline:
                        break
                    time.sleep(.05)
                entry=dict(tile=label,wall=wall,position=position,expected=sorted(expected),received=observed['columns'],missing=sorted(missing),
                    ready_ms=(observed['ns']-placed['ns'])/1e6,load=os.getloadavg())
                result['walls'].append(entry)
                module.save(args.name+'.json',result)
                if missing:
                    raise RuntimeError(f'{label} {wall} withheld owned columns: {sorted(missing)}')
                client.send('hold off')
                print(f'E12 owned border {label}/{wall}: {len(expected)} expected, zero missing',flush=True)
            if label!='d':
                # Return inside this tile, then use the real fenced wall path to the next one.
                position=[lowx+6 if ix==0 else highx-6,-60,lowz+6 if iz==0 else highz-6]
                began=time.time_ns()
                cluster.tiles[label].ask('console tp E4Probe '+' '.join(f'{float(v):.1f}' for v in position))
                module.e8.wait_event(client,lambda e:e.get('kind')=='teleport' and e['pos']==position,began)
                next_label={'a':'b','b':'c','c':'d'}[label]
                x,z={'b':(-6,6),'c':(-6,-6),'d':(6,-6)}[next_label]
                began=time.time_ns()
                client.send(f'walk next-{next_label} {x} {z}')
                module.e8.chat(client,f'E4 wall arrived {next_label}',began,30)
                module.e8.wait_event(client,lambda e:e.get('kind')=='reached' and e.get('token')==f'next-{next_label}',began)
        if len(result['walls'])!=16:
            raise RuntimeError('incomplete owned-wall matrix')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        if client:
            result['client_tail']=client.events[-10:]
        raise
    finally:
        if client:
            client.stop()
        module.save(args.name+'.json',result)


def terrain(module, cluster, args):
    result = dict(status='running',trials=[],crossings=args.crossings,load_start=os.getloadavg(),terrain_off=args.terrain_off)
    result['binary_sha256']={p:module.e8.prereq.sha256(module.BINS/p) for p in ['horizon-client','meridian','e4-client']}
    client = None
    try:
        pid=cluster.proxy.process.pid
        join_cpu=cpu_seconds(pid)
        join_started=time.monotonic()
        with module.environment(module.env(cluster.root/'terrain-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'terrain-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        deadline=time.monotonic()+30
        while True:
            initial=audit(module,client,'standing')
            expected={(x,z) for x in (range(4) if args.terrain_off else range(-4,4)) for z in (range(4) if args.terrain_off else range(-4,4))}
            if expected.issubset({tuple(p) for p in initial['columns']}):
                break
            if time.monotonic()>deadline:
                raise RuntimeError(f'standing view incomplete: {initial["missing"]}')
            time.sleep(.1)
        result['standing']=initial
        result['join_cost']=dict(seconds=time.monotonic()-join_started,proxy_cpu_seconds=cpu_seconds(pid)-join_cpu,client_bytes=initial['bytes'],columns=len(initial['columns']),load=os.getloadavg())
        if not args.terrain_off:
            audit(module,client,'start',True)
        entity_start=time.time_ns()
        result['foreign_entity_command']=cluster.tiles['b'].ask('console summon minecraft:cow -3 -60 1 {NoAI:1b,Tags:["E12Foreign"]}')
        time.sleep(.3)
        result['foreign_entities_seen']=[e for e in client.events if e.get('kind')=='entity' and e['ns']>=entity_start and e['pos'][0]<0]
        if result['foreign_entities_seen']:
            raise RuntimeError('foreign entity appeared on the source tile')
        began=time.time_ns()
        result['block_command']=cluster.tiles['b'].ask('console setblock -3 -59 1 minecraft:diamond_block')
        if not args.terrain_off:
            changed=module.e8.wait_event(client,lambda e:e.get('kind') in ['block_update','section_update'] and (e.get('pos')==[-3,-59,1] or e.get('section')==[-1,-4,0]),began)
            result['foreign_block_update']=changed
            client.send('break -3 -59 1')
            time.sleep(.2)
            result['read_only']=cluster.tiles['b'].ask('console execute if block -3 -59 1 minecraft:diamond_block run say E12_READONLY')
            if not any('E12_READONLY' in line for line in result['read_only'].get('output',[])):
                raise RuntimeError('a foreign block changed through the current player lane')
        before=audit(module,client,'bandwidth-before')
        cpu_before=cpu_seconds(pid)
        began=time.monotonic()
        # Same route and client cadence. Preparation/activation delays may differ.
        for index in range(args.crossings):
            target=module.e4.ROUTE[index%len(module.e4.ROUTE)]
            x,z=module.e4.POINTS[target]
            sent=time.time_ns()
            client.send(f'walk {index} {x} {z}')
            module.e8.chat(client,f'E4 wall arrived {target}',sent,30)
            module.e8.wait_event(client,lambda e:e.get('kind')=='reached' and e.get('token')==str(index),sent)
            observed=audit(module,client,str(index))
            result['trials'].append(dict(index=index,target=target,load=os.getloadavg(),audit=observed))
            if not args.terrain_off and (observed['missing'] or observed['missing_frames'] or observed['visible_reloads']):
                raise RuntimeError('missing or reloaded visible column in packet stream')
            if index%25==0:
                module.save(args.name+'.json',result)
                print(f'E12 terrain {index+1}/{args.crossings}',flush=True)
        if not args.terrain_off:
            result['entity_positive_control']=[e for e in client.events if e.get('kind')=='entity' and e['ns']>=entity_start and e['pos'][0]<0]
            if not result['entity_positive_control']:
                raise RuntimeError('foreign entity never appeared after entering its owning tile')
        after=audit(module,client,'bandwidth-after')
        seconds=time.monotonic()-began
        cpu=cpu_seconds(pid)-cpu_before
        result['cost']=dict(seconds=seconds,proxy_cpu_seconds=cpu,proxy_cpu_percent=100*cpu/seconds,client_bytes=after['bytes']-before['bytes'],client_bytes_per_second=(after['bytes']-before['bytes'])/seconds,load=os.getloadavg())
        result['final']=after
        result['disconnects']=sum(e.get('kind')=='disconnect' for e in client.events)
        if result['disconnects']:
            raise RuntimeError('terrain client disconnected')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        if client:
            result['client_tail']=client.events[-15:]
            result['client_log_tail']=(cluster.root/'terrain-client.log').read_text().rpartition('\n')[0].splitlines()[-8:]
        result['proxy_tail']=(cluster.root/'proxy.log').read_text().rpartition('\n')[0].splitlines()[-20:]
        raise
    finally:
        try:
            if client:
                client.stop()
        finally:
            module.save(args.name+'.json',result)


def vehicle_uuid(module, record):
    blob=base64.b64decode(record['body']['record']['player'])
    tag=module.upstream.read_nbt(blob)
    root=tag.get('RootVehicle',{})
    entity=root.get('Entity',{})
    return entity.get('UUID'),entity.get('id'),entity.get('Pos')


def saved_entities(module, world, uuid):
    count=0
    def visit(entity):
        return int(entity.get('UUID')==uuid)+sum(visit(e) for e in entity.get('Passengers',[]))
    for path in world.glob('**/entities/r.*.*.mca'):
        raw=path.read_bytes()
        for slot in range(1024):
            location=int.from_bytes(raw[slot*4:slot*4+4],'big')
            offset=(location>>8)*4096
            if not offset:
                continue
            size=int.from_bytes(raw[offset:offset+4],'big')
            codec=raw[offset+4]
            data=raw[offset+5:offset+4+size]
            if codec==2:
                data=zlib.decompress(data)
            elif codec==1:
                data=gzip.decompress(data)
            elif codec!=3:
                raise RuntimeError(f'unsupported entity region compression {codec}')
            tag=module.upstream.read_nbt(data)
            count+=sum(visit(e) for e in tag.get('Entities',[]))
    return count


def vehicles(module, cluster, args):
    result=dict(status='running',kind=args.kind,trials=[],requested=args.crossings,load_start=os.getloadavg())
    client=None
    try:
        with module.environment(module.env(cluster.root/'vehicle-private')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'vehicle-client.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='teleport')
        tile=cluster.tiles['a']
        # All types use the same kinematic client route. Minecart world physics is not the driver.
        kind={'boat':'oak_boat','minecart':'minecart','horse':'horse'}[args.kind]
        result['summon']=tile.ask(f'console summon minecraft:{kind} 1 -60 1 {{Tags:["E12Vehicle"],NoGravity:1b,NoAI:1b,Tame:1b}}')
        mounted_at=time.time_ns()
        entity=module.e8.wait_event(client,lambda e:e.get('kind')=='entity' and abs(e['pos'][0]-1.5)<.1 and abs(e['pos'][2]-1.5)<.1,timeout=10)
        client.send(f'mount {entity["entity"]}')
        result['mounted_packet']=module.e8.wait_event(client,lambda e:e.get('kind')=='passengers' and e.get('passengers'),mounted_at,10)
        uuid=str(module.upstream.offline_uuid('E4Probe'))
        time.sleep(.2)
        exported=tile.ask(f'player-export {uuid}')
        if exported.get('ok') is not True:
            raise RuntimeError(f'group export refused: {exported}')
        tag=module.upstream.read_nbt(base64.b64decode(exported['record']['player']))
        root=tag.get('RootVehicle',{}).get('Entity',{})
        if not root:
            raise RuntimeError('mounted export has no RootVehicle')
        offset=tag['Pos'][1]-root['Pos'][1]
        client.send(f'vehicle on {offset}')
        identity=None
        source='a'
        for index in range(args.crossings):
            target='b' if source=='a' else 'a'
            x=-3 if target=='b' else 3
            began=time.time_ns()
            client.send(f'walk {index} {x} 1')
            module.e8.chat(client,f'E4 wall arrived {target}',began,30)
            module.e8.wait_event(client,lambda e:e.get('kind')=='reached' and e.get('token')==str(index),began)
            record=cluster.player(uuid)
            vehicle=vehicle_uuid(module,record)
            if identity is None:
                identity=vehicle[0]
            if vehicle[0] is None or vehicle[0]!=identity or record['generation']!=index+1:
                raise RuntimeError('vehicle UUID missing, changed or wrong generation')
            counts={label:tile.ask('console execute as @e[tag=E12Vehicle] run say E12_GROUP') for label,tile in cluster.tiles.items()}
            owners={label:sum('E12_GROUP' in line for line in reply.get('output',[])) for label,reply in counts.items()}
            if owners!={label:int(label==target) for label in cluster.tiles}:
                raise RuntimeError(f'vehicle live count differs: {owners}')
            result['trials'].append(dict(index=index,source=source,target=target,generation=record['generation'],vehicle=vehicle,live_counts=owners,load=os.getloadavg()))
            source=target
            if index%25==0:
                module.save(args.name+'.json',result)
                print(f'E12 {args.kind} {index+1}/{args.crossings}',flush=True)
        client.send('vehicle off')
        client.stop()
        client=None
        time.sleep(.2)
        cluster.stop_proxy()
        for tile in cluster.tiles.values():
            tile.stop()
        files={}
        for label,tile in cluster.tiles.items():
            path=tile.folder/'world/players/data'/f'{uuid}.dat'
            if path.exists():
                tag=module.upstream.read_nbt(gzip.decompress(path.read_bytes()))
                files[label]=tag.get('RootVehicle')
        result['saved_before_reconcile']=files
        for tile in cluster.tiles.values():
            tile.start()
        result['saved_after_reconcile']={label:(tile.folder/'world/players/data'/f'{uuid}.dat').exists() for label,tile in cluster.tiles.items()}
        if result['saved_after_reconcile']!={label:label==source for label in cluster.tiles}:
            raise RuntimeError('departed player files survived recovery')
        saved=module.upstream.read_nbt(gzip.decompress((cluster.tiles[source].folder/'world/players/data'/f'{uuid}.dat').read_bytes()))
        result['saved_root']=saved.get('RootVehicle',{}).get('Entity',{}).get('UUID')
        result['saved_entity_copies']={label:saved_entities(module,tile.folder/'world',identity) for label,tile in cluster.tiles.items()}
        if result['saved_root']!=identity or any(result['saved_entity_copies'].values()):
            raise RuntimeError('saved vehicle is missing or duplicated between player and entity files')
        cluster.start_proxy()
        with module.environment(module.env(cluster.root/'vehicle-reconnect')):
            client=module.e2.Child([module.BINS/'e4-client',cluster.port],cluster.root,cluster.root/'vehicle-reconnect.log')
        module.e8.wait_event(client,lambda e:e.get('kind')=='passengers' and e.get('passengers'),timeout=30)
        result['recovered_counts']={label:tile.ask('console execute as @e[tag=E12Vehicle] run say E12_GROUP') for label,tile in cluster.tiles.items()}
        owners={label:sum('E12_GROUP' in line for line in reply.get('output',[])) for label,reply in result['recovered_counts'].items()}
        if owners!={label:int(label==source) for label in cluster.tiles}:
            raise RuntimeError(f'vehicle recovery count differs: {owners}')
        result['status']='complete'
    except BaseException as error:
        result.update(status='failed',error=str(error))
        if client:
            result['client_tail']=client.events[-15:]
            result['client_log_tail']=(cluster.root/'vehicle-client.log').read_text().rpartition('\n')[0].splitlines()[-8:]
            result['entities']=[e for e in client.events if e.get('kind') in ['entity','passengers','teleport']]
            result['source_status']=cluster.tiles['a'].ask('server-status')
            result['vehicle_nbt']=cluster.tiles['a'].ask('console data get entity @e[tag=E12Vehicle,limit=1]')
        result['proxy_tail']=(cluster.root/'proxy.log').read_text().rpartition('\n')[0].splitlines()[-20:]
        raise
    finally:
        try:
            if client:
                client.stop()
        finally:
            module.save(args.name+'.json',result)
