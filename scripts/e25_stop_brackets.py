"""Records existing signal calls on the shared raw clock, without changing budget decisions."""
import os
import signal
import threading
import time


class OsProxy:
    def __init__(self,original,kill):self.original,self.kill=original,kill
    def __getattr__(self,name):return getattr(self.original,name)


class StopBrackets:
    def __init__(self,maximum=2_000_000):
        self.roles,self.events={},[]
        self.maximum,self.dropped=maximum,0
        self.lock=threading.Lock()
        self.original_kill=os.kill
        self.os_proxy=OsProxy(os,self.kill)

    def register(self,role,pid):
        with self.lock:self.roles[pid]=role

    def kill(self,pid,action):
        with self.lock:role=self.roles.get(pid)
        if role is None or action not in [signal.SIGSTOP,signal.SIGCONT]:return self.original_kill(pid,action)
        before=time.clock_gettime_ns(time.CLOCK_MONOTONIC_RAW)
        error=None
        try:return self.original_kill(pid,action)
        except BaseException as caught:
            error=type(caught).__name__
            raise
        finally:
            after=time.clock_gettime_ns(time.CLOCK_MONOTONIC_RAW)
            row=dict(pid=pid,role=role,action='STOP' if action==signal.SIGSTOP else 'CONT',
                raw_before_ns=before,raw_after_ns=after,error=error)
            with self.lock:
                if len(self.events)<self.maximum:self.events.append(row)
                else:self.dropped+=1

    def report(self):
        with self.lock:
            return dict(method='before/after existing os.kill calls on CLOCK_MONOTONIC_RAW; not kernel stop acknowledgement',
                budget_or_membership_changed=False,dropped=self.dropped,roles=dict(self.roles),events=list(self.events))
