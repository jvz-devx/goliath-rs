"""Owns the Linux soak memory rule: resident plus swapped memory, judged from a
baseline taken after the store's bounded receipt window has filled.

The host swaps to compressed memory during a soak, so resident size alone
falls while use does not. The store grows until its receipt window is full and
is flat afterwards, so a baseline before that point reads the fill as a leak.
"""
from pathlib import Path

FIRST_MINUTE = 5
JUDGED_SECONDS = 1500
# A baseline later than this would push the run past the runner's 45-minute cap.
LAST_BASELINE_MINUTE = 15


def window_full(path):
    """Read the store's own memory trace: its last complete row says when the window filled."""
    try:
        with Path(path).open('rb') as stream:
            header = stream.readline().decode().strip().split(',')
            size = stream.seek(0, 2)
            stream.seek(max(0, size-8192))
            tail = stream.read()
    except OSError as error:
        return dict(readable=False, error=type(error).__name__)
    lines = tail[:tail.rfind(b'\n')+1].decode().splitlines()
    if size > 8192:
        lines = lines[1:]
    rows = [line.split(',') for line in lines if line and not line.startswith('at_ns')]
    if 'window_full_at_ns' not in header or 'receipts' not in header or not rows or len(rows[-1]) != len(header):
        return dict(readable=False, error='no complete memory row')
    last = dict(zip(header, rows[-1]))
    return dict(readable=True, full=int(last['window_full_at_ns']) >= 0, window_full_at_store_ns=int(last['window_full_at_ns']),
        receipts=int(last['receipts']), trace_at_store_ns=int(last['at_ns']))


class Rule:
    """Decides the baseline minute while the soak runs, so the run can be extended to match."""
    def __init__(self, trace, last_baseline_minute=LAST_BASELINE_MINUTE, run_cap_minutes=None, minimum_loaded_seconds=1800):
        self.trace = Path(trace)
        self.baseline = None
        self.checks = []
        self.minute = FIRST_MINUTE-1
        # A registered long soak waits longer for the fill and, without one, runs to its cap.
        self.last_baseline_minute = last_baseline_minute
        self.run_cap_minutes = run_cap_minutes
        self.capped = False
        self.minimum_loaded_seconds = minimum_loaded_seconds

    def sample(self, elapsed):
        """Call with every memory sample. Looks once at each whole loaded minute from minute 5."""
        minute = int(elapsed//60)
        if self.baseline or minute <= self.minute:
            return
        self.minute = minute
        state = window_full(self.trace)
        self.checks.append(dict(minute=minute, elapsed_seconds=elapsed, **state))
        detection = 'store memory trace: window_full_at_ns on its last complete row'
        if state.get('full'):
            self.baseline = dict(minute=minute, fill_detected=True, detection=detection,
                window_full_at_store_ns=state['window_full_at_store_ns'], receipts=state['receipts'])
        elif not state['readable']:
            self.baseline = dict(minute=FIRST_MINUTE, fill_detected=False, detection=detection,
                fallback='minute 5: the store memory trace could not be read ('+state['error']+')')
        elif minute >= self.last_baseline_minute:
            self.capped = self.run_cap_minutes is not None
            self.baseline = dict(minute=FIRST_MINUTE, fill_detected=False, detection=detection, receipts=state['receipts'],
                fallback=f'minute 5: the receipt window was not full by minute {self.last_baseline_minute}, the last baseline the run length allows'
                    + (f'; the run continues to its {self.run_cap_minutes}-minute cap' if self.capped else ''))

    def extension(self):
        """Seconds past minute 30 the run needs so the judged window is 25 minutes long."""
        if self.capped:
            return max((self.run_cap_minutes-30)*60., self.minimum_loaded_seconds-1800)
        ruled_extension = (self.baseline['minute']-FIRST_MINUTE)*60. if self.baseline else 0.
        return max(ruled_extension, self.minimum_loaded_seconds-1800)

    def record(self):
        return dict(rule='baseline at the later of minute 5 and the first per-minute sample after the store receipt window is full; judged window 25 minutes; memory is resident plus swapped',
            baseline=self.baseline, extension_seconds=self.extension(), checks=self.checks,
            first_minute=FIRST_MINUTE, judged_seconds=JUDGED_SECONDS, last_baseline_minute=self.last_baseline_minute,
            run_cap_minutes=self.run_cap_minutes, ran_to_cap_without_fill=self.capped,
            minimum_loaded_seconds=self.minimum_loaded_seconds)


def nearest(memory, seconds):
    return min(memory, key=lambda row: abs(row['elapsed_seconds']-seconds))


def growth(memory, start, end, group='processes'):
    """Per-role growth of resident plus swapped memory between two loaded times."""
    a, b = nearest(memory, start), nearest(memory, end)
    result = {}
    for role, left in a[group].items():
        right = b[group].get(role)
        if not right:
            continue
        before = left['rss_mb']+(left.get('swap_mb') or 0)
        after = right['rss_mb']+(right.get('swap_mb') or 0)
        result[role] = dict(from_seconds=a['elapsed_seconds'], to_seconds=b['elapsed_seconds'],
            before_rss_mib=left['rss_mb'], after_rss_mib=right['rss_mb'],
            before_swap_mib=left.get('swap_mb'), after_swap_mib=right.get('swap_mb'),
            before_mib=before, after_mib=after, growth_fraction=after/before-1,
            rss_growth_fraction=right['rss_mb']/left['rss_mb']-1, under_10_percent=after/before-1 < .1,
            swap_sampled=left.get('swap_mb') is not None and right.get('swap_mb') is not None)
    covered = abs(a['elapsed_seconds']-start) < 2 and abs(b['elapsed_seconds']-end) < 2
    return result, covered


def ruled(memory, record, group='processes'):
    """The judged window: 25 minutes from the recorded baseline."""
    start = record['baseline']['minute']*60 if record and record.get('baseline') else FIRST_MINUTE*60
    rows, covered = growth(memory, start, start+JUDGED_SECONDS, group)
    return dict(window_seconds=[start, start+JUDGED_SECONDS], endpoint_coverage=covered, processes=rows,
        every_swap_sampled=all(row['swap_sampled'] for row in rows.values()))
