"""E17 loopback packet observers, without modifying the proxy or backend binaries."""
from collections import Counter, defaultdict, deque
import hashlib
import math
import selectors
import socket
import threading
import time
import zlib
import errno


def varint(data, offset=0):
    value=0
    for index in range(5):
        if offset+index>=len(data): return None
        byte=data[offset+index]
        value|=(byte&127)<<(7*index)
        if byte<128: return value,offset+index+1
    raise ValueError('oversized VarInt')


class Decoder:
    """One connection's framing state shared by its two packet directions."""
    def __init__(self, observe, packet_observe=None):
        self.buffers=[bytearray(),bytearray()]
        self.phase='handshake'
        self.name=None
        self.compressed=False
        self.observe=observe
        self.packet_observe=packet_observe

    def feed(self, direction, data, at):
        buffer=self.buffers[direction]
        buffer.extend(data)
        while True:
            header=varint(buffer)
            if header is None: return
            size,offset=header
            if size>8*1024*1024: raise ValueError('packet frame too large')
            if len(buffer)<offset+size: return
            body=bytes(buffer[offset:offset+size])
            wire_size=offset+size
            del buffer[:offset+size]
            if self.compressed:
                expected,start=varint(body)
                body=zlib.decompress(body[start:]) if expected else body[start:]
                if expected and len(body)!=expected: raise ValueError('bad decompressed length')
            packet,start=varint(body)
            if self.phase=='handshake' and direction==0:
                self.phase='status' if body[-1]==1 else 'login'
            elif self.phase=='login':
                if direction==0 and packet==0:
                    size,start=varint(body,start)
                    self.name=body[start:start+size].decode('utf-8')
                elif direction==1 and packet==3:
                    self.compressed=True
                elif direction==0 and packet==3:
                    self.phase='configuration'
            elif self.phase=='configuration' and direction==0 and packet==3:
                self.phase='play'
            elif self.phase=='play':
                if self.packet_observe:
                    self.packet_observe(self.name,direction,wire_size)
                if direction==1 and packet==120:
                    self.phase='configuration'
                # Sample every position packet, not a sample chosen by latency.
                elif direction==0 and packet in (30,31):
                    self.observe(self.name,body,at)


class PacketObserver:
    """One selector thread forwards all observer sockets and timestamps movements.

    Front-read to backend-read includes both observer forwarding queues and TCP
    scheduling. It is a conservative proxy-path measurement for serverbound
    position packets, not all packet types or a subtraction of direct latency.
    """
    def __init__(self):
        self.selector=selectors.DefaultSelector()
        self.done=threading.Event()
        self.thread=threading.Thread(target=self.run,daemon=True)
        self.pending=defaultdict(deque)
        self.histogram=Counter()
        self.front_packets=0
        self.backend_packets=0
        self.matched=0
        self.errors=[]
        self.open=[]
        self.lock=threading.Lock()
        self.network=defaultdict(Counter)
        # Keep observer-created outbound sockets out of Darwin's shared
        # 49152..65535 ephemeral range. Bind owns only transient local sockets;
        # it changes no host setting and skips ports another application uses.
        self.source_ports=(20000,40000)
        self.next_source_port=self.source_ports[0]
        self.outbound_connections=0

    def connect(self,target):
        first,last=self.source_ports
        for _ in range(min(512,last-first)):
            port=self.next_source_port
            self.next_source_port=first+(port-first+1)%(last-first)
            backend=socket.socket()
            backend.settimeout(2)
            backend.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
            try:
                backend.bind(('127.0.0.1',port))
                backend.connect(target)
                self.outbound_connections+=1
                return backend
            except OSError as error:
                backend.close()
                if error.errno not in (errno.EADDRINUSE,errno.EADDRNOTAVAIL): raise
        raise OSError(errno.EADDRNOTAVAIL,'observer source-port pool has no usable tuple')

    def listen(self, target, front=False, label=None):
        server=socket.socket()
        server.bind(('127.0.0.1',0))
        server.listen(1024)
        server.setblocking(False)
        self.selector.register(server,selectors.EVENT_READ,('listen',target,front,label or ('front' if front else 'backend')))
        self.open.append(server)
        return server.getsockname()[1]

    def packet(self,label,name,direction,size):
        kind='reader' if name and name.startswith('E12View') else ('real_or_held' if name and name.startswith('E17Bot') else 'other')
        with self.lock:
            record=self.network[label+':'+kind]
            side='to_backend' if direction==0 else 'from_backend'
            record[side+'_packets']+=1
            record[side+'_wire_bytes']+=size

    def observe(self, front, name, body, at):
        if not name or not name.startswith('E17Bot'): return
        key=(name,hashlib.sha256(body).digest())
        with self.lock:
            if front:
                self.front_packets+=1
                self.pending[key].append(at)
            else:
                self.backend_packets+=1
                queue=self.pending.get(key)
                if queue:
                    elapsed=at-queue.popleft()
                    if not queue: del self.pending[key]
                    if elapsed<0: raise ValueError('backend timestamp precedes frontend')
                    # Round up to microseconds, including the complete tail.
                    self.histogram[math.ceil(elapsed*1e6)]+=1
                    self.matched+=1

    def run(self):
        try:
            while not self.done.is_set():
                for key,mask in self.selector.select(.05):
                    sock=key.fileobj
                    state=key.data
                    if state[0]=='listen':
                        _,target,front,label=state
                        client,_=sock.accept()
                        try:
                            backend=self.connect(target)
                        except OSError as error:
                            # A failed new connection must not close all unrelated
                            # observed sessions. Any error still invalidates latency.
                            self.errors.append(f'connect {label} {target}: {error!r}')
                            client.close()
                            continue
                        for item in [client,backend]:
                            item.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
                            item.setblocking(False)
                            self.open.append(item)
                        decoder=Decoder(lambda name,body,at,front=front:self.observe(front,name,body,at),
                            lambda name,direction,size,label=label:self.packet(label,name,direction,size))
                        left={'peer':backend,'output':bytearray(),'decoder':decoder,'direction':0}
                        right={'peer':client,'output':bytearray(),'decoder':decoder,'direction':1}
                        left['other']=right
                        right['other']=left
                        self.selector.register(client,selectors.EVENT_READ,('stream',left))
                        self.selector.register(backend,selectors.EVENT_READ,('stream',right))
                        continue
                    _,state=state
                    try:
                        if mask&selectors.EVENT_READ:
                            data=sock.recv(262144)
                            at=time.monotonic()
                            if not data:
                                self.close_pair(sock,state)
                                continue
                            state['decoder'].feed(state['direction'],data,at)
                            state['other']['output'].extend(data)
                            self.selector.modify(state['peer'],selectors.EVENT_READ|selectors.EVENT_WRITE,('stream',state['other']))
                        if mask&selectors.EVENT_WRITE and state['output']:
                            size=sock.send(state['output'])
                            del state['output'][:size]
                            if not state['output']:
                                self.selector.modify(sock,selectors.EVENT_READ,('stream',state))
                    except BlockingIOError:
                        continue
                    except (ConnectionError,OSError):
                        self.close_pair(sock,state)
        except BaseException as error:
            self.errors.append(repr(error))
        finally:
            for sock in self.open:
                sock.close()
            self.selector.close()

    def close_pair(self,sock,state):
        for item in [sock,state['peer']]:
            try: self.selector.unregister(item)
            except (KeyError,ValueError): pass
            item.close()

    def start(self):
        self.thread.start()

    def reset(self):
        """Discard setup observations before a fixed-load measurement interval."""
        with self.lock:
            self.pending.clear()
            self.histogram.clear()
            self.network.clear()
            self.front_packets=0
            self.backend_packets=0
            self.matched=0

    def report(self):
        with self.lock:
            def quantile(q):
                rank=math.ceil(q*self.matched)
                total=0
                for micros,count in sorted(self.histogram.items()):
                    total+=count
                    if total>=rank: return micros/1000
                return None
            return {'scope':'serverbound position packets, observer front-read to backend-read',
                'includes':'observer queues, proxy queues and TCP scheduling',
                'front_packets':self.front_packets,'backend_packets':self.backend_packets,
                'matched':self.matched,'unmatched_front':sum(map(len,self.pending.values())),
                'p50_ms':quantile(.5),'p95_ms':quantile(.95),'p99_ms':quantile(.99),
                'maximum_ms':max(self.histogram,default=0)/1000,
                'matched_fraction':self.matched/self.front_packets if self.front_packets else None,
                'observer_source_port_range':list(self.source_ports),
                'outbound_connections':self.outbound_connections,
                'histogram_us':dict(self.histogram),'errors':list(self.errors)}

    def network_report(self):
        with self.lock:
            return {key:dict(value) for key,value in self.network.items()}

    def stop(self):
        self.done.set()
        self.thread.join(5)
        if self.thread.is_alive(): raise RuntimeError('observer thread did not stop')
