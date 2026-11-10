"""Owns whole-window D12 evidence, descendant placement and restoration checks."""
import json
import os
from pathlib import Path
import subprocess
import time

from e18_process import ProcessSampler
import goliath_platform as platform


def ready():
    if platform.window_owner(None) == 'E28S':
        required = platform.ROOT/'docs/experiments/data/E28/reviewer-lite/repro-e28-lite-b2-summary.json'
        if not required.is_file():
            raise RuntimeError('required reviewer lite summary is not present; do not queue a measurement')
        status = subprocess.check_output([platform.ROOT/'.local/bin/host-window', 'status'], text=True)
        if 'window: free' not in status.splitlines():
            raise RuntimeError('host window is not free; do not queue a measurement')


def policy():
    from e33_memory import thp_settings
    units = sorted(path.name for path in platform.CGROUP.iterdir()
        if path.name.endswith('.slice') or path.name == 'init.scope')
    return dict(transparent_hugepage=thp_settings(),slices={unit:subprocess.check_output(
        ['systemctl', 'show', unit, '-p', 'AllowedCPUs', '--value'], text=True).strip() for unit in units},
        # The kernel value, because a cleared systemd setting leaves the cpuset behind.
        kernel_cpus={unit:(platform.CGROUP/unit/'cpuset.cpus.effective').read_text().strip() for unit in units
            if (platform.CGROUP/unit/'cpuset.cpus.effective').exists()},
        governors={str(path):path.read_text().strip()
            for path in sorted(platform.SYS_CPU.glob('cpufreq/policy*/scaling_governor'))},
        turbo={str(path):path.read_text().strip() for path in
            [platform.SYS_CPU/'intel_pstate/no_turbo',platform.SYS_CPU/'cpufreq/boost'] if path.exists()})


def same_policy(before, after):
    """Compare settings, not khugepaged's read-only cumulative counters."""
    def settings(value):
        return {key:({name:item for name,item in row.items()
                     if name not in ['khugepaged/full_scans','khugepaged/pages_collapsed']}
                     if key=='transparent_hugepage' else row) for key,row in value.items()}
    return settings(before)==settings(after)


def process_placement(pid):
    directory=platform.PROC/str(pid)
    def group():
        return next(line.split('::',1)[1] for line in (directory/'cgroup').read_text().splitlines() if line.startswith('0::'))
    def status():return platform.parse_status((directory/'status').read_text())
    for attempt in range(4):
        first_group=group();first_exe=(directory/'exe').readlink().name;first=status()
        command=(directory/'cmdline').read_bytes().split(b'\0')
        last_exe=(directory/'exe').readlink().name;last_group=group();last=status()
        stable=first_group==last_group and first_exe==last_exe and first['Cpus_allowed_list']==last['Cpus_allowed_list']
        role='servers' if last_exe in {'horizon-client','meridian','meridian-sidecar','goliath-store'} else (
            'bots' if last_exe in {'horizon-bots','e4-client'} else 'harness')
        # A known single-threaded launcher may be sampled after moving but before exec.
        # Charge it to the declared destination only after it actually joins that group.
        wrapper=False
        if last_exe.startswith('python') and b'--goliath-cgroup-exec' in command:
            index=command.index(b'--goliath-cgroup-exec')
            if index+1<len(command) and command[index+1].decode()==str(platform.CGROUP/last_group.lstrip('/')):
                destination=Path(last_group).name
                if destination in {'servers','bots','harness'}:
                    role=destination;wrapper=True
        rss=last.get('VmRSS');swap=last.get('VmSwap')
        record=dict(pid=pid,role=role,cpus_allowed_list=last['Cpus_allowed_list'],cgroup=last_group,
            rss_mib=int(rss.split()[0])/1024 if rss else None,swap_mib=int(swap.split()[0])/1024 if swap else None,
            stable=stable,placement_reads=attempt+1,pre_exec_wrapper=wrapper)
        if stable:return record
    return record


def audit(rows, error=None):
    elapsed = rows[-1]['at'] - rows[0]['at']
    totals = {}
    for left, right in zip(rows, rows[1:]):
        owned = set(left['owned']) | set(right['owned'])
        for pid, value in right['processes'].items():
            if pid in owned:
                continue
            previous = left['processes'].get(pid)
            same = previous and previous.get('start_ticks') == value.get('start_ticks')
            delta = max(0, value['cpu_s'] - (previous['cpu_s'] if same else 0))
            totals[pid] = totals.get(pid, 0) + delta
    outside = [dict(pid=pid, name='outside', cpu_s=amount, cores=amount/elapsed)
        for pid, amount in sorted(totals.items(), key=lambda pair:pair[1], reverse=True)]
    d12 = platform.host_delta(rows[0]['host'], rows[-1]['host'])
    expected = dict(servers='0-3', bots='4', harness='5') if platform.cpu_mode() == 'pin' else dict(
        servers='0-4', bots='0-4', harness='5')
    bad = [dict(at=row['at'], **entry) for row in rows for entry in row['descendant_placement']
        if not entry.get('stable',True) or entry['cpus_allowed_list'] != expected.get(entry['role']) or Path(entry['cgroup']).name != entry['role']]
    if bad:
        d12['reasons'].append('descendant CPU placement mismatch')
    if error:
        d12['reasons'].append('whole-window sampler failed')
    if any(row['cores'] > .5 for row in outside):
        d12['reasons'].append('outside process above half core over whole window')
    d12['valid'] = not d12['reasons']
    return dict(seconds=elapsed, d12=d12, valid=d12['valid'], error=error,
        maximum_outside_process_cores=max((row['cores'] for row in outside), default=0),
        outside_total_cores=sum(row['cores'] for row in outside), outside=outside,
        placement_errors=bad, samples=rows,
        host_first=rows[0]['host'],host_last=rows[-1]['host'],
        frequency_khz_range={str(cpu):[min(values),max(values)] for cpu in range(6)
            if (values:=[row['host']['frequency_khz'][str(cpu)] for row in rows if str(cpu) in row['host'].get('frequency_khz',{})])},
        scope='one-second and boundary snapshots, acquisition through recovery and cleanup; no outside names retained')


def minute_rss(rows,origin,minutes=30):
    result=[]
    for minute in range(minutes+1):
        target=origin+minute*60
        row=min(rows,key=lambda value:abs(value['at']-target))
        host=row.get('host') or {}
        result.append(dict(minute=minute,elapsed_seconds=row['at']-origin,
            endpoint_covered=abs(row['at']-target)<2,
            host={key:host.get(key) for key in ['swap_in_pages','swap_out_pages','memory_available_kb']},
            processes=[dict(pid=entry['pid'],role=entry['role'],rss_mib=entry.get('rss_mib'),swap_mib=entry.get('swap_mib'))
                for entry in row['descendant_placement']]))
    return result


class WholeWindow(ProcessSampler):
    def start(self):
        deadline=time.monotonic()+600
        while os.getloadavg()[0]>4:
            if time.monotonic()>deadline:raise RuntimeError('whole-window start load did not settle below four')
            time.sleep(1)
        self.original_policy = policy()
        return super().start()

    def capture(self, mark=None):
        row = super().capture(mark)
        owned = {os.getpid()}
        while True:
            updated = owned | {pid for pid, value in row['processes'].items() if value['parent'] in owned}
            if updated == owned:
                break
            owned = updated
        row['owned'] = sorted(owned)
        placement = []
        for pid in sorted(owned):
            try:
                placement.append(process_placement(pid))
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                pass
        row['descendant_placement'] = placement
        for value in row['processes'].values():
            value['name'] = 'outside'
        return row

    def finish(self):
        self.capture('window-end-before-restore')
        self.stop()
        result = audit(self.rows, self.error)
        restored = policy()
        result['policy_before'] = self.original_policy
        result['policy_after'] = restored
        result['restored'] = same_policy(self.original_policy,restored) and not platform.STATE.exists()
        result['valid'] = result['valid'] and result['restored']
        return result
