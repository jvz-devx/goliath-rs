#!/usr/bin/env python3
"""Coordinate the D9-matched plain-09 staircase, never the cluster's W2 run."""
import argparse
import importlib.machinery
import importlib.util
from pathlib import Path
from e11_support import ROOT
from e17_d9_baseline import cell


def main():
    loader=importlib.machinery.SourceFileLoader('d9_baseline_coordinator',str(ROOT/'scripts/e17-flat09-baseline'))
    spec=importlib.util.spec_from_loader(loader.name,loader)
    base=importlib.util.module_from_spec(spec);loader.exec_module(base)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cell',type=int)
    parser.add_argument('--repeat',type=int,default=0)
    parser.add_argument('--out',type=Path)
    parser.add_argument('--data-dir',type=Path,default=ROOT/'docs/experiments/data/E17/d9-plain09-baseline')
    parser.add_argument('--resume',action='store_true')
    args=parser.parse_args()
    base.DATA=args.data_dir.resolve()
    if not base.DATA.is_relative_to(ROOT/'docs/experiments/data/E17'):
        parser.error('data directory must remain under E17 evidence')
    base.REQUEST_PREFIX='d9-plain09-baseline'
    original=base.provenance
    def provenance():
        result=original()
        for name in ['e17-d9-baseline','e17_d9_baseline.py','e18_d9_layout.py']:
            result[name+'_sha256']=base.digest(ROOT/'scripts'/name)
        return result
    base.provenance=provenance
    if args.cell:
        if not args.out: parser.error('--cell requires --out')
        cell(args,base)
    else:
        args.entrypoint=Path(__file__).resolve();args.start=300;args.minimum_count=300;args.step_up=50;args.kind='E17-D9-plain09-baseline'
        args.workload={'layout':'D9','world_seed':12345,'cohort_seed':12345,'witness_seed':18,'pigs':256,
            'pigs_per_square':64,'interior_square_centers':'(+/-1024,+/-1024)','interior_fraction':.9,
            'crossing_fraction':'floor(N/10)','view_distance':3,'speed':4,'warmup_seconds':30,
            'measurement_seconds':60,'server_cpu_cap':4,'aggregate_five_bot_process_cap':1,
            'geometry_owner':'scripts/e18_d9_layout.py::plan'}
        base.run(args)


if __name__=='__main__': main()
