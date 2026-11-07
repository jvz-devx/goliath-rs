"""Observe an unmodified offline Horizon client through its control and wire protocols."""

import json
import select
import socket
import struct
import threading
import time
import zlib


def varint(data, offset=0):
    value = 0
    for shift in range(0, 35, 7):
        if offset >= len(data):
            raise EOFError
        byte = data[offset]
        offset += 1
        value |= (byte & 127) << shift
        if not byte & 128:
            return value, offset
    raise ValueError('varint exceeds five bytes')


def attributes(payload):
    target, offset = varint(payload)
    count, offset = varint(payload, offset)
    values = []
    for _ in range(count):
        attribute, offset = varint(payload, offset)
        base = struct.unpack_from('>d', payload, offset)[0]
        offset += 8
        modifiers, offset = varint(payload, offset)
        kept = []
        for _ in range(modifiers):
            length, offset = varint(payload, offset)
            name = payload[offset:offset + length].decode()
            offset += length
            amount = struct.unpack_from('>d', payload, offset)[0]
            offset += 8
            operation, offset = varint(payload, offset)
            kept.append({'id': name, 'amount': amount, 'operation': operation})
        values.append({'attribute': attribute, 'base': base, 'modifiers': kept})
    if offset != len(payload):
        raise ValueError('attribute payload has trailing bytes')
    return target, values


class Control:
    def __init__(self, endpoint):
        self.socket = socket.create_connection(('127.0.0.1', endpoint['port']), timeout=10)
        self.socket.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.reader = self.socket.makefile('rb')
        assert self.ask('auth ' + endpoint['token']).get('ok') is True

    def ask(self, line):
        began = time.time_ns()
        self.socket.sendall((line + '\n').encode())
        raw = self.reader.readline()
        if not raw.endswith(b'\n'):
            raise RuntimeError('control closed before a complete reply')
        result = json.loads(raw)
        result['_sent_ns'] = began
        result['_received_ns'] = time.time_ns()
        return result

    def close(self):
        self.reader.close()
        self.socket.close()


class Sampler:
    def __init__(self, endpoint, command, extract, interval=.002):
        self.endpoint, self.command, self.extract = endpoint, command, extract
        self.interval = interval
        self.rows, self.errors = [], []
        self.done = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        connection = None
        try:
            connection = Control(self.endpoint)
            previous = None
            while not self.done.is_set():
                raw = connection.ask(self.command)
                row = self.extract(raw)
                if row is not None:
                    key = row.pop('_key')
                    if key != previous:
                        row.update(sent_ns=raw['_sent_ns'], ns=raw['_received_ns'])
                        self.rows.append(row)
                        previous = key
                self.done.wait(self.interval)
        except Exception as error:
            if not self.done.is_set():
                self.errors.append(type(error).__name__ + ': ' + str(error))
        finally:
            if connection:
                connection.close()

    def stop(self):
        self.done.set()
        self.thread.join(12)
        if self.thread.is_alive():
            raise RuntimeError('sampler did not stop')


def client_state(raw):
    player = raw.get('player')
    if not player:
        return None
    return {'_key': raw['frames'], 'frame': raw['frames'], 'tick': raw['ticks'],
            'pos': player['position'], 'velocity': player['velocity'],
            'ground': player['on_ground'], 'yaw': raw['yaw'], 'pitch': raw['pitch'],
            'screen': raw['screen'], 'keys': raw['keys'], 'chunks': raw['chunks'],
            'pending_chunks': raw['pending_chunks'], 'streaming': raw['streaming']}


def server_state(raw):
    tick = raw.get('timing', {}).get('tick')
    if tick is None:
        raise RuntimeError('server export has no simulation tick')
    record = raw.get('record')
    return {'_key': (tick, bool(record)), 'tick': tick,
            'pos': record.get('position') if record else None,
            'velocity': record.get('velocity') if record else None,
            'frozen': record.get('handover') is not None if record else False,
            'apply_ms': raw['timing']['apply_ms'], 'queue_ms': raw['timing']['queue_ms']}


class Observer:
    """Forward bytes unchanged, retaining only movement, correction and particle packets.

    IDs and field layouts are protocol 777 from Horizon's pinned ids.rs/play.rs.
    No login profile, UUID, chat or chunk payload is retained.
    """

    def __init__(self, target_port):
        self.target_port = target_port
        self.listener = socket.socket()
        self.listener.bind(('127.0.0.1', 0))
        self.listener.listen(1)
        self.listener.settimeout(.2)
        self.port = self.listener.getsockname()[1]
        self.done = threading.Event()
        self.rows, self.errors = [], []
        self.particles = 0
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        connections = []
        try:
            while not self.done.is_set():
                try:
                    client, _ = self.listener.accept()
                    break
                except socket.timeout:
                    continue
            else:
                return
            backend = socket.create_connection(('127.0.0.1', self.target_port), timeout=10)
            connections = [client, backend]
            for connection in connections:
                connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            buffers = [bytearray(), bytearray()]
            states = ['handshake', 'login']
            compressed = False
            entity = None
            while not self.done.is_set():
                readable, _, _ = select.select(connections, [], [], .1)
                for connection in readable:
                    direction = connections.index(connection)
                    raw = connection.recv(1024 * 1024)
                    if not raw:
                        return
                    received = time.time_ns()
                    # Forward before inspecting; the observer adds no readiness wait.
                    connections[1 - direction].sendall(raw)
                    buffers[direction].extend(raw)
                    buffer = buffers[direction]
                    while buffer:
                        try:
                            size, offset = varint(buffer)
                        except EOFError:
                            break
                        if len(buffer) < offset + size:
                            break
                        body = bytes(buffer[offset:offset + size])
                        del buffer[:offset + size]
                        if compressed:
                            expanded, offset = varint(body)
                            body = zlib.decompress(body[offset:]) if expanded else body[offset:]
                            if expanded and len(body) != expanded:
                                raise ValueError('compressed packet length differs')
                        packet, offset = varint(body)
                        payload = body[offset:]
                        state = states[direction]
                        if direction == 0:
                            if state == 'handshake':
                                states[0] = 'login'
                            elif state == 'login' and packet == 3:
                                states[0] = 'configuration'
                            elif state == 'configuration' and packet == 3:
                                states[0] = 'play'
                            elif state == 'play' and packet in (30, 31):
                                self.rows.append({'kind': 'sent_position', 'ns': received,
                                                  'pos': list(struct.unpack_from('>ddd', payload))})
                            elif state == 'play' and packet == 40:
                                self.rows.append({'kind': 'client_abilities', 'ns': received,
                                                  'flags': payload[0]})
                        elif state == 'login':
                            if packet == 3:
                                compressed = True
                            elif packet == 2:
                                states[1] = 'configuration'
                        elif state == 'configuration' and packet == 3:
                            states[1] = 'play'
                        elif state == 'play':
                            if packet == 50:
                                entity = struct.unpack_from('>i', payload)[0]
                                self.rows.append({'kind': 'login_play', 'ns': received})
                            elif packet == 46:
                                self.rows.append({'kind': 'chunk', 'ns': received,
                                                  'chunk': list(struct.unpack_from('>ii', payload))})
                            elif packet == 38:
                                z, x = struct.unpack_from('>ii', payload)
                                self.rows.append({'kind': 'forget_chunk', 'ns': received, 'chunk': [x, z]})
                            elif packet == 96:
                                x, offset = varint(payload)
                                z, _ = varint(payload, offset)
                                self.rows.append({'kind': 'chunk_center', 'ns': received,
                                                  'chunk': [x if x < 2**31 else x-2**32,
                                                            z if z < 2**31 else z-2**32]})
                            elif packet == 84:
                                self.rows.append({'kind': 'respawn', 'ns': received})
                            elif packet == 134:
                                target, values = attributes(payload)
                                if target == entity:
                                    self.rows.append({'kind': 'attributes', 'ns': received, 'values': values})
                            elif packet == 65:
                                self.rows.append({'kind': 'server_abilities', 'ns': received, 'flags': payload[0],
                                                  'speeds': list(struct.unpack_from('>ff', payload, 1))})
                            elif packet == 101:
                                target, offset = varint(payload)
                                if target == entity and payload[offset] == 0:
                                    kind, offset = varint(payload, offset + 1)
                                    if kind == 0:
                                        self.rows.append({'kind': 'entity_flags', 'ns': received,
                                                          'flags': payload[offset]})
                            elif packet == 73:
                                teleport, offset = varint(payload)
                                values = struct.unpack_from('>ddddddffi', payload, offset)
                                self.rows.append({'kind': 'correction', 'ns': received,
                                                  'pos': list(values[:3]), 'velocity': list(values[3:6]),
                                                  'yaw': values[6], 'pitch': values[7],
                                                  'relatives': values[8], 'teleport': teleport})
                            elif packet == 103:
                                target, offset = varint(payload)
                                if target == entity:
                                    self.rows.append({'kind': 'motion_packet', 'ns': received,
                                                      'payload_hex': payload[offset:].hex()})
                            elif packet == 48:
                                self.particles += 1
                                self.rows.append({'kind': 'particle', 'ns': received})
        except Exception as error:
            if not self.done.is_set():
                self.errors.append(type(error).__name__ + ': ' + str(error))
        finally:
            for connection in connections:
                connection.close()

    def stop(self):
        self.done.set()
        self.thread.join(12)
        self.listener.close()
        if self.thread.is_alive():
            raise RuntimeError('wire observer did not stop')
