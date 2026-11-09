"""Owns incremental accepted-mutation rate checks on complete store CSV rows."""
from collections import deque
import math


class MutationWatch:
    def __init__(self,path,limit):
        self.path,self.limit=path,limit
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
            while self.queue and self.queue[0][0]<start-1_000_000:
                self.total-=self.queue.popleft()[1]
            self.queue.append((end,count));self.total+=count
            if self.total>self.maximum:self.maximum,self.at_us=self.total,start-1_000_000
            if self.maximum>self.limit:
                raise RuntimeError(f'accepted mutation interval upper{self.maximum}/s exceeds planning{self.limit}/s; stop before changing capacity')

    def report(self):
        return dict(maximum_interval_upper_per_second=self.maximum,planning_bound=self.limit,
            at_window_start_us=self.at_us,precision_padding_us=2,within_bound=self.maximum<=self.limit)
