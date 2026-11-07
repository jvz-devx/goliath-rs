"""E11 private process, control, CPU-budget and result-writing helpers."""
import ctypes
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parent.parent
RUNS = ROOT / '.local/runs'
DATA = ROOT / 'docs/experiments/data/E11'
BINS = ROOT / '.local/bin/horizon-e11'
RESOURCES = Path.home() / 'Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3'


def sanitize(value):
    return str(value).replace(str(ROOT), '$REPO').replace(str(Path.home()), '$HOME')


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix+'.tmp')
    temp.write_text(sanitize(json.dumps(value, indent=2))+'\n')
    temp.replace(path)


def complete_json_lines(path):
    """A live writer may have stopped halfway through the last JSON line."""
    if not Path(path).exists():
        return []
    raw = Path(path).read_bytes()
    end = raw.rfind(b'\n')
    if end < 0:
        return []
    rows = []
    for line in raw[:end].splitlines():
        try:
            rows.append(json.loads(line))
        except (ValueError, UnicodeDecodeError):
            pass
    return rows


def wait(predicate, seconds=120, message='timeout'):
    until = time.monotonic()+seconds
    while True:
        value = predicate()
        if value:
            return value
        if time.monotonic() >= until:
            raise RuntimeError(message)
        time.sleep(.05)


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def private_env(folder):
    env = {k:v for k,v in os.environ.items() if not k.startswith(('HORIZON_', 'XDG_', 'E2_', 'E8_'))}
    for key in ['HORIZON_HOME','HORIZON_LAUNCHER_HOME','XDG_DATA_HOME','XDG_CONFIG_HOME',
                'XDG_CACHE_HOME','XDG_RUNTIME_DIR','HORIZON_LAUNCHER_TRASH',
                'HORIZON_LAUNCHER_CLIPBOARD','HORIZON_LAUNCHER_DESKTOP']:
        path = folder/key.lower()
        path.mkdir(parents=True, exist_ok=True)
        env[key] = str(path)
    env.update(HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_AUDIO='1')
    return env


class Control:
    def __init__(self, endpoint):
        info = json.loads(Path(endpoint).read_text())
        self.socket = socket.create_connection(('127.0.0.1', info['port']), timeout=120)
        self.stream = self.socket.makefile('r')
        if not self.ask('auth '+info['token']).get('ok'):
            raise RuntimeError('control auth failed')

    def ask(self, text):
        self.socket.sendall((text+'\n').encode())
        raw = self.stream.readline()
        if not raw:
            raise RuntimeError('control endpoint closed')
        value = json.loads(raw)
        if 'error' in value:
            raise RuntimeError(f'{text}: {value}')
        return value

    def console(self, text):
        return self.ask('console '+text)

    def close(self):
        self.stream.close()
        self.socket.close()


class Engine(Control):
    def __init__(self, folder, config, index=0):
        self.folder = folder
        self.game = folder/'game'
        self.game.mkdir(parents=True)
        self.port = free_port()
        self.endpoint = self.game/'.horizon/engine.json'
        self.endpoint.parent.mkdir()
        properties = {'online-mode':'false','white-list':'false','server-ip':'127.0.0.1',
            'server-port':self.port,'level-name':'world','level-type':'minecraft\\:flat',
            'level-seed':config.get('seed',12345),'generate-structures':'false','gamemode':'creative',
            'force-gamemode':'true','allow-flight':'true','spawn-protection':0,
            'view-distance':config.get('view_distance',3),'simulation-distance':config.get('view_distance',3),
            'max-players':10000,'pause-when-empty-seconds':0,'max-tick-time':0,
            'generator-settings':json.dumps({'layers':[{'block':'minecraft:bedrock','height':1},
                {'block':'minecraft:dirt','height':2},{'block':'minecraft:grass_block','height':1}],
                'biome':'minecraft:plains','features':False,'lakes':False,'structure_overrides':[]})}
        spec = config.get('servers',[{}])[index % len(config.get('servers',[{}]))]
        properties.update(spec.get('properties',{}))
        (self.game/'server.properties').write_text(''.join(f'{k}={v}\n' for k,v in properties.items()))
        (self.game/'eula.txt').write_text('eula=true\n')
        self.env = private_env(folder)
        self.env.update(HORIZON_E11_MEASURE='1')
        self.env.update(spec.get('env',{}))
        command = spec.get('command',[str(BINS/'horizon-client'),'--resources',str(RESOURCES),
            '--control-socket','{endpoint}','--dedicated','--nogui'])
        command = [word.replace('{game}',str(self.game)).replace('{endpoint}',str(self.endpoint))
            .replace('{port}',str(self.port)).replace('{resources}',str(RESOURCES)).replace('{root}',str(ROOT)) for word in command]
        self.lines=[]
        self.log=(folder/'engine.log').open('w')
        self.process=subprocess.Popen(command,cwd=self.game,env=self.env,stdin=subprocess.PIPE,
            stdout=self.log,stderr=subprocess.STDOUT,text=True)
        self.pid=self.process.pid
        self.socket=None
        try:
            def ready():
                if self.process.poll() is not None:
                    raise RuntimeError('engine exited: '+sanitize((folder/'engine.log').read_text()[-3000:]))
                # Startup marker is accepted only in a complete line.
                raw=(folder/'engine.log').read_bytes()
                return b']: Done (' in raw[:raw.rfind(b'\n')+1]
            wait(ready,240,'engine startup timed out')
            super().__init__(self.endpoint)
        except BaseException:
            self.stop()
            raise

    def stop(self):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGCONT)
            try:
                self.process.stdin.write('stop\n')
                self.process.stdin.flush()
                self.process.wait(60)
            except (BrokenPipeError,subprocess.TimeoutExpired):
                self.process.kill()
                self.process.wait()
        if self.socket:
            self.close()
        self.log.close()

    def console_many(self,commands,parallelism=8):
        """Independent setup commands, on separate authenticated control streams.

        The game still executes commands serially. Multiple queued requests avoid
        paying a whole control-poll interval for every unrelated bot placement.
        Never use this for ordered fixture actions or measured queries.
        """
        from concurrent.futures import ThreadPoolExecutor
        commands=list(commands)
        if not commands: return
        def worker(part):
            control=Control(self.endpoint)
            try:
                for command in part: control.console(command)
            finally:
                control.close()
        parts=[commands[i::parallelism] for i in range(min(parallelism,len(commands)))]
        with ThreadPoolExecutor(max_workers=len(parts)) as pool:
            for result in pool.map(worker,parts): pass


class TaskInfo(ctypes.Structure):
    # Darwin sys/proc_info.h struct proc_taskinfo, PROC_PIDTASKINFO = 4.
    _fields_ = [(n,ctypes.c_uint64) for n in ['virtual','rss','user','system','threads_user','threads_system']] + [
        (n,ctypes.c_int32) for n in ['policy','faults','pageins','cow','messages_sent','messages_received',
            'syscalls_mach','syscalls_unix','csw','threads','running','priority']]


if sys.platform == 'darwin':
    LIBPROC=ctypes.CDLL('/usr/lib/libproc.dylib',use_errno=True)
    LIBPROC.proc_pidinfo.argtypes=[ctypes.c_int,ctypes.c_int,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_int]
    LIBPROC.proc_pidinfo.restype=ctypes.c_int
    class Timebase(ctypes.Structure):
        _fields_=[('numer',ctypes.c_uint32),('denom',ctypes.c_uint32)]
    timebase=Timebase()
    system=ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    if system.mach_timebase_info(ctypes.byref(timebase)) != 0:
        raise RuntimeError('mach_timebase_info failed')
    TICK_SECONDS=timebase.numer/timebase.denom/1e9


def usage(pid):
    if sys.platform != 'darwin':
        raise RuntimeError('E11 CPU limiter currently requires Darwin proc_pidinfo')
    info=TaskInfo()
    count=LIBPROC.proc_pidinfo(pid,4,0,ctypes.byref(info),ctypes.sizeof(info))
    if count != ctypes.sizeof(info):
        raise ProcessLookupError(pid)
    return {'cpu_s':(info.user+info.system)*TICK_SECONDS,'rss_mb':info.rss/(1024*1024),'threads':info.threads}


class CpuBudget:
    """Bound aggregate CPU seconds with a 5 ms token bucket and SIGSTOP/SIGCONT.

    All threads of each PID count. No idle-time credit accumulates. The budget
    is shared by all listed server PIDs, including a proxy when configured.
    """
    def __init__(self, pids, cores):
        if cores <= 0:
            raise ValueError('CPU budget must be positive')
        self.pids=list(pids)
        if not self.pids or len(set(self.pids))!=len(self.pids) or any(pid<=1 or pid==os.getpid() for pid in self.pids):
            raise ValueError('CPU limiter requires distinct positive task PIDs, never a process group or itself')
        self.cores=cores
        self.done=threading.Event()
        self.error=None
        self.stops=0
        self.max_debt=0.
        self.thread=threading.Thread(target=self.run,daemon=True)

    def total(self):
        return sum(usage(pid)['cpu_s'] for pid in self.pids)

    def signal(self, action):
        for pid in self.pids:
            os.kill(pid,action)

    def run(self):
        try:
            previous=self.total()
            at=time.monotonic()
            credit=0.
            stopped=False
            while not self.done.wait(.002):
                now=time.monotonic()
                cpu=self.total()
                credit=min(.005*self.cores,credit+(now-at)*self.cores-(cpu-previous))
                at,previous=now,cpu
                self.max_debt=max(self.max_debt,-credit)
                # Hysteresis avoids stopping a one-core worker over microseconds of
                # sampling skew. The maximum extra credit is still only 5 ms per core.
                if credit < -.005*self.cores and not stopped:
                    self.signal(signal.SIGSTOP)
                    self.stops+=1
                    stopped=True
                elif credit >= 0 and stopped:
                    self.signal(signal.SIGCONT)
                    stopped=False
        except BaseException as error:
            self.error=repr(error)
        finally:
            for pid in self.pids:
                try: os.kill(pid,signal.SIGCONT)
                except ProcessLookupError: pass

    def start(self):
        self.thread.start()
        return self

    def stop(self):
        self.done.set()
        self.thread.join(5)
        if self.thread.is_alive():
            raise RuntimeError('CPU limiter did not stop')

    def report(self):
        return {'cores':self.cores,'stops':self.stops,'max_debt_cpu_s':self.max_debt,'error':self.error}


def percentile(values, quantile):
    if not values:
        raise ValueError('empty sample set')
    values=sorted(values)
    import math
    return values[max(0,min(len(values)-1,math.ceil(len(values)*quantile)-1))]
