"""Owns Linux counters, resource selection and the recoverable D12 CPU window.

Darwin counter implementations stay in their original modules. Linux schedstat
is nanoseconds; stat user/system counters are clock ticks. Short-lived threads
not seen by a sample retain only the process stat counter's tick precision.
"""
import argparse
import atexit
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

LINUX = sys.platform.startswith('linux')
ROOT = Path(__file__).resolve().parent.parent
PROC = Path('/proc')
SYS_CPU = Path('/sys/devices/system/cpu')
CGROUP = Path('/sys/fs/cgroup')
STATE = ROOT / '.local/evidence/E28/harness-restore.json'
ACTIVE = None
_cpu_cache = {}
_cpu_lock = threading.Lock()


def resources():
    default = (Path.home() / '.local/share/goliath-resources/26.3' if LINUX else
        Path.home() / 'Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3')
    return Path(os.environ.get('GOLIATH_RESOURCES', default)).expanduser()


def mac_only(operation):
    if LINUX:
        raise RuntimeError(operation + ': macOS only')


def window_owner(mac_owner):
    """Linux runners take the window under GOLIATH_WINDOW_OWNER (default E28)."""
    return os.environ.get('GOLIATH_WINDOW_OWNER', 'E28') if LINUX else mac_owner


def original_command(command):
    words = list(map(str, command))
    if '--goliath-cgroup-exec' in words:
        return words[words.index('--')+1:]
    return words


def thread_role(name):
    # Linux comm is 15 bytes, including names whose suffix used to be an ID.
    for prefix, role in [('proxy-io-', 'IO drivers'), ('proxy-control-', 'command workers'),
            ('terrain-manager', 'terrain managers'),
            ('wall-', 'wall workers'), ('player ', 'player supervisors')]:
        if name.startswith(prefix):
            return role
    return 'other matching threads'


def cpu_mode():
    mode = os.environ.get('GOLIATH_CPU_MODE', 'pin' if LINUX else 'limiter')
    if mode not in {'pin', 'limiter'} or (mode == 'pin' and not LINUX):
        raise ValueError('CPU mode must be pin on Linux or limiter')
    return mode


def cpu_options(parser):
    class Select(argparse.Action):
        def __call__(self, parser, namespace, values, option_string=None):
            os.environ['GOLIATH_CPU_MODE'] = values
            setattr(namespace, self.dest, values)
    parser.add_argument('--cpu-mode', choices=['pin', 'limiter'], default=cpu_mode(), action=Select)


def parse_stat(text, hz=None):
    """Parse comm safely even when it contains whitespace or closing brackets."""
    hz = hz or os.sysconf('SC_CLK_TCK')
    end = text.rindex(')')
    fields = text[end+2:].split()  # starts at stat field 3
    return dict(name=text[text.index('(')+1:end], parent=int(fields[1]),
        user_s=int(fields[11])/hz, system_s=int(fields[12])/hz,
        faults=int(fields[7])+int(fields[9]),
        start_ticks=int(fields[19]), threads=int(fields[17]),
        rss_mb=int(fields[21])*os.sysconf('SC_PAGE_SIZE')/1048576)


def parse_status(text):
    return {key: value.strip() for line in text.splitlines() if ':' in line
            for key, value in [line.split(':', 1)]}


def thread_ids(pid, proc=PROC):
    try:
        return sorted(int(path.name) for path in (proc / str(pid) / 'task').iterdir())
    except FileNotFoundError as error:
        raise ProcessLookupError(pid) from error


def thread_usage(pid, tid, proc=PROC):
    try:
        directory = proc / str(pid) / 'task' / str(tid)
        value = parse_stat((directory / 'stat').read_text())
        total = int((directory / 'schedstat').read_text().split()[0])/1e9
        name = (directory / 'comm').read_text().rstrip('\n')
        return dict(id=tid, name=name, is_main=tid == pid, at=time.monotonic(),
            user_cpu_s=value['user_s'], system_cpu_s=value['system_s'], cpu_s=total,
            cpu_precision='schedstat ns; user/system split in clock ticks')
    except (FileNotFoundError, ProcessLookupError) as error:
        raise RuntimeError('native thread identity disappeared') from error


def usage(pid, proc=PROC):
    directory = proc / str(pid)
    try:
        value = parse_stat((directory / 'stat').read_text())
        status = parse_status((directory / 'status').read_text())
        live = {}
        switches = [0, 0]
        for tid in thread_ids(pid, proc):
            task = directory / 'task' / str(tid)
            try:
                live[tid] = int((task / 'schedstat').read_text().split()[0])/1e9
                counters = parse_status((task / 'status').read_text())
                switches[0] += int(counters['voluntary_ctxt_switches'])
                switches[1] += int(counters['nonvoluntary_ctxt_switches'])
            except (FileNotFoundError, ProcessLookupError):
                # Linux may return ESRCH, not ENOENT, after a listed worker exits.
                # A vanished main thread still means the process counter failed.
                if tid == pid:raise ProcessLookupError(pid)
                continue
        # Remember observed retired threads; process stat also includes threads
        # that ran and exited entirely between samples. Never lower a total.
        with _cpu_lock:
            key = (str(proc), pid, value['start_ticks'])
            prior, retired, previous = _cpu_cache.get(key, ({}, 0., 0.))
            retired += sum(cpu for tid, cpu in prior.items() if tid not in live)
            total = max(value['user_s']+value['system_s'], retired+sum(live.values()), previous)
            _cpu_cache[key] = (live, retired, total)
        rss = float(status['VmRSS'].split()[0])/1024 if 'VmRSS' in status else value['rss_mb']
        return dict(at=time.monotonic(), cpu_s=total, user_s=value['user_s'], system_s=value['system_s'],
            rss_mb=rss, threads=int(status['Threads']), voluntary_context_switches=switches[0],
            faults=value['faults'],
            involuntary_context_switches=switches[1], context_switches=sum(switches),
            unix_syscalls=None, mach_syscalls=None, pageins=None, cow=None,
            unavailable=['unix_syscalls', 'mach_syscalls', 'pageins', 'cow'],
            context_switches_scope='live threads only; retired thread switches unavailable',
            cpu_precision='schedstat ns for observed threads; stat ticks for unobserved retired threads')
    except FileNotFoundError as error:
        raise ProcessLookupError(pid) from error


def process_snapshot(proc=PROC):
    """Stat includes exited threads; retain identity to detect PID reuse."""
    rows = {}
    for path in proc.iterdir():
        if not path.name.isdigit():
            continue
        try:
            value = parse_stat((path / 'stat').read_text())
            rows[int(path.name)] = dict(parent=value['parent'], name='outside',
                cpu_s=value['user_s']+value['system_s'], start_ticks=value['start_ticks'])
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return rows


def parse_cpu_stat(text):
    result = {}
    for line in text.splitlines():
        fields = line.split()
        if not fields or not fields[0].startswith('cpu') or not fields[0][3:].isdigit():
            continue
        ticks = list(map(int, fields[1:]))
        # guest and guest_nice are already included in user/nice.
        result[fields[0][3:]] = dict(total=sum(ticks[:8]), idle=ticks[3]+ticks[4])
    return result


def host_snapshot(proc=PROC, cpu=SYS_CPU):
    vm = dict(line.split() for line in (proc / 'vmstat').read_text().splitlines())
    memory = parse_status((proc / 'meminfo').read_text())
    throttle = {str(path.relative_to(cpu)): int(path.read_text())
        for path in sorted(cpu.glob('cpu[0-9]*/thermal_throttle/*')) if path.is_file()}
    frequency = {}
    for path in sorted(cpu.glob('cpu[0-9]*/cpufreq/scaling_cur_freq')):
        frequency[path.parent.parent.name[3:]] = int(path.read_text())
    return dict(at=time.monotonic(), load=os.getloadavg(), cpus=parse_cpu_stat((proc / 'stat').read_text()),
        swap_in_pages=int(vm['pswpin']), memory_available_kb=int(memory['MemAvailable'].split()[0]),
        frequency_khz=frequency, thermal_throttle=throttle,
        thermal_throttle_available=bool(throttle))


def host_delta(first, last):
    busy = {}
    for name, end in last['cpus'].items():
        start = first['cpus'][name]
        total = end['total']-start['total']
        busy[name] = dict(total_ticks=total, busy_ticks=total-(end['idle']-start['idle']),
            busy_fraction=(total-end['idle']+start['idle'])/total if total else None)
    rising = {name: value-first['thermal_throttle'].get(name, value)
        for name, value in last['thermal_throttle'].items()
        if value > first['thermal_throttle'].get(name, value)}
    reasons = []
    if first['load'][0] > 4:
        reasons.append('start load above 4')
    if busy.get('5', {}).get('busy_fraction') is None:
        reasons.append('CPU 5 busy coverage unavailable')
    elif busy['5']['busy_fraction'] >= .95:
        reasons.append('CPU 5 busy fraction at least 95%')
    if rising:
        reasons.append('thermal throttle counters rose')
    if not first['thermal_throttle_available'] or not last['thermal_throttle_available']:
        reasons.append('thermal throttle counters unavailable')
    return dict(cpu_mode=cpu_mode(), per_cpu=busy, throttle_delta=rising,
        swap_in_pages=last['swap_in_pages']-first['swap_in_pages'],
        memory_available_kb_start=first['memory_available_kb'], memory_available_kb_end=last['memory_available_kb'],
        valid=not reasons, reasons=reasons)


def socket_inputs(pid):
    """Render Linux TCP4 counters for the existing socket sampler's parser."""
    inodes = set()
    for fd in (PROC/str(pid)/'fd').iterdir():
        try:
            target = fd.readlink().as_posix()
            if target.startswith('socket:['):
                inodes.add(target[8:-1])
        except FileNotFoundError:
            pass
    states = {'01':'ESTABLISHED', '02':'SYN_SENT', '03':'SYN_RECEIVED', '04':'FIN_WAIT_1',
        '05':'FIN_WAIT_2', '06':'TIME_WAIT', '07':'CLOSED', '08':'CLOSE_WAIT', '09':'LAST_ACK',
        '0A':'LISTEN', '0B':'CLOSING'}
    owned, netstat = [], []
    for line in (PROC/'net/tcp').read_text().splitlines()[1:]:
        fields = line.split()
        def address(value):
            ip, port = value.split(':')
            octets = bytes.fromhex(ip)[::-1]
            return '.'.join(map(str, octets)), int(port, 16)
        left, a = address(fields[1]); right, b = address(fields[2])
        state = states[fields[3]]
        if fields[9] in inodes:
            owned.extend(['f'+fields[9], 'n'+f'{left}:{a}'+('->'+f'{right}:{b}' if b else ''), 'TST='+state])
        if left == right == '127.0.0.1':
            netstat.append(f'tcp4 0 0 {left}.{a} {right}.{b} {state}')
    return '\n'.join(owned), '\n'.join(netstat)


def sudo_write(path, value):
    subprocess.run(['sudo', '-n', 'tee', str(path)], input=str(value)+'\n', text=True,
        stdout=subprocess.DEVNULL, check=True)


def slice_command(unit, cpus):
    return ['sudo', '-n', 'systemctl', 'set-property', '--runtime', unit, 'AllowedCPUs='+cpus]


def save_state(state, path=STATE):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2)+'\n')
    temporary.replace(path)


def restore(path=STATE):
    """Journal each successful restoration so retries are safe after partial failure."""
    if not path.exists():
        return
    state = json.loads(path.read_text())
    errors = []
    for unit, previous in list(state['slices'].items()):
        try:
            subprocess.run(slice_command(unit, previous), check=True)
            del state['slices'][unit]
            save_state(state, path)
        except Exception as error:
            errors.append(str(error))
    for name, previous in list(state['governors'].items()):
        try:
            sudo_write(name, previous)
            del state['governors'][name]
            save_state(state, path)
        except Exception as error:
            errors.append(str(error))
    # Restore slices before putting the driver back in its original cgroup.
    recovery_group = state.get('recovery_cgroup', state.get('driver_cgroup'))
    if state.get('driver_cgroup'):
        try:
            process = PROC/str(state['pid'])/'stat'
            if process.exists() and parse_stat(process.read_text())['start_ticks'] == state.get('driver_start_ticks'):
                sudo_write(CGROUP / state['driver_cgroup'].lstrip('/') / 'cgroup.procs', state['pid'])
            state['driver_cgroup'] = None
            save_state(state, path)
        except Exception as error:
            errors.append(str(error))
    base = Path(state['cgroup'])
    if base.exists():
        for group in [base/'servers', base/'bots', base/'harness', base]:
            if group.exists():
                try:
                    # Recovery after SIGKILL must not kill unknown surviving
                    # children. Restore their ordinary placement, then remove
                    # the now-empty fixture cgroup. Regular cleanup reaps them.
                    members = (group/'cgroup.procs').read_text().split()
                    if members and not recovery_group:
                        raise RuntimeError('recovery cgroup missing for surviving processes')
                    for member in members:
                        sudo_write(CGROUP/recovery_group.lstrip('/')/'cgroup.procs', member)
                    subprocess.run(['sudo', '-n', 'rmdir', str(group)], check=True)
                except Exception as error:
                    errors.append(str(error))
    if errors:
        raise RuntimeError('D12 restore incomplete; retry scripts/goliath-cpu restore: '+ '; '.join(errors))
    path.unlink(missing_ok=True)


class CpuWindow:
    """One fixture cgroup tree per cell; caller already holds the host window."""
    def __init__(self):
        self.owner = False
        self.base = None
        self.previous_handlers = {}

    def start(self):
        global ACTIVE
        if not LINUX or ACTIVE:
            return self
        inherited=os.environ.get('GOLIATH_CPU_CGROUP')
        if inherited:
            if not Path(inherited).is_dir() or not STATE.exists() or json.loads(STATE.read_text())['cgroup'] != inherited:
                raise RuntimeError('inherited D12 cgroup is stale or does not match the restore journal')
            return self
        owner = ROOT / '.local/host/quiet/owner'
        if not owner.exists() or owner.read_text().splitlines()[0] != window_owner(None):
            raise RuntimeError('D12 requires a host-window quiet reservation')
        status = subprocess.check_output([str(ROOT/'.local/bin/host-window'), 'status'], text=True)
        if not any(line.strip() == 'builds:' for line in status.splitlines()):
            raise RuntimeError('D12 host window is still draining builds')
        if STATE.exists():
            raise RuntimeError('stale D12 window; run scripts/goliath-cpu restore first')
        if (CGROUP/'cgroup.controllers').read_text().split().count('cpuset') != 1:
            raise RuntimeError('cgroup v2 cpuset controller unavailable')
        if (SYS_CPU/'online').read_text().strip() != '0-5':
            raise RuntimeError('D12 requires exactly CPUs 0-5 online')
        base = CGROUP / ('goliath-e28-'+str(os.getpid()))
        original = next(line.split('::', 1)[1] for line in (PROC/'self/cgroup').read_text().splitlines() if line.startswith('0::'))
        state = dict(pid=os.getpid(), driver_start_ticks=parse_stat((PROC/'self/stat').read_text())['start_ticks'],
            cgroup=str(base), driver_cgroup=original, recovery_cgroup=original, slices={}, governors={})
        # Collect the complete undo journal before the first mutation.
        units = sorted(path.name for path in CGROUP.iterdir() if path.name.endswith('.slice') or path.name == 'init.scope')
        for unit in units:
            state['slices'][unit] = subprocess.check_output(['systemctl', 'show', unit, '-p', 'AllowedCPUs', '--value'], text=True).strip()
        for governor in sorted(SYS_CPU.glob('cpufreq/policy*/scaling_governor')):
            state['governors'][str(governor)] = governor.read_text().strip()
        if not state['governors']:
            raise RuntimeError('CPU governors unavailable')
        save_state(state)
        self.owner = True
        atexit.register(self.stop)
        for sig in [signal.SIGINT, signal.SIGTERM, signal.SIGHUP]:
            self.previous_handlers[sig] = signal.getsignal(sig)
            signal.signal(sig, self.interrupted)
        try:
            sudo_write(CGROUP/'cgroup.subtree_control', '+cpuset')
            subprocess.run(['sudo', '-n', 'mkdir', str(base)], check=True)
            sudo_write(base/'cpuset.cpus', '0-5')
            sudo_write(base/'cpuset.mems', (CGROUP/'cpuset.mems.effective').read_text().strip())
            sudo_write(base/'cgroup.subtree_control', '+cpuset')
            for name, cpus in [('servers', '0-3' if cpu_mode() == 'pin' else '0-4'), ('bots', '4' if cpu_mode() == 'pin' else '0-4'), ('harness', '5')]:
                group = base/name
                subprocess.run(['sudo', '-n', 'mkdir', str(group)], check=True)
                sudo_write(group/'cpuset.cpus', cpus)
                sudo_write(group/'cpuset.mems', (CGROUP/'cpuset.mems.effective').read_text().strip())
            sudo_write(base/'harness/cgroup.procs', os.getpid())
            for unit in units:
                subprocess.run(slice_command(unit, '5'), check=True)
            for governor in state['governors']:
                sudo_write(governor, 'performance')
            os.environ['GOLIATH_CPU_CGROUP'] = str(base)
            self.base = base
            ACTIVE = self
            return self
        except BaseException:
            self.stop()
            raise

    def interrupted(self, sig, frame):
        # Raising unwinds fixture cleanup before cgroup removal.
        raise KeyboardInterrupt('D12 window interrupted by signal '+str(sig))

    def stop(self):
        global ACTIVE
        if not self.owner:
            return
        restore()
        self.owner = False
        ACTIVE = None
        os.environ.pop('GOLIATH_CPU_CGROUP', None)
        for sig, handler in self.previous_handlers.items():
            signal.signal(sig, handler)
        self.previous_handlers.clear()
        atexit.unregister(self.stop)


def launch_command(command, role=None):
    """Move the single-threaded wrapper before exec, never after target startup."""
    base = ACTIVE.base if ACTIVE else os.environ.get('GOLIATH_CPU_CGROUP')
    if not LINUX or not base:
        return command
    words = list(map(str, command))
    if '--goliath-cgroup-exec' in words:
        return command
    name = Path(words[0]).name
    # e4-client is a test client, so it runs on the bot core, not the server cores.
    role = role or ('bots' if name in {'horizon-bots', 'e4-client'} else 'servers' if name in
        {'horizon-client', 'meridian', 'goliath-store', 'meridian-sidecar'} else 'harness')
    if role not in {'servers', 'bots', 'harness'}:
        raise ValueError('unknown D12 role')
    group = Path(base)/role
    return [sys.executable, '-B', str(Path(__file__).resolve()), '--goliath-cgroup-exec', str(group), '--', *words]


if __name__ == '__main__':
    if len(sys.argv) < 5 or sys.argv[1] != '--goliath-cgroup-exec' or sys.argv[3] != '--':
        raise SystemExit('internal cgroup wrapper; use scripts/goliath-cpu restore for recovery')
    group = Path(sys.argv[2]).resolve()
    if group.parent.parent != CGROUP or not group.parent.name.startswith('goliath-e28-') or group.name not in {'servers', 'bots', 'harness'}:
        raise SystemExit('invalid D12 cgroup target')
    sudo_write(group/'cgroup.procs', os.getpid())
    os.execvpe(sys.argv[4], sys.argv[4:], os.environ)
