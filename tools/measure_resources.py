#!/usr/bin/env python3
"""在运行该脚本的 Linux 主机测量 daemon；不将宿主机结果当作目标板结果。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import sys
import tempfile
import threading
import time

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tests'))
from test_e2e import Daemon, fixture
from test_stream import stream, unread_client


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
    parser.add_argument('--warmup',type=int,default=2)
    parser.add_argument('--baseline-binary',type=Path)
    args=parser.parse_args()
    assert args.seconds>=2 and args.warmup>=0
    results=[]
    binaries=[('current',args.binary)]
    if args.baseline_binary:binaries.insert(0,('baseline',args.baseline_binary))
    for label,binary in binaries:
      for count,trace,client in ((1,False,'none'),(300,False,'none'),(3000,False,'none'),(300,False,'sse'),(300,True,'sse'),(300,True,'slow')):
        with tempfile.TemporaryDirectory(prefix='procface-profile-') as folder:
            root=Path(folder)/'proc';root.mkdir();fixture(root,count)
            if trace:(root/'100/sched').write_text('x'*(200*1024))
            daemon=Daemon(str(binary.resolve()),root)
            response=None;worker=None;stop=threading.Event();received=[0];errors=[]
            try:
                if trace:assert daemon.request('/api/v1/trace','POST',{'pid':100})[0]==202
                if client=='sse':
                    response=stream(daemon)
                    def consume():
                        try:
                            while not stop.is_set():
                                chunk=response.read1(65536)
                                if not chunk:
                                    if not stop.is_set():errors.append('unexpected_eof')
                                    break
                                received[0]+=len(chunk)
                        except Exception as error:
                            if not stop.is_set():errors.append(type(error).__name__)
                    worker=threading.Thread(target=consume,daemon=True);worker.start()
                elif client=='slow':response=unread_client(daemon,'/api/v1/stream')
                started=time.monotonic();first=None;rss_peak=0
                seconds=max(args.seconds,15) if client=='slow' else args.seconds
                while time.monotonic()-started<args.warmup+seconds:
                    # 原子替换，避免采集器读到截断中的空 uptime。
                    temp=root/'uptime.next';temp.write_text(f'{10+time.monotonic()-started} 0\n');temp.replace(root/'uptime')
                    now=usage(daemon.process.pid)
                    if time.monotonic()-started>=args.warmup:
                        if first is None:first=now;start=time.monotonic();received_start=received[0]
                        rss_peak=max(rss_peak,now['rss_bytes'])
                    time.sleep(.2)
                last=usage(daemon.process.pid);elapsed=time.monotonic()-start
                health=daemon.data('/api/v1/health')
                results.append({'build':label,'client':client,'process_count':count,'trace':trace,'duration_seconds':elapsed,
                    **last,'cpu_percent_one_core':100*(last['cpu_ticks']-first['cpu_ticks'])/os.sysconf('SC_CLK_TCK')/elapsed,
                    'rss_sampled_peak_bytes':rss_peak,'received_bytes':received[0]-received_start,
                    'received_bytes_per_second':(received[0]-received_start)/elapsed,'stream_errors':errors.copy(),
                    'performance':health.get('performance'),
                    'history_bytes':health['memory_bytes'],'trace_history_bytes':health['trace_memory_bytes'],
                    'dropped_batches':health['dropped_batches'],'skipped_rounds':health['skipped_rounds']})
                print(f'{label}: {count} processes, trace={trace}, client={client}: measured',flush=True)
            finally:
                stop.set();daemon.close()
                if worker:worker.join(timeout=25)
                if response:response.close()
                if worker and worker.is_alive():raise RuntimeError('SSE reader did not stop')
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps({'environment':platform.platform(),'machine':platform.machine(),
        'binaries':{label:{'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'bytes':binary.stat().st_size} for label,binary in binaries},
        'warmup_seconds':args.warmup,'measurement_seconds':args.seconds,'slow_min_seconds':15,
        'sampling_seconds':1,'fixture':True,'results':results},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(results,ensure_ascii=False))


if __name__=='__main__':main()
