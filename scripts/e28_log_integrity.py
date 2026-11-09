"""Owns E28 single-writer proxy logs, drained tile logs and explicit framing evidence."""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import threading
import time

import e11_support as support
import goliath_platform as platform


def framed(raw,marker=None):
    boundary=raw.rfind(b'\n')+1
    tail=raw[boundary:]
    rows=[];errors=[]
    for number,line in enumerate(raw[:boundary].splitlines(),1):
        if not line.strip():continue
        if marker is not None:
            if marker not in line:continue
            line=line.split(marker,1)[1]
        try:rows.append(json.loads(line))
        except (ValueError,UnicodeDecodeError) as error:
            errors.append(dict(line=number,type=type(error).__name__,message=str(error),
                line_sha256=hashlib.sha256(line).hexdigest()))
    return rows,dict(complete=not tail and not errors,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),
        complete_records=len(rows),invalid_complete_lines=errors,tail_bytes=len(tail),
        tail_sha256=hashlib.sha256(tail).hexdigest() if tail else None,
        interpretation='only newline-terminated records decoded; any undecoded tail or malformed complete line leaves strict evidence unproven')


def read_trace(path):
    opener=gzip.open if path.suffix=='.gz' else open
    with opener(path,'rb') as source:return framed(source.read())


def trace_integrity(folder):
    return {path.name:read_trace(path)[1] for path in sorted((folder/'tiles').glob('*-lease.jsonl*'))}


def phase_events(path,evidence):
    began=time.monotonic();observations=[]
    while True:
        raw=path.read_bytes()
        rows,record=framed(raw,b'E10_PHASE ')
        observations.append(record)
        if record['complete'] or record['invalid_complete_lines'] or time.monotonic()-began>=1:
            break
        time.sleep(.01)
    number=len(list(evidence.glob('phase-integrity-*.json')))
    support.save(evidence/f'phase-integrity-{number:04d}.json',dict(
        source=path.name,complete=record['complete'],snapshots=observations,
        bounded_live_tail_wait_seconds=1,malformed_complete_line_never_ignored=True))
    if not record['complete']:
        target=evidence/f'phase-integrity-{number:04d}-native.log.gz'
        target.write_bytes(gzip.compress(raw,mtime=0))
        raise RuntimeError('phase log framing unproven; retained native bytes and integrity record '+target.name)
    return rows


def proxy_child_class(base):
    class SplitProxyChild(base):
        def __init__(self,command,cwd,log):
            self.split_proxy=Path(platform.original_command(command)[0]).name=='meridian'
            self.stdout_log=log.with_name(log.stem+'-stdout.log').open('x') if self.split_proxy else None
            self.logging_error=None
            try:super().__init__(command,cwd,log)
            except BaseException:
                if self.stdout_log:self.stdout_log.close()
                raise

        def read(self):
            if not self.split_proxy:return super().read()
            try:
                for line in self.process.stdout:
                    self.stdout_log.write(line);self.stdout_log.flush()
                    try:event=json.loads(line)
                    except ValueError:continue
                    self.events.append(event);self.incoming.put(event)
            except BaseException as error:self.logging_error=repr(error)

        def stop(self):
            try:return super().stop()
            finally:
                if self.stdout_log:
                    self.reader.join(5)
                    if self.reader.is_alive():raise RuntimeError('proxy stdout log reader did not drain')
                    self.stdout_log.close()
                    if self.logging_error:raise RuntimeError('proxy stdout logging failed: '+self.logging_error)
    return SplitProxyChild


def tile_server_class(base,evidence):
    class DrainedServer(base):
        def __init__(self,*args,**kwargs):
            self.log_drained=threading.Event()
            self.log_reader_error=None
            super().__init__(*args,**kwargs)

        def _read(self):
            try:return super()._read()
            except BaseException as error:self.log_reader_error=repr(error)
            finally:self.log_drained.set()

        def stop(self):
            forced=False;error=None
            try:
                if self.process.poll() is None:
                    try:
                        self.console('stop')
                        self.process.wait(timeout=180)
                    except (BrokenPipeError,subprocess.TimeoutExpired):
                        forced=True;self.process.kill();self.process.wait()
                if not self.log_drained.wait(5):
                    raise RuntimeError('tile stdout log reader did not drain after exit')
                if self.log_reader_error:raise RuntimeError('tile stdout logging failed: '+self.log_reader_error)
            except BaseException as caught:
                error=repr(caught);raise
            finally:
                if self.log_drained.is_set():self.log.close()
                number=len(list(evidence.glob(f'tile-stop-{self.process.pid}-*.json')))
                support.save(evidence/f'tile-stop-{self.process.pid}-{number:02d}.json',dict(
                    pid=self.process.pid,exit=self.process.poll(),console_stop_before_kill=True,
                    forced_kill=forced,log_drained=self.log_drained.is_set(),error=error,
                    lease_writer_joined=False,
                    interpretation='stdout reader drained before close; detached lease writer completion is separately required by strict trace framing'))
    return DrainedServer


def install_cluster(module,evidence):
    evidence.mkdir(parents=True,exist_ok=True)
    module.e2.Child=proxy_child_class(module.e2.Child)
    # The Linux cgroup wrapper resolves this original class when it launches a tile.
    module._server=tile_server_class(module._server,evidence)
    module.phase_events=lambda path:phase_events(path,evidence)


def path_command(instance,reset=False):
    path=instance.root/'proxy-stdout.log';offset=path.stat().st_size
    instance.proxy.send('e18-path'+(' reset' if reset else ''))
    def reply():
        raw=path.read_bytes()[offset:]
        for line in raw[:raw.rfind(b'\n')+1].splitlines():
            if b'E18_PATH ' in line:return json.loads(line.split(b'E18_PATH ',1)[1])
    return support.wait(reply,5,'proxy path histogram timeout')
