"""Owns incremental accepted-mutation rate checks on complete store CSV rows."""
from collections import deque
import math


RETRY_SECONDS=27
RECEIPT_MARGIN=4


def covered_rate(capacity):
    """The largest planning rate a receipt cap covers: 1 + 4 * ceil(27 * rate) receipts must fit."""
    return (capacity-1)//(RECEIPT_MARGIN*RETRY_SECONDS)


class MutationWatch:
    """Stops a run whose accepted mutations outrun the planning rate.

    The rate stands for the receipt window's budget: the cap must hold four
    times the mutations of one 27-second retry budget. The default checks the
    rate over every rolling second, a conservative stand-in for that budget.
    With `window_seconds=27` it checks the budget itself: the mutations in any
    rolling 27 seconds may not exceed 27 times the rate.
    """
    def __init__(self,path,limit,window_seconds=1):
        self.path,self.limit=path,limit
        self.window_seconds=window_seconds;self.window_us=window_seconds*1_000_000
        self.bound=limit*window_seconds
        self.offset=0;self.partial=b'';self.header=None
        self.queue=deque();self.total=0;self.maximum=0;self.at_us=None

    def sample(self):
        with self.path.open('rb') as source:
            source.seek(self.offset);content=self.partial+source.read();self.offset=source.tell()
        complete,separator,self.partial=content.rpartition(b'\n')
        if not separator:self.partial=content;return
        for line in complete.decode().splitlines():
            fields=line.split(',')
            if self.header is None:
                self.header=fields
                if 'mutations' not in fields:raise RuntimeError('accepted-mutation diagnostic absent')
                continue
            row=dict(zip(self.header,fields,strict=True));count=int(row['mutations'])
            if not count:continue
            start=int(row['start_us']);end=start+math.ceil(float(row['execute_ms'])*1000)+2
            while self.queue and self.queue[0][0]<start-self.window_us:
                self.total-=self.queue.popleft()[1]
            self.queue.append((end,count));self.total+=count
            if self.total>self.maximum:self.maximum,self.at_us=self.total,start-self.window_us
            if self.maximum>self.bound:
                if self.window_seconds==1:
                    raise RuntimeError(f'accepted mutation interval upper{self.maximum}/s exceeds planning{self.limit}/s; stop before changing capacity')
                raise RuntimeError(f'accepted mutations in a rolling{self.window_seconds}s window upper{self.maximum} exceed planning{self.limit}/s for{self.window_seconds}s; stop before changing capacity')

    def report(self):
        if self.window_seconds==1:
            return dict(maximum_interval_upper_per_second=self.maximum,planning_bound=self.limit,
                at_window_start_us=self.at_us,precision_padding_us=2,within_bound=self.maximum<=self.limit)
        return dict(window_seconds=self.window_seconds,maximum_window_upper=self.maximum,planning_rate_per_second=self.limit,
            planning_bound=self.bound,at_window_start_us=self.at_us,precision_padding_us=2,within_bound=self.maximum<=self.bound)
