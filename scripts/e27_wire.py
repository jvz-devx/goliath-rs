"""E27's lossless loopback forwarding and per-session movement audit."""
import hashlib
import selectors
import socket
import struct
import time
import zlib

from e17_wire import Decoder, varint
from e18_d9_wire import D9Observer, HostPrelude


class AuditDecoder(Decoder):
    def __init__(self, tap, session):
        super().__init__(lambda *_: None)
        self.tap, self.session = tap, session

    def emit(self, kind, **fields):
        self.tap.events.append(dict(kind=kind, session=self.session['id'],
            label=self.session['label'], name=self.name, ns=time.time_ns(), **fields))

    def feed(self, direction, data, at):
        buffer = self.buffers[direction]
        buffer.extend(data)
        while True:
            header = varint(buffer)
            if header is None:
                return
            size, offset = header
            if size > 8*1024*1024:
                raise ValueError('audit frame cap')
            if len(buffer) < offset+size:
                return
            body = bytes(buffer[offset:offset+size])
            del buffer[:offset+size]
            if self.compressed:
                expected, start = varint(body)
                body = zlib.decompress(body[start:]) if expected else body[start:]
                if expected and len(body) != expected:
                    raise ValueError('audit decompression length')
            packet, start = varint(body)
            if self.phase == 'handshake' and direction == 0:
                self.phase = 'status' if body[-1] == 1 else 'login'
            elif self.phase == 'login':
                if direction == 0 and packet == 0:
                    length, start = varint(body, start)
                    self.name = body[start:start+length].decode()
                    self.session['name'] = self.name
                    if self.name.startswith('E27'):
                        self.emit('login')
                elif direction == 1 and packet == 3:
                    self.compressed = True
                elif direction == 0 and packet == 3:
                    self.phase = 'configuration'
            elif self.phase == 'configuration' and direction == 0 and packet == 3:
                self.phase = 'play'
                if self.name and self.name.startswith('E27'):
                    self.emit('play')
            elif self.phase == 'play':
                if direction == 1 and packet == 120:
                    self.phase = 'configuration'
                    self.emit('configuration_reentry')
                elif direction == 0 and packet in (30, 31) and self.name.startswith('E27'):
                    self.emit('move', position=list(struct.unpack_from('>ddd', body, start)),
                        payload_sha256=hashlib.sha256(body).hexdigest(), at=at)


class AuditTap(D9Observer):
    def __init__(self):
        super().__init__([])
        self.events, self.sessions, self.kill_names = [], [], set()

    def close_pair(self, sock, state, reason='eof'):
        session = state['session']
        if not session.get('closed'):
            session.update(closed=True, close_ns=time.time_ns(), close_reason=reason)
            if session.get('name', '').startswith('E27'):
                self.events.append(dict(kind='close', name=session['name'],
                    label=session['label'], session=session['id'], ns=session['close_ns'], reason=reason))
        super().close_pair(sock, state)

    def run(self):
        try:
            while not self.done.is_set():
                for key, mask in self.selector.select(.02):
                    sock, state = key.fileobj, key.data
                    if state[0] == 'listen':
                        _, target, front, label = state
                        client, _ = sock.accept()
                        backend = self.connect(target)
                        for item in (client, backend):
                            item.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                            item.setblocking(False)
                            self.open.append(item)
                        session = dict(id=len(self.sessions), label=label,
                            accepted_ns=time.time_ns(), name=None)
                        self.sessions.append(session)
                        decoder = AuditDecoder(self, session)
                        host = self.hosts.get(sock.getsockname()[1])
                        left = dict(peer=backend, output=bytearray(), decoder=decoder,
                            direction=0, prelude=HostPrelude(host) if host else None, session=session)
                        right = dict(peer=client, output=bytearray(), decoder=decoder,
                            direction=1, prelude=None, session=session)
                        left['other'], right['other'] = right, left
                        self.selector.register(client, selectors.EVENT_READ, ('stream', left))
                        self.selector.register(backend, selectors.EVENT_READ, ('stream', right))
                        continue
                    _, state = state
                    if state['session'].get('closed'):
                        continue
                    try:
                        name = state['session'].get('name')
                        if state['session']['label'] == 'front' and name in self.kill_names:
                            self.kill_names.remove(name)
                            self.close_pair(sock, state, 'planted_connection_kill')
                            continue
                        if mask & selectors.EVENT_READ:
                            data = sock.recv(262144)
                            if not data:
                                self.close_pair(sock, state)
                                continue
                            at = time.monotonic()
                            if state['prelude']:
                                data = state['prelude'].feed(data)
                                if data is None:
                                    continue
                            state['decoder'].feed(state['direction'], data, at)
                            state['other']['output'].extend(data)
                            self.maximum_pending_bytes = max(self.maximum_pending_bytes,
                                len(state['other']['output']))
                            if len(state['other']['output']) > 8*1024*1024:
                                raise ValueError('audit forwarding cap')
                            self.selector.modify(state['peer'], selectors.EVENT_READ | selectors.EVENT_WRITE,
                                ('stream', state['other']))
                        if mask & selectors.EVENT_WRITE and state['output']:
                            size = sock.send(state['output'])
                            del state['output'][:size]
                            if not state['output']:
                                self.selector.modify(sock, selectors.EVENT_READ, ('stream', state))
                    except BlockingIOError:
                        continue
                    except (ConnectionError, OSError) as error:
                        self.close_pair(sock, state, type(error).__name__)
        except BaseException as error:
            self.errors.append(repr(error))
        finally:
            for sock in self.open:
                sock.close()
            self.selector.close()
