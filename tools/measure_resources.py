#!/usr/bin/env python3
"""在运行该脚本的 Linux 主机测量 daemon；不将宿主机结果当作目标板结果。"""
import argparse
import json
import os
from pathlib import Path
import platform
import sys
import tempfile
import time

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tests'))
from test_e2e import Daemon, fixture


def usage(pid):
    status={line.split(':',1)[0]:line.split(':',1)[1].strip() for line in Path(f'/proc/{pid}/status').read_text().splitlines() if ':' in line}
    stat=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
    return {'rss_bytes':int(status['VmRSS'].split()[0])*1024,
            'peak_bytes':int(status['VmHWM'].split()[0])*1024,
            'cpu_ticks':int(stat[11])+int(stat[12]),'threads':int(status['Threads'])}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--seconds',type=int,default=4)
    args=parser.parse_args()
    assert args.seconds>=2
    results=[]
    for count,trace in ((1,False),(300,False),(3000,False),(300,True)):
        with tempfile.TemporaryDirectory(prefix='procface-profile-') as folder:
            root=Path(folder)/'proc';root.mkdir();fixture(root,count)
            if trace:(root/'100/sched').write_text('x'*(200*1024))
            daemon=Daemon(str(args.binary.resolve()),root)
            try:
                if trace:assert daemon.request('/api/v1/trace','POST',{'pid':100})[0]==202
                first=usage(daemon.process.pid);start=time.monotonic()
                while time.monotonic()-start<args.seconds:
                    (root/'uptime').write_text(f'{10+time.monotonic()-start} 0\n')
                    time.sleep(.2)
                last=usage(daemon.process.pid);elapsed=time.monotonic()-start
                health=daemon.data('/api/v1/health')
                results.append({'process_count':count,'trace':trace,'duration_seconds':elapsed,
                    **last,'cpu_percent_one_core':100*(last['cpu_ticks']-first['cpu_ticks'])/os.sysconf('SC_CLK_TCK')/elapsed,
                    'history_bytes':health['memory_bytes'],'trace_history_bytes':health['trace_memory_bytes'],
                    'dropped_batches':health['dropped_batches'],'skipped_rounds':health['skipped_rounds']})
            finally:daemon.close()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps({'environment':platform.platform(),'machine':platform.machine(),
        'binary':str(args.binary.resolve()),'sampling_seconds':1,'fixture':True,'results':results},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(results,ensure_ascii=False))


if __name__=='__main__':main()
