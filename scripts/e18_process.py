"""One-second whole-host CPU counters with explicit fixture roles and phase boundaries."""
import os
from pathlib import Path
import subprocess
import threading
import time
import goliath_platform as platform


def cpu_seconds(value):
    days = 0
    if '-' in value:
        day, value = value.split('-', 1)
        days = int(day)
    return days*86400+sum(float(part)*60**index for index, part in enumerate(reversed(value.split(':'))))


class ProcessSampler:
    def __init__(self, observer=False):
        self.roles = {('observer-and-driver' if observer else 'measurement-driver'): os.getpid()}
        self.rows = []
        self.marks = {}
        self.lock = threading.Lock()
        self.done = threading.Event()
        self.error = None
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.cpu_window = None

    def register(self, name, pid):
        self.roles[name] = pid

    def capture(self, mark=None):
        with self.lock:
            sent = time.monotonic()
            raw = '' if platform.LINUX else subprocess.check_output(['ps', '-axo', 'pid=,ppid=,time=,comm='], text=True)
            received = time.monotonic()
            processes = {}
            if platform.LINUX:
                processes = platform.process_snapshot()
                received = time.monotonic()
            for line in raw.splitlines():
                parts = line.split(None, 3)
                if len(parts) != 4: continue
                pid, parent, cpu, name = parts
                processes[int(pid)] = dict(parent=int(parent), cpu_s=cpu_seconds(cpu), name=Path(name).name)
            row = dict(at=(sent+received)/2, sent=sent, received=received, processes=processes,
                host_load=os.getloadavg(), mark=mark)
            if platform.LINUX:
                row['host'] = platform.host_snapshot()
                row['placement'] = {}
                for role, pid in self.roles.items():
                    try:
                        status=platform.parse_status((platform.PROC/str(pid)/'status').read_text())
                        group=next(line.split('::',1)[1] for line in (platform.PROC/str(pid)/'cgroup').read_text().splitlines() if line.startswith('0::'))
                        row['placement'][role]=dict(pid=pid,cpus_allowed_list=status['Cpus_allowed_list'],cgroup=group)
                    except FileNotFoundError:
                        continue
            self.rows.append(row)
            if mark: self.marks[mark] = row
            return row

    def start(self):
        if platform.LINUX:
            self.cpu_window = platform.CpuWindow().start()
        try:
            self.capture('cell-start')
            self.thread.start()
        except BaseException:
            if self.cpu_window:
                self.cpu_window.stop()
            raise
        return self

    def run(self):
        try:
            while not self.done.wait(1): self.capture()
        except BaseException as error:
            self.error = repr(error)

    def stop(self):
        self.done.set()
        self.thread.join(5)
        if self.thread.is_alive(): raise RuntimeError('process sampler did not stop')
        if self.cpu_window:
            self.cpu_window.stop()

    def phase(self, first, last):
        if first not in self.marks or last not in self.marks: return None
        a, b = self.marks[first], self.marks[last]
        duration = b['at']-a['at']
        if duration <= 0: return None
        fixture = {}
        for role, pid in self.roles.items():
            left = a['processes'].get(pid, {}).get('cpu_s', 0)
            right = b['processes'].get(pid, {}).get('cpu_s')
            if right is not None:
                fixture[role] = dict(pid=pid, cpu_s=max(0, right-left), cores=max(0, right-left)/duration)
        outsider = {}
        rows = [row for row in self.rows if a['at'] <= row['at'] <= b['at']]
        fixture_pids = set(self.roles.values()) if platform.LINUX else {pid for role, pid in self.roles.items() if role != 'measurement-driver'}
        for previous, current in zip(rows, rows[1:]):
            for pid, process in current['processes'].items():
                if pid in fixture_pids: continue
                old = previous['processes'].get(pid)
                same = old and old.get('start_ticks') == process.get('start_ticks')
                delta = max(0, process['cpu_s']-(old['cpu_s'] if same else 0))
                if delta:
                    record = outsider.setdefault(pid, dict(pid=pid, name='outside' if platform.LINUX else process['name'], cpu_s=0))
                    record['cpu_s'] += delta
        outside = sorted((dict(record, cores=record['cpu_s']/duration) for record in outsider.values()),
            key=lambda record: record['cores'], reverse=True)
        return dict(seconds=duration, fixture=fixture,
            server_cores=sum(value['cores'] for role, value in fixture.items() if role not in ['bots', 'observer-and-driver', 'measurement-driver']),
            outside=outside, outside_total_cores=sum(record['cores'] for record in outside),
            outside_max_process_cores=max((record['cores'] for record in outside), default=0),
            no_outside_process_above_half_core=all(record['cores'] <= .5 for record in outside))

    def report(self):
        fixture_pids = set(self.roles.values()) if platform.LINUX else {pid for role, pid in self.roles.items() if role != 'measurement-driver'}
        trace = []
        names = {}
        previous = {}
        for row in self.rows:
            outside_delta = {}
            for pid, process in row['processes'].items():
                if pid in fixture_pids: continue
                names[str(pid)] = process['name']
                old = previous.get(pid)
                if old and old.get('start_ticks') != process.get('start_ticks'):old=None
                delta = max(0, process['cpu_s']-(old['cpu_s'] if old else (0 if previous else process['cpu_s'])))
                if delta: outside_delta[str(pid)] = delta
            trace.append(dict(at=row['at'], mark=row['mark'], host_load=row['host_load'],
                fixture_cpu_s={role: row['processes'].get(pid, {}).get('cpu_s') for role, pid in self.roles.items()},
                outside_cpu_s_delta=outside_delta))
            if platform.LINUX:
                trace[-1]['host'] = row['host']
                trace[-1]['placement'] = row['placement']
            previous = row['processes']
        phases = {name: self.phase(*bounds) for name, bounds in {
            'cold_join': ('cell-start', 'joined'), 'warmup': ('joined', 'steady-first'),
            'steady': ('steady-first', 'steady-last'), 'whole_cell': ('cell-start', 'steady-last')}.items()}
        result = dict(method='ps cumulative process CPU time, one-second samples plus exact phase-boundary samples',
            precision='ps CPU counters have centisecond resolution; exited processes can lose their final subsecond CPU',
            error=self.error, roles=self.roles, phases=phases,
            outside_process_names=names, samples=trace)
        if platform.LINUX:
            result.update(method='/proc process stat with PID start identity; one-second and boundary samples',
                precision='process clock ticks; exited processes may lose final subsecond CPU', cpu_mode=platform.cpu_mode())
            end = self.marks.get('steady-last', self.rows[-1])
            result['d12'] = platform.host_delta(self.rows[0]['host'], end['host'])
            placement=end.get('placement',{})
            result['d12']['placement']=placement
            if result['d12'].get('platform'):result['platform']=result['d12']['platform']
            for role, row in placement.items():
                leaf=Path(row['cgroup']).name
                expected={'servers':'0-3','bots':'4'} if platform.cpu_mode()=='pin' else {'servers':'0-4','bots':'0-4'}
                expected['harness']=platform.cpu_span(platform.harness_cpus(end['host']['cpus']))
                if leaf not in expected or row['cpus_allowed_list']!=expected[leaf]:
                    result['d12']['reasons'].append('fixture CPU placement mismatch: '+role)
            result['d12']['valid']=not result['d12']['reasons']
        return result
