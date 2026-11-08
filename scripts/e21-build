#!/usr/bin/env python3
"""Build and freeze E21's byte-proved tile patch in the single owned target dir."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import e11_support as support

ROOT=support.ROOT
DATA=ROOT/'docs/experiments/data/E21'
SOURCE=ROOT/'.local/src/horizon'


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name',required=True)
    parser.add_argument('--resume-from',help='reuse completed gates from an identical-source interrupted runner, never rerun Cargo')
    args=parser.parse_args()
    if (ROOT/'.local/quiet-request').exists():raise RuntimeError('no build during a measurement window')
    target=ROOT/'.local/bin/horizon-e21-fc2868b2b'/args.name
    result_path=DATA/(args.name+'-build.json')
    if target.exists() or result_path.exists():raise RuntimeError('use a fresh candidate name')
    proof=json.loads((DATA/'stack-proof.json').read_text())
    if proof['source_equality']!='passed' or not proof['patch10_sha256']:raise RuntimeError('patch10 source proof required')
    internal=json.loads((ROOT/'.local/bin/meridian-e18/flat09-tiles/canonical.json').read_text())['internal_crates']['files']
    settings={key:value for key,value in os.environ.items() if key not in ['CI','CARGO_TARGET_DIR'] and not key.startswith('CARGO_PROFILE_')}
    settings.update(CARGO_INCREMENTAL='0',CARGO_PROFILE_RELEASE_LTO='thin',CARGO_PROFILE_RELEASE_CODEGEN_UNITS='1',CARGO_BUILD_JOBS='6')
    result=dict(status='running',source=str(SOURCE.relative_to(ROOT)),profile='release',incremental=False,lto='thin',codegen_units=1,
        source_proof_sha256=sha(DATA/'stack-proof.json'),patch10_sha256=proof['patch10_sha256'],commands=[])
    if args.resume_from:
        previous=DATA/(args.resume_from+'-build.json')
        prior=json.loads(previous.read_text())
        if prior['source_proof_sha256']!=result['source_proof_sha256'] or prior['patch10_sha256']!=result['patch10_sha256']:
            raise RuntimeError('cannot resume gates from different source')
        result['resumed_from']=dict(path=str(previous.relative_to(ROOT)),sha256=sha(previous),status=prior['status'])
        result['commands']=[dict(row,reused_from=args.resume_from) for row in prior['commands'] if row['exit']==0]
    began=time.monotonic()
    def run(label,command):
        completed=[row for row in result['commands'] if row['label']==label and row['command']==command and row['exit']==0]
        if completed:
            if label=='unit' and not any('3 passed;' in value for value in completed[0]['test_results']):
                raise RuntimeError('cannot reuse an incomplete unit-test gate')
            return
        if (ROOT/'.local/quiet-request').exists():raise RuntimeError('quiet request appeared before '+label)
        print('E21_GATE '+label,flush=True)
        log=ROOT/'.local/evidence/E21'/(args.name+'-'+label+'.log')
        log.parent.mkdir(parents=True,exist_ok=True)
        started=time.monotonic()
        with log.open('w') as stream:
            completed=subprocess.run(command,cwd=SOURCE,env=settings,stdout=stream,stderr=subprocess.STDOUT)
        text=log.read_text(errors='replace')
        row=dict(label=label,command=command,exit=completed.returncode,seconds=time.monotonic()-started,
            test_results=re.findall(r'test result: .*',text))
        result['commands'].append(row)
        (DATA/(args.name+'-'+label+'.log.gz')).write_bytes(gzip.compress(support.sanitize(text).encode(),mtime=0))
        print(json.dumps(row),flush=True)
        if completed.returncode or label=='unit' and not any('3 passed;' in value for value in row['test_results']):
            print('\n'.join(text.splitlines()[-60:]),flush=True)
            raise RuntimeError('gate failed: '+label)
    try:
        for path,digest in internal.items():
            if sha(ROOT/path)!=digest:raise RuntimeError('internal FINAL dependency changed: '+path)
        for path,digest in proof['files'].items():
            if sha(SOURCE/path)!=digest:raise RuntimeError('source changed after proof: '+path)
        run('fmt',['cargo','fmt','--all','--check'])
        run('unit',['cargo','test','--locked','--release','--no-default-features','-p','horizon-client','--lib','game::dedicated::tile_cut::tests::'])
        run('clippy',['cargo','clippy','--locked','--release','--no-default-features','-p','horizon-client','--lib','--','-D','warnings'])
        run('release',['cargo','build','--locked','--release','--no-default-features','-p','horizon-client','--bin','horizon-client'])
        for path,digest in proof['files'].items():
            if sha(SOURCE/path)!=digest:raise RuntimeError('source changed during build: '+path)
        for path,digest in internal.items():
            if sha(ROOT/path)!=digest:raise RuntimeError('internal dependency changed during build: '+path)
        parent=ROOT/'.local/bin/meridian-e18/reactor-1'
        candidate=json.loads((parent/'candidate.json').read_text())
        for name,digest in candidate['binaries'].items():
            if sha(parent/name)!=digest:raise RuntimeError('frozen reactor1 changed: '+name)
        target.mkdir(parents=True)
        for name in ['meridian','meridian-sidecar','horizon-bots','goliath-store','e4-client','LEASES']:
            shutil.copy2(parent/name,target/name)
        shutil.copy2(SOURCE/'target/release/horizon-client',target/'horizon-client')
        canonical=json.loads((parent/'canonical.json').read_text())
        patch_record=dict(path='patches/horizon-10-warm-tick.patch',sha256=proof['patch10_sha256'])
        for manifest in [candidate,canonical]:
            manifest['projects']['horizon']['patches'].append(patch_record)
            manifest['binaries']['horizon-client']=sha(target/'horizon-client')
            manifest['kind']='E21-warm-tick-candidate-not-capacity-accepted'
            manifest['source_equality']='passed'
            manifest['e21_source_files']=proof['files']
        (target/'canonical.json').write_text(json.dumps(canonical,indent=2)+'\n')
        (target/'candidate.json').write_text(json.dumps(candidate,indent=2)+'\n')
        result.update(status='complete',directory=str(target.relative_to(ROOT)),binaries={name:sha(target/name) for name in candidate['binaries']})
        support.save(target/'build.json',result)
    except BaseException as error:
        result.update(status='failed',error=repr(error));raise
    finally:
        result['seconds']=time.monotonic()-began
        support.save(result_path,result)


if __name__=='__main__':main()
