"""D9 benchmark tap: identical TCP hops, startup virtual-host routing, seeded witness decoding."""
import selectors
import socket
import struct
import time
from e17_wire import Decoder, PacketObserver, varint


def vint(value):
    output=bytearray()
    while value>=128:
        output.append((value&127)|128);value>>=7
    output.append(value)
    return bytes(output)


class HostPrelude:
    """Rewrite only a complete, uncompressed initial handshake, preserving its tail."""
    def __init__(self,host): self.host=host.encode();self.buffer=bytearray();self.finished=False
    def feed(self,data):
        if self.finished: return data
        self.buffer.extend(data)
        if len(self.buffer)>8192: raise ValueError('initial handshake buffer cap')
        header=varint(self.buffer)
        if header is None: return None
        size,offset=header
        if size>4096: raise ValueError('initial handshake frame cap')
        if len(self.buffer)<offset+size: return None
        body=bytes(self.buffer[offset:offset+size]);packet,start=varint(body)
        if packet!=0: raise ValueError('first packet is not a handshake')
        _,length_at=varint(body,start);old_length,host_at=varint(body,length_at)
        if host_at+old_length+3>len(body): raise ValueError('truncated handshake fields')
        rewritten=body[:length_at]+vint(len(self.host))+self.host+body[host_at+old_length:]
        result=vint(len(rewritten))+rewritten+bytes(self.buffer[offset+size:])
        self.buffer.clear();self.finished=True
        return result


class WitnessDecoder(Decoder):
    def __init__(self,witnesses,*args): super().__init__(*args);self.witnesses=witnesses
    def feed(self,direction,data,at):
        if self.name is None or self.name in self.witnesses: super().feed(direction,data,at)


class D9Observer(PacketObserver):
    def __init__(self,witnesses):
        super().__init__();self.witnesses=set(witnesses);self.hosts={};self.positions={}
        self.forwarded_bytes=[0,0];self.maximum_pending_bytes=0
    def listen(self,target,front=False,label=None,host=None):
        port=super().listen(target,front=front,label=label)
        if host:self.hosts[port]=host
        return port
    def reset(self):
        super().reset()
        with self.lock:self.positions.clear()
    def observe(self,front,name,body,at):
        if front and name in self.witnesses:
            with self.lock:
                if name not in self.positions:
                    _,offset=varint(body);self.positions[name]=dict(position=struct.unpack_from('>ddd',body,offset),at=at)
        super().observe(front,name,body,at)
    def report(self):
        result=super().report()
        with self.lock:result['witness_positions']=dict(self.positions)
        result.update(witnesses=sorted(self.witnesses),decoding_scope='preselected eight only; all streams traverse the same forwarding path')
        result.update(forwarded_bytes=self.forwarded_bytes.copy(),maximum_pending_bytes=self.maximum_pending_bytes)
        return result
    def run(self):
        try:
            while not self.done.is_set():
                for key,mask in self.selector.select(.05):
                    sock=key.fileobj;state=key.data
                    if state[0]=='listen':
                        _,target,front,label=state;client,_=sock.accept()
                        try: backend=self.connect(target)
                        except OSError as error:
                            self.errors.append(f'connect {label}: {error!r}');client.close();continue
                        for item in [client,backend]:
                            item.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1);item.setblocking(False);self.open.append(item)
                        decoder=WitnessDecoder(self.witnesses,lambda name,body,at,front=front:self.observe(front,name,body,at),
                            lambda name,direction,size,label=label:self.packet(label,name,direction,size))
                        host=self.hosts.get(sock.getsockname()[1])
                        left=dict(peer=backend,output=bytearray(),decoder=decoder,direction=0,prelude=HostPrelude(host) if host else None)
                        right=dict(peer=client,output=bytearray(),decoder=decoder,direction=1,prelude=None)
                        left['other']=right;right['other']=left
                        self.selector.register(client,selectors.EVENT_READ,('stream',left));self.selector.register(backend,selectors.EVENT_READ,('stream',right));continue
                    _,state=state
                    try:
                        if mask&selectors.EVENT_READ:
                            data=sock.recv(262144);at=time.monotonic()
                            if not data:self.close_pair(sock,state);continue
                            if state['prelude']:
                                data=state['prelude'].feed(data)
                                if data is None:continue
                            state['decoder'].feed(state['direction'],data,at);state['other']['output'].extend(data)
                            with self.lock:
                                self.forwarded_bytes[state['direction']]+=len(data)
                                self.maximum_pending_bytes=max(self.maximum_pending_bytes,len(state['other']['output']))
                            if len(state['other']['output'])>8*1024*1024:raise ValueError('benchmark shim output cap exceeded')
                            self.selector.modify(state['peer'],selectors.EVENT_READ|selectors.EVENT_WRITE,('stream',state['other']))
                        if mask&selectors.EVENT_WRITE and state['output']:
                            size=sock.send(state['output']);del state['output'][:size]
                            if not state['output']:self.selector.modify(sock,selectors.EVENT_READ,('stream',state))
                    except BlockingIOError:continue
                    except (ConnectionError,OSError):self.close_pair(sock,state)
                    except ValueError as error:self.errors.append(str(error));self.close_pair(sock,state)
        except BaseException as error:self.errors.append(repr(error))
        finally:
            for sock in self.open:sock.close()
            self.selector.close()
