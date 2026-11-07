"""Owns E13 process fixtures, TCP fault gates and recovered identity accounting."""
import base64
from collections import Counter
import gzip
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import socket
import struct
import threading
import time
import zlib

ROOT = Path(__file__).resolve().parent.parent
loader = importlib.machinery.SourceFileLoader('e13_cluster', str(ROOT/'scripts/goliath-cluster'))
spec = importlib.util.spec_from_loader(loader.name, loader)
cluster = importlib.util.module_from_spec(spec)
loader.exec_module(cluster)
DATA = ROOT/'docs/experiments/data/E13'


def save(name, value):
    path = DATA/name
    index = DATA/'campaign-trials-index.json'
    archived = index.exists() and name in json.loads(index.read_text())['files']
    if path.exists() or archived:
        raise RuntimeError(f'refusing to overwrite evidence {name}')
    cluster.e8.save(path, value)


def wait(predicate, seconds=15, description='condition'):
    deadline = time.monotonic()+seconds
    while time.monotonic()<deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.005)
    raise RuntimeError('timed out waiting for '+description)


def lines(path):
    """Never parse the partial last line of a live log."""
    return path.read_text().rpartition('\n')[0].splitlines() if path.exists() else []


class Proxy:
    """Loopback-only, one framed store call or line-based control stream per socket.

    A gate holds the exact request before forwarding, or its response before
    delivery. It never invents a successful response. Rules are one-shot.
    """
    def __init__(self, upstream, protocol='store'):
        self.upstream, self.protocol = upstream, protocol
        self.listener = socket.socket()
        self.listener.bind(('127.0.0.1',0))
        self.listener.listen()
        self.listener.settimeout(.1)
        self.address = self.listener.getsockname()
        self.closed = threading.Event()
        self.blocked = False
        self.lock = threading.Lock()
        self.rule = None
        self.threads, self.sockets, self.calls, self.errors = [], [], [], []
        self.thread = threading.Thread(target=self.accept, daemon=True)
        self.thread.start()

    def arm(self, operation, when='before'):
        with self.lock:
            if self.rule:
                raise RuntimeError('unreleased fault gate')
            self.rule = {'operation':operation,'when':when,'reached':threading.Event(),
                         'release':threading.Event(),'message':None,'response':None}
            return self.rule

    def gate(self, operation, when, message, response=None):
        with self.lock:
            rule = self.rule
            if not rule or rule['operation']!=operation or rule['when']!=when or rule['reached'].is_set():
                return
            rule.update(message=message,response=response)
            rule['reached'].set()
        if not rule['release'].wait(25):
            raise RuntimeError('fault gate timed out')

    def release(self):
        with self.lock:
            if self.rule:
                self.rule['release'].set()
                self.rule = None

    def accept(self):
        while not self.closed.is_set():
            try:
                sock,_ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            with self.lock:
                self.sockets.append(sock)
            worker = threading.Thread(target=self.serve,args=(sock,),daemon=True)
            self.threads.append(worker)
            worker.start()

    def serve(self, sock):
        remote = None
        try:
            sock.settimeout(30)
            if self.blocked:
                return
            if self.protocol=='store':
                message = cluster.store.receive(sock)
                operation = message[17]
                self.calls.append({'operation':operation,'ns':time.time_ns()})
                self.gate(operation,'before',message)
                if self.blocked:
                    return
                response = cluster.store.call(self.upstream,message)
                self.gate(operation,'after',message,response)
                sock.sendall(struct.pack('<I',len(response))+response)
            else:
                remote = socket.create_connection(self.upstream,timeout=30)
                with sock.makefile('rb') as source,remote.makefile('rb') as reader:
                    for message in source:
                        operation = message.split(b' ',1)[0].strip().decode()
                        self.gate(operation,'before',message)
                        remote.sendall(message)
                        response = reader.readline()
                        self.gate(operation,'after',message,response)
                        sock.sendall(response)
        except (OSError,ConnectionError,RuntimeError) as error:
            self.errors.append(str(error))
        finally:
            sock.close()
            if remote:
                remote.close()
            with self.lock:
                self.sockets.remove(sock)

    def close(self):
        self.release()
        self.closed.set()
        self.listener.close()
        with self.lock:
            for sock in self.sockets:
                try:
                    sock.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
        self.thread.join(2)
        for thread in self.threads:
            thread.join(1)


def kill(tile):
    if not hasattr(tile,'server'):
        return
    tile.server.process.kill()
    tile.server.process.wait(timeout=10)
    if hasattr(tile,'control'):
        tile.control[0].close()
        tile.control[1].close()


def connect(c, name, serial):
    with cluster.environment(cluster.env(c.root/f'client-private-{name}-{serial}')):
        child = cluster.e2.Child([cluster.BINS/'e4-client',c.port,name],c.root,c.root/f'client-{name}-{serial}.log')
    try:
        cluster.e8.wait_event(child,lambda e:e.get('kind')=='teleport',timeout=15)
        return child
    except BaseException:
        cluster.stop_child(child)
        raise


def tag_counts(tag, names):
    out = Counter()
    if isinstance(tag,list):
        for value in tag:
            out.update(tag_counts(value,names))
    elif isinstance(tag,dict):
        if 'id' in tag and 'count' in tag:
            text = json.dumps(tag)
            for name in names:
                if name in text:
                    out[name]+=tag['count']
            return out
        for value in tag.values():
            out.update(tag_counts(value,names))
    return out


def region_tags(world, kind):
    for path in sorted(world.glob(f'dimensions/minecraft/*/{kind}/*.mca')):
        data = path.read_bytes()
        for index in range(1024):
            at = (int.from_bytes(data[index*4:index*4+4],'big')>>8)*4096
            if not at:
                continue
            size = int.from_bytes(data[at:at+4],'big')
            mode = data[at+4]
            payload = data[at+5:at+4+size]
            if mode==1:
                payload = gzip.decompress(payload)
            elif mode==2:
                payload = zlib.decompress(payload)
            elif mode!=3:
                raise RuntimeError('unsupported Anvil compression')
            yield cluster.upstream.read_nbt(payload)


def audit(c, uuid, names):
    """Check recovered files with every process stopped, plus store escrow.

    Store blobs are entry anchors, not extra copies. Count escrow only when no
    current-generation owner file exists. Report all stale files separately.
    """
    record = c.player(uuid)
    locations, stale = {}, []
    resident = False
    for label,tile in c.tiles.items():
        world = tile.folder/'world'
        for path in sorted((world/'players/data').glob('*.dat')):
            tag = cluster.upstream.read_nbt(gzip.decompress(path.read_bytes()))
            counts = tag_counts(tag.get('Inventory',[]),names)
            generation = tag.get('goliath:generation')
            if path.stem==uuid and (record['owner']!=tile.index or generation!=record['generation']):
                stale.append({'tile':label,'generation':generation,'counts':dict(counts)})
                continue
            locations[f'{label}/player/{path.stem}'] = dict(counts)
            resident |= path.stem==uuid
        for kind in ('region','entities'):
            counts = Counter()
            for tag in region_tags(world,kind):
                # Only stacks, never independent regular world block counts.
                for entry in tag.get('block_entities' if kind=='region' else 'Entities',[]):
                    counts.update(tag_counts(entry,names))
            locations[f'{label}/{kind}'] = dict(counts)
    store_tag = cluster.upstream.read_nbt(base64.b64decode(record['body']['record']['player']))
    entry_counts = dict(tag_counts(store_tag.get('Inventory',[]),names))
    if not resident:
        locations['store/escrow'] = entry_counts
    totals = Counter()
    for counts in locations.values():
        totals.update(counts)
    return {'generation':record['generation'],'owner':record['owner'],
            'locations':locations,'store_anchor':entry_counts,'stale_files':stale,
            'totals':{name:totals[name] for name in names}}


def fixture(c, seed):
    """One real menu take, plus durable named chest and unpickable drop controls."""
    name = f'E13S{seed}'
    uuid = str(cluster.upstream.offline_uuid(name))
    child = connect(c,name,0)
    try:
        child.send('walk setup 3.5 1')
        cluster.e8.wait_event(child,lambda e:e.get('kind')=='reached' and e.get('token')=='setup')
        names = {f'E13-{seed}-carry':7,f'E13-{seed}-chest':11,f'E13-{seed}-drop':13}
        tile = c.tiles['a']
        tile.ask('console setblock 3 -60 2 minecraft:chest')
        for slot,(item,count) in enumerate(list(names.items())[:2]):
            tile.ask(f'console item replace block 3 -60 2 container.{slot} with minecraft:diamond[minecraft:custom_name="{item}"] {count}')
        drop = list(names)[2]
        tile.ask('console summon minecraft:item 12 -58 12 '+
                 '{Item:{id:"minecraft:emerald",count:13,components:{"minecraft:custom_name":"'+drop+'"}},PickupDelay:32767s,Age:-32768s,NoGravity:1b}')
        durable = tile.ask(f'tile-cut {tile.token}')
        if durable.get('ok') is not True:
            raise RuntimeError('fixture baseline cut refused')
        child.send('open 3 -60 2')
        cluster.e8.wait_event(child,lambda e:e.get('kind')=='open')
        time.sleep(.1)
        for slot in [0,54]:
            child.send(f'click {slot}')
            time.sleep(.15)
        child.send('close')
        def taken():
            reply = tile.ask('player-export '+uuid)
            if reply.get('ok') is not True:
                return False
            tag = cluster.upstream.read_nbt(base64.b64decode(reply['record']['player']))
            return tag_counts(tag.get('Inventory',[]),names).get(list(names)[0])==7
        wait(taken,description='named chest take')
        # No post-take cut. A pre-durable crash must restore the chest, not
        # destroy or duplicate its item. Later gates must retain the take.
        return child,name,uuid,names
    except BaseException:
        cluster.stop_child(child)
        raise


def tails(c):
    return {'proxy':lines(c.root/'proxy.log')[-12:],
            'store':lines(c.root/'store.log')[-8:],
            'servers':{label:tile.server.since(0)[-8:] for label,tile in c.tiles.items() if hasattr(tile,'server')}}
