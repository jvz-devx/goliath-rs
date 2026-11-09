"""Reuse E24's fixed soak, changing only evidence destinations and window ownership."""
import ast
import gzip
import hashlib
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / '.local/evidence/E25'
DERIVED = EVIDENCE / 'soak-derived'
DATA = ROOT / 'docs/experiments/data/E25'


def configure(data_dir):
    """Keep E25 defaults; E28 evidence stays in its own private namespace."""
    global DATA,EVIDENCE,DERIVED
    DATA=Path(data_dir).resolve()
    DATA.relative_to(ROOT)
    EVIDENCE=ROOT/'.local/evidence/E25'
    DERIVED=EVIDENCE/'soak-derived'
    if DATA != ROOT/'docs/experiments/data/E25':
        # Each Linux host keeps its own private namespace: E28 the workstation, E32 the VM host.
        experiment=next((name for name in ['E28','E32'] if DATA.is_relative_to(ROOT/'docs/experiments/data'/name)),None)
        if not experiment:
            DATA.relative_to(ROOT/'docs/experiments/data/E28')
        EVIDENCE=ROOT/'.local/evidence'/experiment/'soak'
        DERIVED=EVIDENCE/'soak-derived'
    for directory in [DATA,EVIDENCE,DERIVED]:directory.mkdir(parents=True,exist_ok=True)


def load(name,path):
    loader = importlib.machinery.SourceFileLoader(name,str(path))
    spec = importlib.util.spec_from_loader(name,loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


storage = load('e25_soak_storage',ROOT/'scripts/e25-memory')


def read(path):
    if not path.exists():
        path = path.with_suffix(path.suffix+'.gz')
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == '.gz' else raw)


def digest(path):
    total = 0
    result = hashlib.sha256()
    opener = gzip.open if path.suffix == '.gz' else open
    with opener(path,'rb') as source:
        for chunk in iter(lambda:source.read(1024*1024),b''):
            result.update(chunk)
            total += len(chunk)
    return dict(path=str(path.relative_to(ROOT)),sha256=result.hexdigest(),bytes=total,
        hash_scope='decompressed bytes' if path.suffix == '.gz' else 'file bytes')


class Relocate(ast.NodeTransformer):
    def __init__(self,owner='E25'):
        self.owner = owner

    def visit_Constant(self,node):
        if isinstance(node.value,str):
            value = node.value.replace('.local/evidence/E24',str(EVIDENCE.relative_to(ROOT)))
            value = value.replace('docs/experiments/data/E24',str(DERIVED.relative_to(ROOT)))
            return ast.copy_location(ast.Constant(value),node)
        return node

    def visit_keyword(self,node):
        node = self.generic_visit(node)
        if node.arg == 'owner' and isinstance(node.value,ast.Constant) and node.value.value == 'E24':
            node.value = ast.copy_location(ast.Constant(self.owner),node.value)
        return node


def execute(script,arguments,tag,owner='E25'):
    source = ROOT / 'scripts' / script
    tree = ast.fix_missing_locations(Relocate(owner).visit(ast.parse(source.read_text())))
    executed = ast.unparse(tree)+'\n'
    path = EVIDENCE / (tag+'-'+script+'-executed.py')
    if path.exists():
        raise RuntimeError('executed soak helper already exists')
    path.write_text(executed)
    namespace = dict(__name__='e25_'+script.replace('-','_'),__file__=str(source))
    exec(compile(tree,str(path),'exec'),namespace)
    previous = sys.argv
    try:
        sys.argv = [script,*map(str,arguments)]
        namespace['main']()
    finally:
        sys.argv = previous
    return dict(original=digest(source),executed=digest(path))


def closures(folder):
    events = []
    for path in sorted((folder/'tiles').glob('*-lease.jsonl*')):
        from e28_log_integrity import read_trace
        rows,_=read_trace(path)
        for row in rows:
            clock = row.get('data',{})
            if row.get('event') in ['authority-stop','clock-anomaly'] or (
                row.get('event') == 'clock-metrics' and any(clock.get('closures_by_cause',{}).values())):
                events.append(dict(path=str(path.relative_to(ROOT)),record=row))
                break
    return events


def publication_check(value):
    if isinstance(value,dict):
        for key,item in value.items():
            if key == 'outside' and isinstance(item,list) and any(row.get('name') != 'outside' for row in item):
                raise ValueError('outside host process name was not redacted')
            publication_check(item)
    elif isinstance(value,list):
        for item in value:
            publication_check(item)
    elif isinstance(value,str):
        if str(Path.home()) in value or '/Users/' in value or '/home/' in value:
            raise ValueError('private path in published soak rows')


def classify(native,store,authority_events):
    proxy_pass = bool(native['memory_endpoint_coverage'] and native['memory']['proxy']['growth_fraction'] < .1)
    d9 = bool(native['capacity_diagnostic_pass'] and native['noise_valid'])
    rss = bool(native['verdict'] == 'passed' and proxy_pass and store['qualified'] and not authority_events)
    return dict(native_soak_verdict=native['verdict'],non_proxy_memory_pass=native['memory_pass'],
        memory_pass=bool(native['memory_pass'] and proxy_pass),proxy_rss_line_pass=proxy_pass,
        rss_soak_lines_pass=rss,d9_valid=d9,d9_memory_qualified=bool(rss and d9),m4_qualified=False,
        verdict='rss-lines-passed' if rss else 'rss-lines-failed')


def readback(name):
    commands = [
        ('e24-analyse',['--name',name]),
        ('e24-clock-soak',['--name',name]),
        ('e24-store-soak',['--name',name,'--planning-rate','72']),
        ('e24-soak-result',['--name',name])]
    executed = [execute(script,arguments,name) for script,arguments in commands]
    native = read(DERIVED/(name+'-result.json'))
    store = read(DERIVED/(name+'-store-diagnostics.json'))
    measurement = read(EVIDENCE/name/(name+'.json'))
    cell = next(iter(measurement['cells'].values()))
    soak = read(EVIDENCE/name/(name+'-soak.json.gz'))
    for value in [native,store,measurement,soak]:
        publication_check(value)
    events = closures(EVIDENCE/name)
    data = dict(native)
    data.update(classify(native,store,events))
    whole_path=EVIDENCE/(name+'-whole-window.json')
    if whole_path.exists() or whole_path.with_suffix('.json.gz').exists():
        whole=read(whole_path)
        data.update(whole_window_d12={key:value for key,value in whole.items() if key!='samples'},
            whole_window_evidence=digest(whole_path if whole_path.exists() else whole_path.with_suffix('.json.gz')))
        from e28_window import minute_rss
        origin=soak['memory'][0]['processes']['store']['at']-soak['memory'][0]['elapsed_seconds']
        minutes=minute_rss(whole['samples'],origin)
        data['every_process_minute_rss']=storage.save(DATA/(name+'-every-process-minute-rss.json'),minutes)
        data['every_process_minute_rss_covered']=all(row['endpoint_covered'] for row in minutes)
        data['rss_soak_lines_pass']=data['rss_soak_lines_pass'] and whole['valid']
        data['verdict']='rss-lines-passed' if data['rss_soak_lines_pass'] else 'rss-lines-failed'
        data['d9_memory_qualified']=data['d9_memory_qualified'] and whole['valid']
    elif EVIDENCE in [ROOT/'.local/evidence'/experiment/'soak' for experiment in ['E28','E32']]:
        raise RuntimeError('E28 requires whole-window D12 evidence, including recovery and restoration')
    data.update(kind='E25-full-soak-readback',proxy_rss_judged=True,
        store_diagnostics=store,executed_readbacks=executed,
        authority_events=events,proxy=cell.get('proxy'),histogram=cell.get('histogram'),
        limiter=cell.get('limiter_window'),native_result=digest(DERIVED/(name+'-result.json')))
    data['memory']['proxy'].update(judged=True,under_10_percent=data['proxy_rss_line_pass'])
    data['inputs'] = {path:digest(ROOT/path) for path in native['inputs']}
    data['store_diagnostics']['inputs'] = {path:digest(ROOT/path) for path in store['inputs']}
    data['interpretation']='RSS and availability lines are reported separately from unchanged D9 gates; reviewer reproduction is required'
    data['raw']=storage.save(DATA/(name+'-raw.json'),measurement)
    data['memory_series']=storage.save(DATA/(name+'-memory-series.json'),soak['memory'])
    data['soak_audits']=storage.save(DATA/(name+'-audits.json'),soak)
    data['evidence']=[digest(path) for path in sorted((EVIDENCE/name).rglob('*')) if path.is_file()]
    data['derived_evidence']=[digest(path) for path in sorted(DERIVED.glob(name+'-*')) if path.is_file()]
    return storage.save(DATA/(name+'-e25-result.json'),data)
