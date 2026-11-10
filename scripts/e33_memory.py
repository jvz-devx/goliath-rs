"""Low-frequency Linux memory probes and disk-backed tile console history."""
from collections import Counter
import itertools
from pathlib import Path

import goliath_platform as platform


def process_detail(pid, proc=platform.PROC):
    directory = proc / str(pid)
    status = platform.parse_status((directory / 'status').read_text())
    raw = (directory / 'stat').read_text()
    fields = raw[raw.rindex(')')+2:].split()
    names = Counter()
    disappeared = 0
    for task in (directory / 'task').iterdir():
        try:
            names[(task / 'comm').read_text().rstrip('\n')] += 1
        except (FileNotFoundError, ProcessLookupError):
            disappeared += 1
    rollup = platform.smaps_rollup(pid, proc)
    return dict(smaps_rollup_kib=rollup,
                file_backed_rss_kib=rollup['Rss']-rollup['Anonymous'],
                file_backed_scope='Rss minus Anonymous; includes any shmem',
                status={key: status.get(key) for key in ['VmData','VmHWM','VmRSS','VmSwap','Threads','THP_enabled']},
                thread_names=dict(sorted(names.items())), vanished_threads=disappeared,
                minor_faults=int(fields[7]), major_faults=int(fields[9]),
                start_ticks=int(fields[19]))


def thp_counters(proc=platform.PROC, sys=Path('/sys')):
    counters = {key: int(value) for key, value in
                (line.split() for line in (proc / 'vmstat').read_text().splitlines())
                if key.startswith('thp_')}
    base = sys / 'kernel/mm/transparent_hugepage'
    for name in ['enabled','defrag','khugepaged/pages_collapsed','khugepaged/full_scans','khugepaged/max_ptes_none']:
        path = base / name
        if path.exists():
            counters[name] = path.read_text().strip()
    return counters


def thp_settings(sys=Path('/sys')):
    base = sys / 'kernel/mm/transparent_hugepage'
    paths = [base/'enabled', base/'defrag', *sorted((base/'khugepaged').glob('*'))]
    return {str(path.relative_to(base)):path.read_text().strip() for path in paths if path.is_file()}


def disk_history_class(base):
    """Keep all native bytes, with one text-stream seek cookie per 1024 lines."""
    class DiskHistoryServer(base):
        def __init__(self, *args, **kwargs):
            self.history_count = 0
            self.history_offsets = [0]
            super().__init__(*args, **kwargs)

        def _read(self):
            for line in self.process.stdout:
                with self.lock:
                    self.log.write(line)
                    self.log.flush()
                    self.history_count += 1
                    if self.history_count % 1024 == 0:
                        self.history_offsets.append(self.log.tell())

        def count(self):
            with self.lock:
                return self.history_count

        def since(self, n):
            with self.lock:
                count = self.history_count
                n = max(0, count+n) if n < 0 else n
                if n >= count:
                    return []
                block = n // 1024
                offset = self.history_offsets[block]
                path = Path(self.log.name)
            with path.open(encoding='utf-8', errors='replace') as stream:
                stream.seek(offset)
                return [line.rstrip('\n') for line in itertools.islice(stream, n-block*1024, count-block*1024)]
    return DiskHistoryServer
