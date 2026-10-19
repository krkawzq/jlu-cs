"""Reproducible small demonstrations; never replace measured results with targets."""
from __future__ import annotations
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from native import matrix_reference

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
FILES=[HERE/'scenarios'/name for name in ('orders.csv','orders.pr','jit.pr')]


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def metadata():
    cpu=next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),'unknown')
    return {'host':platform.node(),'cpu':cpu,'platform':platform.platform(),'logical_cpus':os.cpu_count(),
        'python_version':sys.version,'python_implementation':platform.python_implementation(),
        'python_jit_enabled':bool(getattr(getattr(sys,'_jit',None),'is_enabled',lambda:False)())}


def run(command,cwd):
    started=time.perf_counter_ns()
    result=subprocess.run(command,cwd=cwd,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=40)
    wall=(time.perf_counter_ns()-started)/1_000_000
    if result.returncode:raise ValueError(f'测量失败（退出码 {result.returncode}）：{result.stderr[-4000:]}')
    values={}
    for line in result.stdout.splitlines():
        match=re.fullmatch(r'([a-z_0-9]+)=(-?[0-9.]+)',line)
        if match:values[match[1]]=float(match[2])
    stats={key:float(value) for key,value in re.findall(r'([a-z_]+)=([0-9.]+)',result.stderr)}
    return {'values':values,'stats':stats,'process_ms':wall,'stdout':result.stdout,'stderr':result.stderr,'command':[str(x) for x in command]}


def summarize(samples):
    result={key:statistics.median(s['values'][key] for s in samples) for key in samples[0]['values']}
    result['process_ms']=statistics.median(s['process_ms'] for s in samples)
    result['warm_per_call_ms']=result['warm_total_ms']/result['repeats']
    result['compile_ms']=statistics.median(s['stats'].get('compile_ms',0) for s in samples)
    result['warm_per_call_range_ms']=[min(s['values']['warm_total_ms']/s['values']['repeats'] for s in samples),max(s['values']['warm_total_ms']/s['values']['repeats'] for s in samples)]
    result['samples']=samples
    return result


def measure(runtime:Path,output:Path,quick=False):
    runtime=Path(runtime).resolve();output=Path(output);output.mkdir(parents=True,exist_ok=True)
    source_hashes={str(p.relative_to(ROOT)):digest(p) for p in FILES}
    binary_hash=digest(runtime)
    machine=metadata()
    expected={'rows':0,'accepted':0,'total_cents':0,**{f'group_{i}':0 for i in range(8)}}
    for category,quantity,price,state in csv.reader((HERE/'scenarios/orders.csv').read_text().splitlines()):
        category,quantity,price,state=map(int,(category,quantity,price,state));expected['rows']+=1
        if state==1 and quantity>=3:
            amount=quantity*price;expected['accepted']+=1;expected['total_cents']+=amount;expected[f'group_{category}']+=amount
    repeats=3 if quick else 5
    size=80
    numerical_result=matrix_reference(size)
    results={'jit':{'interpreter':[],'jit':[]},'orders':{'interpreter':[],'jit':[]}}
    with tempfile.TemporaryDirectory(prefix='benchmark-',dir=output) as directory:
        cwd=Path(directory);shutil.copyfile(HERE/'scenarios/orders.csv',cwd/'orders.csv')
        for iteration in range(repeats):
            for scenario,modes in results.items():
                order=list(modes) if iteration%2==0 else list(reversed(modes))
                for mode in order:
                    # No --fuel: it disables the JIT scalar path; run() has its own timeout.
                    command=[str(runtime),'--stats','--jit-log']
                    if mode=='interpreter':command.append('--jit=off')
                    command.append(str(HERE/f'scenarios/{scenario}.pr'))
                    sample=run(command,cwd)
                    if scenario=='orders':
                        for key,value in expected.items():
                            if sample['values'].get(key)!=value:raise ValueError(f'{mode} 的 {key} 校验失败')
                        if sample['values'].get('checksum')!=expected['total_cents']*5:raise ValueError('重复调用结果不一致')
                    else:
                        if sample['values'].get('result')!=numerical_result or sample['values'].get('checksum')!=numerical_result*5:
                            raise ValueError('矩阵乘法结果与独立 Python 参考不一致')
                        if sample['values'].get('size')!=size:raise ValueError('矩阵规模不一致')
                    if mode=='jit' and sample['stats'].get('native_calls',0)==0:raise ValueError('未观察到本机入口')
                    if mode=='interpreter' and sample['stats'].get('native_calls',0)!=0:raise ValueError('解释对照进入了本机入口')
                    results[scenario][mode].append(sample)
            print(f'测量 {iteration+1}/{repeats} 完成',flush=True)
    if source_hashes!={str(p.relative_to(ROOT)):digest(p) for p in FILES} or binary_hash!=digest(runtime):
        raise ValueError('测量期间源码或执行程序发生变化，结果未发布，请重试')
    version=subprocess.run([str(runtime),'--version'],capture_output=True,text=True,check=True,timeout=2).stdout.strip()
    report={'available':True,'generated_at':datetime.now(timezone.utc).isoformat(),'machine':machine,'runtime':version,'runtime_sha256':binary_hash,
        'source_sha256':source_hashes,'dataset':{'kind':'synthetic orders','seed':20261019,'rows':expected['rows'],'bytes':(HERE/'scenarios/orders.csv').stat().st_size,'integer_unit':'cents'},
        'protocol':{'independent_processes_per_mode':repeats,'warm_calls_per_process':5,'aggregation':'median','prim_timer_resolution_ms':1,'python_timer':'perf_counter_ns','validation':'independent matrix oracle; order rows, category totals and checksums match the harness reference','jit_trigger':'manual, function-level, no automatic hotspots','inspection_timed':False},
        'numerical_workload':{'name':'整数矩阵乘法 C = A × B 与加权求和','size':size,'multiply_adds_per_call':size**3,'element_rule':'A[i][k]=(7i+3k)%23-11, B[k][j]=(5k+11j)%19-9','reference_result':numerical_result,'checksum':numerical_result*5,'integer_semantics':'checked i64'},
        'expected':expected,'results':{scenario:{mode:summarize(samples) for mode,samples in modes.items()} for scenario,modes in results.items()}}
    path=output/'benchmarks.json';temporary=output/'benchmarks.pending.json';temporary.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');temporary.replace(path)
    return report


def main():
    parser=argparse.ArgumentParser(description='Prim 课堂演示实测；建议在 CPU worker 运行')
    parser.add_argument('--runtime',type=Path,default=ROOT/'build/prim-rs/prim');parser.add_argument('--output',type=Path,default=ROOT/'build/presentation/evidence')
    parser.add_argument('--quick',action='store_true');args=parser.parse_args()
    report=measure(args.runtime,args.output,args.quick)
    for scenario,modes in report['results'].items():
        print(scenario,{mode:{k:round(v[k],3) for k in ('first_ms','warm_per_call_ms','process_ms')} for mode,v in modes.items()})
    print(args.output/'benchmarks.json')


if __name__=='__main__':main()
