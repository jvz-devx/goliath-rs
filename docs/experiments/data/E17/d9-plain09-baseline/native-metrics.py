"""Owns native main-thread CPU and outside-process validity for the plain-09 run."""
import ctypes
import time
from e11_support import LIBPROC


class ThreadInfo(ctypes.Structure):
    _fields_=[('user',ctypes.c_uint64),('system',ctypes.c_uint64)]+[
        (name,ctypes.c_int32) for name in ['cpu','policy','state','flags','sleep','current_priority','priority','maximum_priority']]+[
        ('name',ctypes.c_char*64)]


def thread_ids(pid):
    identifiers=(ctypes.c_uint64*8192)()
    # Flavor 6 returns pthread addresses. Flavor 28 returns native 64-bit IDs,
    # which flavor 15 accepts and Python's native-thread clock can validate.
    size=LIBPROC.proc_pidinfo(pid,28,0,identifiers,ctypes.sizeof(identifiers))
    if size<=0 or size>=ctypes.sizeof(identifiers):
        raise RuntimeError('native thread list unavailable or truncated')
    return list(identifiers[:size//ctypes.sizeof(ctypes.c_uint64)])


def thread_usage(pid,identity):
    info=ThreadInfo()
    if LIBPROC.proc_pidinfo(pid,15,identity,ctypes.byref(info),ctypes.sizeof(info))!=ctypes.sizeof(info):
        raise RuntimeError('main thread identity disappeared')
    return {'id':identity,'name':info.name.decode(errors='replace'),'at':time.monotonic(),
        'user_cpu_s':info.user/1e9,'system_cpu_s':info.system/1e9,'cpu_s':(info.user+info.system)/1e9}


def outside_validity(report):
    if report.get('error'): return False,['outside-process sampler failed']
    reasons=[]
    for phase in ['steady','whole_cell']:
        row=report.get('phases',{}).get(phase)
        if not row:
            reasons.append('missing outside-process '+phase+' coverage')
        elif row['outside_max_process_cores']>.5:
            reasons.append('outside process above 0.5 core in '+phase)
    return not reasons,reasons
