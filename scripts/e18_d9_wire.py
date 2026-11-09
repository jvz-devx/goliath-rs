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
        header=varint(self.buffer)
        if header is None:
            if len(self.buffer)>8192:raise ValueError('initial handshake buffer cap')
            return None
        size,offset=header
        if size>4096: raise ValueError('initial handshake frame cap')
        if len(self.buffer)<offset+size:
            if len(self.buffer)>8192:raise ValueError('initial handshake buffer cap')
            return None
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
    def __init__(self,witnesses,forward_only=False):
        witnesses=set(witnesses)
        if forward_only and witnesses:raise ValueError('forward-only mode cannot decode witnesses')
        super().__init__();self.witnesses=witnesses;self.hosts={};self.positions={}
        self.forward_only=forward_only
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
        result.update(witnesses=sorted(self.witnesses),forward_only=self.forward_only,
            decoding_scope='none; handshake host rewrite only' if self.forward_only else 'preselected eight only; all streams traverse the same forwarding path')
        result.update(forwarded_bytes=self.forwarded_bytes.copy(),maximum_pending_bytes=self.maximum_pending_bytes)
        return result
    def stream_interest(self,sock,state):
        events=(selectors.EVENT_READ if state['read_open'] else 0)|(selectors.EVENT_WRITE if state['output'] else 0)
        try:
            if events:self.selector.modify(sock,events,('stream',state))
            else:self.selector.unregister(sock)
        except KeyError:
            if events:self.selector.register(sock,events,('stream',state))
    def finish_write(self,sock,state):
        if state['end_write'] and not state['output'] and state['write_open']:
            sock.shutdown(socket.SHUT_WR);state['write_open']=False
    def close_drained(self,sock,state):
        if not state['read_open'] and not state['other']['read_open'] and not state['output'] and not state['other']['output']:
            self.close_pair(sock,state)
    def close_pair(self,sock,state):
        state['closed']=True;state['other']['closed']=True
        super().close_pair(sock,state)
    def forward_eof(self,sock,state):
        if state['prelude'] and not state['prelude'].finished and state['prelude'].buffer:
            raise ValueError('truncated initial handshake')
        state['read_open']=False;state['other']['end_write']=True
        self.finish_write(state['peer'],state['other'])
        self.stream_interest(sock,state)
        self.close_drained(sock,state)
    def flush_output(self,sock,state):
        size=sock.send(state['output'])
        if not size:raise ConnectionError('forwarding socket returned a zero-byte write')
        del state['output'][:size]
        self.finish_write(sock,state)
        self.stream_interest(sock,state)
        self.close_drained(sock,state)
    def queue_forwarded(self,state,data):
        pending=state['other']['output']
        if len(pending)+len(data)>8*1024*1024:raise ValueError('benchmark shim output cap exceeded')
        was_empty=not pending
        remaining=data
        if self.forward_only and was_empty:
            try:
                size=state['peer'].send(data)
                if not size:raise ConnectionError('forwarding socket returned a zero-byte write')
            except BlockingIOError:size=0
            remaining=data[size:]
        pending.extend(remaining)
        with self.lock:
            self.forwarded_bytes[state['direction']]+=len(data)
            self.maximum_pending_bytes=max(self.maximum_pending_bytes,len(pending))
        if pending and (was_empty or not self.forward_only):self.stream_interest(state['peer'],state['other'])
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
                        decoder=None if self.forward_only else WitnessDecoder(self.witnesses,lambda name,body,at,front=front:self.observe(front,name,body,at),
                            lambda name,direction,size,label=label:self.packet(label,name,direction,size))
                        host=self.hosts.get(sock.getsockname()[1])
                        left=dict(peer=backend,output=bytearray(),decoder=decoder,direction=0,prelude=HostPrelude(host) if host else None)
                        right=dict(peer=client,output=bytearray(),decoder=decoder,direction=1,prelude=None)
                        left['other']=right;right['other']=left
                        for stream in [left,right]:stream.update(read_open=True,write_open=True,end_write=False,closed=False)
                        self.selector.register(client,selectors.EVENT_READ,('stream',left));self.selector.register(backend,selectors.EVENT_READ,('stream',right));continue
                    _,state=state
                    if state['closed']:continue
                    try:
                        if mask&selectors.EVENT_READ:
                            data=sock.recv(262144);at=time.monotonic()
                            if not data:
                                if self.forward_only:self.forward_eof(sock,state)
                                else:self.close_pair(sock,state)
                                continue
                            if state['prelude']:
                                data=state['prelude'].feed(data)
                                if data is None:continue
                                if state['prelude'].finished:state['prelude']=None
                            if state['decoder'] is not None:state['decoder'].feed(state['direction'],data,at)
                            self.queue_forwarded(state,data)
                        if mask&selectors.EVENT_WRITE and state['output']:
                            self.flush_output(sock,state)
                    except BlockingIOError:continue
                    except (ConnectionError,OSError):self.close_pair(sock,state)
                    except ValueError as error:self.errors.append(str(error));self.close_pair(sock,state)
        except BaseException as error:self.errors.append(repr(error))
        finally:
            for sock in self.open:sock.close()
            self.selector.close()
