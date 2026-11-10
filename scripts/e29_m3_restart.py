"""Directed store-crash lease observations, separate from the seeded M3 campaign."""
import csv
import json
import os
from pathlib import Path
import struct
import time

import goliath_platform as platform

LEASE_MS = 1000
BUSY = '010507'


def restart_command(command, address):
    result = list(command)
    result[result.index('--listen')+1] = f'{address[0]}:{address[1]}'
    return result


def claim_result(response):
    if response.hex() == BUSY:
        return dict(busy=True)
    if response[:2] != b'\x01\x08' or len(response) != 42:
        raise RuntimeError('unexpected claim response '+response.hex())
    tile, process, epoch, lease_ms, counter = struct.unpack_from('<QQQQQ',response,2)
    return dict(busy=False,tile=tile,process=process,epoch=epoch,lease_ms=lease_ms,counter=counter)


def establish_player(cluster, client, uuid, serial, chat):
    """Login teleport is not proof that the first cut registered a store record."""
    began=time.time_ns()
    client.send(f'walk before{serial} -1 1')
    chat(client,'E4 wall arrived b',began,20)
    return cluster.player(uuid)


def run(cluster, seed, proxies, result):
    """Restart with all owners alive, then restart with one owner intentionally killed."""
    import m3_chaos_support as support
    wire = support.cluster.store
    clients = []
    actors = {}
    audit_counter = [0]
    intentional_owner_kill = [False]
    old_pids = {name:tile.server.process.pid for name,tile in cluster.tiles.items()}
    old_fences = {name:dict(tile.fence) for name,tile in cluster.tiles.items()}
    result.update(case='store-restart',seed=seed,separate_from_1000_seed_count=True,
                  fixture='original low-population four-tile M3 cluster with four moving real clients',
                  lease_ms=LEASE_MS,observations=[],restarts=[],claims=[],client_exits=[])
    trace = Path(os.environ['GOLIATH_STORE_TRACE'])
    trace_bases = [trace]

    def observe(labels, phase):
        points=[(-8,8),(-8,-8),(8,-8),(8,8)]
        for serial,client in enumerate(clients):
            if client.process.poll() is not None:
                result['client_exits'].append(dict(serial=serial,phase=phase,exit=client.process.returncode))
                if not intentional_owner_kill[0]:
                    raise RuntimeError('client exited before the intentional tile kill')
                continue
            x,z=points[(audit_counter[0]//8+serial)%4]
            try:
                client.send(f'walk load{audit_counter[0]} {x} {z}')
            except BrokenPipeError:
                result['client_exits'].append(dict(serial=serial,phase=phase,exit=client.process.poll()))
                if not intentional_owner_kill[0]:
                    raise
        audit_counter[0]+=1
        before={name:cluster.player(uuid) for name,uuid in actors.items()}
        statuses = {}
        for label in labels:
            tile = cluster.tiles[label]
            authority = tile.ask('cluster-authority')
            status = tile.ask('server-status')
            statuses[label] = dict(authorized=authority.get('authorized'),fence=[authority.get(k) for k in ['tile','process','epoch']],
                                  tick=status['tick'],tps=status['tps'],chunks=status['chunks'],
                                  players=[{k:p.get(k) for k in ['name','placed','held','frozen','released','terrain_reader']} for p in status['players']])
            if authority.get('authorized') is not True:
                raise RuntimeError('tile lost authority in '+phase+': '+label)
            if tile.server.process.pid != old_pids[label] or tile.server.process.poll() is not None:
                raise RuntimeError('surviving tile process changed: '+label)
        after={name:cluster.player(uuid) for name,uuid in actors.items()}
        unstable=[];violations=[]
        for name in actors:
            if (before[name]['generation'],before[name]['owner']) != (after[name]['generation'],after[name]['owner']):
                unstable.append(name)
                continue
            locations=[label for label,status in statuses.items() for player in status['players']
                       if player['name']==name and player['placed'] and not any(player.get(k) for k in ['held','frozen','released','terrain_reader'])]
            if len(locations)>1 or any(cluster.tiles[label].index!=after[name]['owner'] for label in locations):
                violations.append(dict(name=name,locations=locations,store_owner=after[name]['owner']))
        result['observations'].append(dict(monotonic_ns=time.monotonic_ns(),wall_ns=time.time_ns(),phase=phase,tiles=statuses,
            crossing_during_query=unstable,owner_violations=violations,
            store_owners={name:dict(generation=row['generation'],owner=row['owner']) for name,row in after.items()}))
        if violations:
            raise RuntimeError('owner audit violation during '+phase)

    def restart(number, killed_owner=None):
        before = time.monotonic_ns()
        cluster.store_process.kill()
        cluster.store_process.wait(timeout=10)
        killed_at = time.monotonic_ns()
        if killed_owner:
            support.kill(cluster.tiles[killed_owner])
        offset = (cluster.root/'store.log').stat().st_size
        destination = trace.with_name(f'store-restart-{number}.csv')
        trace_bases.append(destination)
        command = restart_command(cluster.store_command,cluster.address)
        spawn_before = time.monotonic_ns()
        cluster.store_process = support.cluster.subprocess.Popen(platform.launch_command(command,'servers'),
            stdout=cluster.store_log,stderr=support.cluster.subprocess.STDOUT,
            env=dict(os.environ,GOLIATH_STORE_TRACE=str(destination)))
        def listening():
            with (cluster.root/'store.log').open('rb') as stream:
                stream.seek(offset)
                return b'LISTEN ' in stream.read()
        support.wait(listening,5,'restarted store listener')
        listened = time.monotonic_ns()
        record = dict(number=number,kill_begin_ns=before,killed_ns=killed_at,
                      spawn_before_ns=spawn_before,listen_observed_ns=listened,
                      recovery_end_bracket_ns=[spawn_before,listened],
                      downtime_upper_ms=(listened-before)/1e6,killed_owner=killed_owner,
                      same_store_directory=True,same_listen_address=True,trace_base=destination.name)
        result['restarts'].append(record)
        return record

    def claim(tile, phase):
        sent = time.monotonic_ns()
        response = wire.call(cluster.address,wire.request(15,wire.u64(tile)+wire.u64(LEASE_MS)))
        row = dict(phase=phase,sent_ns=sent,received_ns=time.monotonic_ns(),response_hex=response.hex(),**claim_result(response))
        result['claims'].append(row)
        return row

    try:
        for serial in range(4):
            name=f'E29R{seed}_{serial}'
            client = support.connect(cluster,name,serial)
            clients.append(client)
            actors[name]=str(support.cluster.upstream.offline_uuid(name))
            establish_player(cluster,client,actors[name],serial,support.cluster.e8.chat)
        for _ in range(10):
            observe('abcd','before-first-crash')
            time.sleep(.2)
        restart(1)
        # These are replacement attempts, not renewals by the existing owners.
        for index in range(4):
            row = claim(index,'all-owners-alive')
            if not row['busy']:
                raise RuntimeError('replacement granted while its live owner remains')
        for _ in range(15):
            observe('abcd','after-first-crash')
            time.sleep(.2)
        intentional_owner_kill[0]=True
        second = restart(2,'a')
        end = second['listen_observed_ns']+LEASE_MS*1_000_000
        granted = None
        while time.monotonic_ns() < second['listen_observed_ns']+5_000_000_000:
            row = claim(0,'killed-owner-replacement')
            if not row['busy']:
                granted = row
                break
            observe('bcd','replacement-quarantine')
            time.sleep(.025)
        if granted is None:
            raise RuntimeError('replacement never granted')
        # A success before even the beginning of recovery plus L is a violation.
        # Report the stronger listen-relative margin too; do not hide startup uncertainty.
        if granted['sent_ns'] < second['spawn_before_ns']+LEASE_MS*1_000_000:
            raise RuntimeError('replacement granted before a full recovery lease')
        if granted['process'] == old_fences['a']['process'] or granted['epoch'] <= old_fences['a']['epoch']:
            raise RuntimeError('replacement did not get a fresh fence')
        if granted['lease_ms'] != LEASE_MS or granted['counter'] != 0:
            raise RuntimeError('replacement got an unexpected lease or counter')
        result.update(first_replacement_after_spawn_ms=(granted['sent_ns']-second['spawn_before_ns'])/1e6,
                      first_replacement_after_listener_ms=(granted['sent_ns']-second['listen_observed_ns'])/1e6,
                      recovery_bracket_width_ms=(second['listen_observed_ns']-second['spawn_before_ns'])/1e6,
                      no_observed_replacement_before_full_recovery_lease=True,
                      listener_relative_full_lease_observed=granted['sent_ns']>=end,
                      old_owner_dead_before_replacement=cluster.tiles['a'].server.process.poll() is not None,
                      replacement_fence={k:granted[k] for k in ['tile','process','epoch','lease_ms','counter']})
        for _ in range(10):
            observe('bcd','after-replacement')
            time.sleep(.2)
        renewal_paths = {}
        for base in trace_bases:
            path = base.with_suffix('.renewals.csv')
            with path.open() as stream:
                rows = list(csv.DictReader(stream))
            renewal_paths[path.name] = dict(rows=len(rows),first=rows[0] if rows else None,last=rows[-1] if rows else None)
        result['early_renewal_traces'] = renewal_paths
        result['all_three_stores_answered_early_renewals'] = all(row['rows']>0 for row in renewal_paths.values())
        if not result['all_three_stores_answered_early_renewals']:
            raise RuntimeError('a crash phase did not exercise early renewals')
        result['status'] = 'complete'
        result['limits'] = ['Finite control samples and replacement attempts, not continuous owner auditing.',
                            'Recovery completion lies between spawn and listener observation; both clock-relative grant delays are reported.',
                            'Four-client M3 fault fixture, not the 425-player soak or a power-cut durability test.',
                            'The killed tile is intentional; the other three keep their processes and authorization.']
        return result
    finally:
        for client in clients:
            support.cluster.stop_child(client)
