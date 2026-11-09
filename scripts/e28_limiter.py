"""Owns native aggregate process-CPU reads for Linux's comparison limiter.

The token bucket, polling interval, signals, membership and caps are inherited
unchanged. Unlike full /proc usage reports, a process clock requires no Python
walk of every thread on the harness CPU for each budget sample.
"""
import ctypes
import os
import time

import goliath_platform as platform


def process_clock(pid):
    library = ctypes.CDLL(None)
    function = library.clock_getcpuclockid
    function.argtypes = [ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
    function.restype = ctypes.c_int
    clock = ctypes.c_int()
    error = function(pid, ctypes.byref(clock))
    if error:
        raise OSError(error, os.strerror(error))
    return clock.value


def budget_class(base):
    if not platform.LINUX:
        return base

    class NativeBudget(base):
        def __init__(self, pids, cores):
            super().__init__(pids, cores)
            self.process_clocks = [process_clock(pid) for pid in self.pids]

        def total(self):
            return sum(time.clock_gettime_ns(clock) for clock in self.process_clocks) / 1e9

        def report(self):
            result = super().report()
            result['cpu_counter'] = 'clock_getcpuclockid / clock_gettime_ns, aggregate running CPU of all process threads'
            return result

    return NativeBudget
