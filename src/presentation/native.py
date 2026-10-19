"""Capture actual loaded code exclusively through Prim inspection builtins."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import tempfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCE=HERE/'scenarios/jit.pr'


def matrix_reference(size):
    """Independent oracle for scenarios/jit.pr: weighted sum of C = A x B."""
    total=0
    for i in range(size):
        a=[(i*7+k*3)%23-11 for k in range(size)]
        for j in range(size):
            c=sum(a[k]*((k*5+j*11)%19-9) for k in range(size))
            total+=c*((i+2*j)%7+1)
    return total


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def capture_native(runtime,output):
    runtime=Path(runtime).resolve();output=Path(output);output.mkdir(parents=True,exist_ok=True)
    source_hash,binary_hash=digest(SOURCE),digest(runtime)
    with tempfile.TemporaryDirectory(prefix='native-',dir=output) as directory:
        command=[str(runtime),'--stats','--jit-log',str(SOURCE),'--','--inspect']
        result=subprocess.run(command,cwd=directory,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=20)
        if result.returncode:raise ValueError('机器码导出失败：'+result.stderr[-4000:])
        cwd=Path(directory)
        metadata=dict(line.split('=',1) for line in (cwd/'kernel.meta').read_text().splitlines())
        hex_text=(cwd/'kernel.hex').read_text();assembly=(cwd/'kernel.asm').read_text()
        raw=bytes.fromhex(hex_text)
        if metadata['origin']!='loaded-memory' or len(raw)!=int(metadata['size']):raise ValueError('机器码来源或字节数不一致')
        if int(metadata['calls_before'])!=0 or int(metadata['calls_after'])!=1:raise ValueError('编译与执行计数不符合协议')
        expected=matrix_reference(int(metadata['validation_size']))
        if int(metadata['validation_result'])!=expected:raise ValueError('本机验证结果错误')
        instructions=[]
        for line in assembly.splitlines():
            match=re.match(r'^([0-9a-fA-F]+)\s+((?:[0-9a-fA-F]{2}\s+)+)\s*(.*)$',line)
            if match:
                instructions.append({'address':match[1],'bytes':match[2].strip(),'instruction':match[3].strip()})
        if not instructions or int(instructions[0]['address'],16)!=int(metadata['address'],16):raise ValueError('反汇编入口地址不一致')
        decoded=b''.join(bytes.fromhex(row['bytes']) for row in instructions)
        if decoded!=raw:raise ValueError('反汇编字节流与 loaded-memory 机器码不一致')
        arithmetic=next((i for i,row in enumerate(instructions) if re.search(r'\bimul\b',row['instruction'])),0)
        excerpt=instructions[max(0,arithmetic-3):arithmetic+9]
        report={'available':True,'generated_at':datetime.now(timezone.utc).isoformat(),'host':platform.node(),
            'runtime':subprocess.run([str(runtime),'--version'],capture_output=True,text=True,check=True,timeout=2).stdout.strip(),
            'runtime_sha256':binary_hash,'source_sha256':source_hash,'metadata':metadata,
            'code_sha256':hashlib.sha256(raw).hexdigest(),'source':(cwd/'kernel-source.pr').read_text(),
            'llvm_ir':(cwd/'kernel.ll').read_text(),'hex':hex_text,'assembly':assembly,'instructions':instructions,
            'excerpt':excerpt,'validation':{'size':int(metadata['validation_size']),'expected':expected,'actual':int(metadata['validation_result']),'bytes_verified':True},
            'command':command,'stderr':result.stderr}
    if digest(SOURCE)!=source_hash or digest(runtime)!=binary_hash:raise ValueError('导出期间源码或执行程序改变，拒绝发布')
    temporary=output/'native.pending.json';temporary.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');temporary.replace(output/'native.json')
    return report


if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser(description='通过 builtin 导出真实 JIT 机器码')
    parser.add_argument('--runtime',type=Path,default=ROOT/'build/prim-rs/prim')
    parser.add_argument('--output',type=Path,default=ROOT/'build/presentation/evidence')
    args=parser.parse_args();record=capture_native(args.runtime,args.output)
    print(json.dumps({'origin':record['metadata']['origin'],'bytes':record['metadata']['size'],'instructions':len(record['instructions']),'validation':record['validation']},ensure_ascii=False))
