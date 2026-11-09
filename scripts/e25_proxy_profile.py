"""Owns local proxy CPU capture after the latency window and Running-only attribution."""
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import time
import xml.etree.ElementTree as ET


def capture(instance,evidence,monitor,native,thread_counters):
    folder=evidence/'proxy-profile'
    folder.mkdir(exist_ok=False)
    trace=folder/'proxy.trace'
    pid=instance.proxy.process.pid
    result=dict(kind='E25-proxy-profile',timing='after completed latency window',profiling_perturbs_runtime=True,
        pid=pid,raw_trace_local=True,commands=[])
    result['native_before']=native(pid)
    result['threads_before']=thread_counters(pid)
    command=['xcrun','xctrace','record','--template','Time Profiler','--attach',str(pid),
        '--time-limit','10s','--no-prompt','--output',str(trace)]
    log=folder/'record.log'
    with log.open('x') as output:
        child=subprocess.Popen(command,stdout=output,stderr=subprocess.STDOUT)
        monitor.register('fixture-proxy-profiler',child.pid)
        try:code=child.wait(timeout=55)
        except subprocess.TimeoutExpired:
            child.terminate();child.wait(timeout=10);code=-1
    result['commands'].append(dict(label='capture',exit=code,log=str(log)))
    result['native_after']=native(pid)
    result['threads_after']=thread_counters(pid)
    result['status']='captured' if code==0 else 'failed'
    if code==0:result['trace']=str(trace)
    (folder/'capture.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def running(path):
    raw=gzip.decompress(path.read_bytes())
    root=ET.fromstring(raw)
    definitions={node.get('id'):node for node in root.iter() if node.get('id')}
    def resolve(node):
        while node is not None and node.get('ref'):node=definitions[node.get('ref')]
        return node
    states,categories,leaves,threads=Counter(),Counter(),Counter(),Counter()
    examples={}
    for row in root.findall('.//row'):
        state=resolve(row.find('thread-state')).text
        weight=int(resolve(row.find('weight')).text)
        states[state]+=weight
        if state!='Running':continue
        thread=resolve(row.find('thread')).get('fmt','')
        tagged=resolve(row.find('tagged-backtrace'))
        back=resolve(tagged.find('backtrace')) if tagged is not None else None
        frames=[resolve(frame).get('name','') for frame in back.findall('frame')] if back is not None else []
        leaf=frames[0] if frames else 'missing unwind'
        joined='\n'.join(frames)
        if any(term in leaf for term in ['sendto','recvfrom','sendmsg','recvmsg','writev','readv','__read_nocancel','__write_nocancel']):category='socket syscalls'
        elif any(term in leaf for term in ['malloc','calloc','realloc','_free','memcpy','memmove','bzero','::dealloc']):category='allocation and byte copies'
        elif any(term in leaf for term in ['psynch','ulock','pthread_cond','pthread_mutex','semaphore','sema_','semwait','nanosleep','os_unfair','kevent','kqueue','mach_msg','thread_switch','sched_']):category='readiness, synchronization and wakeups'
        elif leaf in ['mach_absolute_time','mach_continuous_time','clock_gettime']:category='clock reads'
        elif any(term in joined for term in ['flate2','miniz','inflate','deflate','horizon_protocol::frame','horizon_protocol::connection']):category='framing and compression'
        elif 'meridian_ghost' in joined or 'meridian_rewrite' in joined:category='ghost view and packet rewrite'
        elif 'meridian_sidecar' in joined or 'serde_json' in joined:category='control and JSON'
        elif 'terrain' in joined:category='terrain work'
        else:category='other or unattributed'
        categories[category]+=weight;leaves[leaf]+=weight;threads[thread]+=weight
        examples.setdefault(category,frames[:12])
    total=states['Running']
    if total<=0:raise ValueError('profile has no Running weights; CPU attribution remains unproven')
    return dict(method='exclusive Running weights only; blocked occupancy excluded',classification_version=2,total_cpu_seconds=total/1e9,
        categories={name:dict(cpu_seconds=weight/1e9,percent=100*weight/total if total else None) for name,weight in categories.most_common()},
        top_leaves=[dict(frame=name,cpu_seconds=weight/1e9,percent=100*weight/total if total else None) for name,weight in leaves.most_common(25)],
        thread_cpu_seconds={name:weight/1e9 for name,weight in threads.most_common()},
        states_weight_ns=dict(states),category_examples=examples,export_decompressed_sha256=hashlib.sha256(raw).hexdigest(),
        limitations=['category heuristics are not exact function boundaries','missing/inlined work remains in the denominator',
            'readiness/parking frames count only when Running; no blocked time is charged as CPU',
            'context-switch counts alone do not give context-switch CPU seconds'])


def export(evidence,root):
    folder=evidence/'proxy-profile'
    record_path=folder/'capture.json'
    if not record_path.exists():return None
    result=json.loads(record_path.read_text())
    if result['status']!='captured':return dict(status='failed',capture_local=str(record_path.relative_to(root)))
    trace=Path(result['trace'])
    command=['xcrun','xctrace','export','--input',str(trace),'--xpath','/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]']
    completed=subprocess.run(command,capture_output=True,timeout=55)
    (folder/'export.stderr').write_bytes(completed.stderr)
    if completed.returncode:return dict(status='failed-export',capture_local=str(record_path.relative_to(root)))
    path=folder/'time-profile.xml.gz'
    path.write_bytes(gzip.compress(completed.stdout,mtime=0))
    value=running(path)
    value.update(status='complete',trace_local=str(trace.relative_to(root)),export_local=str(path.relative_to(root)),
        native_before=result['native_before'],native_after=result['native_after'])
    text=json.dumps(value,indent=2).replace(str(root),'$REPO').replace(str(Path.home()),'$HOME')
    return json.loads(text)
