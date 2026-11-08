"""Socket-pressure sampling without root privileges or changes to the host network."""
from collections import Counter
import subprocess
import threading
import time


def endpoint(value):
    if not value.startswith('127.0.0.1.'):return None
    try:return int(value.rsplit('.',1)[1])
    except ValueError:return None


class SocketSampler:
    def __init__(self,pid,listen,targets,exclusive):
        self.pid=pid;self.listen=listen;self.targets=set(targets);self.exclusive=set(exclusive)
        self.rows=[];self.lock=threading.Lock();self.done=threading.Event();self.error=None
        self.proxy_seen=set();self.fixture_seen=set();self.previous=None
        self.thread=threading.Thread(target=self.run,daemon=True)
    def add_target(self,port):
        with self.lock:self.targets.add(port)
    def capture(self,mark=None):
        with self.lock:
            sent=time.monotonic()
            owned=subprocess.run(['lsof','-nP','-a','-p',str(self.pid),'-iTCP','-F','fnT'],capture_output=True,text=True,timeout=3)
            if owned.returncode not in [0,1]:raise RuntimeError('lsof failed: '+owned.stderr)
            tuples=set();outbound=set();states=Counter();fds=0;name=None
            for line in owned.stdout.splitlines():
                if line.startswith('f'):fds+=1
                elif line.startswith('n'):
                    name=line[1:];tuples.add(name)
                    if '->' in name and not name.split('->')[0].endswith(':'+str(self.listen)):outbound.add(name.replace(':','.').replace('->','>'))
                elif line.startswith('TST='):states[line[4:]]+=1
            raw=subprocess.check_output(['netstat','-an','-p','tcp'],text=True,timeout=3)
            tcp=Counter();fixture=set();exclusive=set()
            ephemeral=set()
            for line in raw.splitlines():
                fields=line.split()
                if len(fields)<6 or fields[0]!='tcp4':continue
                left,right,state=fields[3:6];a,b=endpoint(left),endpoint(right)
                if a is None or b is None:continue
                tcp[state]+=1
                for port in [a,b]:
                    if 49152<=port<=65535:ephemeral.add(port)
                if b in self.targets:key=left+'>'+right;target=b
                elif a in self.targets:key=right+'>'+left;target=a
                else:continue
                fixture.add(key)
                if target in self.exclusive:exclusive.add(key)
            proxy=outbound|exclusive;at=time.monotonic();dt=at-self.previous if self.previous else None
            new_proxy=proxy-self.proxy_seen;new_fixture=fixture-self.fixture_seen
            row=dict(at=at,mark=mark,seconds_since_previous=dt,proxy_tcp_fds=fds,proxy_open_tcp_sockets=len(tuples),
                proxy_open_outbound_connections=len(outbound),proxy_fd_states=dict(states),loopback_tcp4_states=dict(tcp),
                loopback_time_wait=tcp['TIME_WAIT'],proxy_new_observed_lower_bound=len(new_proxy) if dt else 0,
                loopback_ephemeral_ports_observed=len(ephemeral),
                fixture_new_observed_upper_envelope=len(new_fixture) if dt else 0,
                proxy_new_observed_lower_bound_per_s=len(new_proxy)/dt if dt else None,
                fixture_new_observed_upper_envelope_per_s=len(new_fixture)/dt if dt else None,
                sampling_seconds=at-sent)
            self.proxy_seen.update(proxy);self.fixture_seen.update(fixture);self.previous=at;self.rows.append(row)
            return row
    def start(self):
        self.capture('socket-start');self.thread.start();return self
    def run(self):
        try:
            while not self.done.wait(1):self.capture()
        except BaseException as error:self.error=repr(error)
    def stop(self):
        self.done.set();self.thread.join(8)
        if self.thread.is_alive():raise RuntimeError('socket sampler did not stop')
    def report(self):
        return dict(error=self.error,method='one-second lsof PID sockets plus loopback netstat TCP tuples, including TIME_WAIT',
            rate_limit='observed unique tuple creation rates, not exact connect syscall counts; subsecond reset/shared-store/control connections may be missed; fixture envelope is not a strict upper bound on unobserved attempts; no root tracing available',
            proxy_exclusive_destination_ports=sorted(self.exclusive),samples=self.rows)
