"""Lazy-reader interior, hysteresis and fast-approach packet checks."""
import os
import time


def lifecycle(module, cluster, args):
    fixtures = __import__('e12_fixtures')
    result = dict(status='running', stages=[], load_start=os.getloadavg())
    client = None
    try:
        with module.environment(module.env(cluster.root/'lifecycle-private')):
            client = module.e2.Child([module.BINS/'e4-client', cluster.port], cluster.root, cluster.root/'lifecycle-client.log')
        module.e8.wait_event(client, lambda event: event.get('kind') == 'teleport')
        def readers():
            return {label: [p for p in tile.ask('server-status')['players']
                if p.get('terrain_reader') or p['name'].startswith('E12View')]
                for label, tile in cluster.tiles.items()}
        def place(position, expected, token):
            began = time.time_ns()
            client.send('hold on')
            module.e8.wait_event(client, lambda e: e.get('kind') == 'hold' and e['active'], began)
            cluster.tiles['a'].ask('console tp E4Probe '+' '.join(str(float(v)) for v in position))
            module.e8.wait_event(client, lambda e: e.get('kind') == 'teleport' and e['pos'] == position, began)
            deadline = time.monotonic()+30
            while True:
                observed = fixtures.audit(module, client, token)
                live = readers()
                if not observed['missing'] and sum(map(len, live.values())) == expected:
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError('reader lifecycle did not converge: '+token)
                time.sleep(.05)
            stage = dict(token=token, position=position, readers=live, audit=observed, load=os.getloadavg())
            result['stages'].append(stage)
            module.save(args.name+'.json', result)
            return stage
        place([256,-60,256], 0, 'interior-no-readers')
        opened = place([70,-60,70], 4, 'corner-preload')
        kept = place([100,-60,70], 4, 'corner-hysteresis')
        if {p['uuid'] for rows in opened['readers'].values() for p in rows} != {p['uuid'] for rows in kept['readers'].values() for p in rows}:
            raise RuntimeError('hysteresis replaced a live reader')
        place([130,-60,130], 0, 'retired-no-readers')
        # Fly above E10's intentional world-marker block at 8,8. A ground
        # diagonal would exercise collision with that block, not wall terrain.
        place([130,-50,130], 0, 'flight-height')
        client.send('fly on')
        for index, destination in enumerate([[-6,-6],[6,6]]):
            began = time.time_ns()
            client.send('pace 32')
            module.e8.wait_event(client, lambda e: e.get('kind') == 'pace' and e['speed'] == 32, began)
            fixtures.audit(module, client, 'fast-start-'+str(index), True)
            client.send('hold off')
            client.send(f'walk fast-{index} {destination[0]} {destination[1]}')
            module.e8.wait_event(client, lambda e: e.get('kind') == 'reached' and e.get('token') == 'fast-'+str(index), began, 45)
            observed = fixtures.audit(module, client, 'fast-'+str(index))
            result['stages'].append(dict(token='fast-'+str(index), speed=32, audit=observed))
            if observed['missing'] or observed['missing_frames'] or observed['visible_reloads']:
                raise RuntimeError('blank or reloaded columns on fast corner approach')
        result['status'] = 'complete'
    except BaseException as error:
        result.update(status='failed', error=str(error))
        if client: result['client_tail'] = client.events[-10:]
        raise
    finally:
        if client: client.stop()
        result['load_end'] = os.getloadavg()
        module.save(args.name+'.json', result)
