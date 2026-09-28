#!/usr/bin/env python3
"""真实 procfs 的 PC 基线：每场景预热 60 秒、测量 30 秒；不代表目标板或浏览器开销。"""
import argparse, hashlib, json, os, platform, statistics, subprocess, sys, threading, time, urllib.request
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tests'))
from test_e2e import Daemon, TOKEN
from measure_resources import usage

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--with-clients-only',action='store_true')
    args=parser.parse_args();binary=args.binary.resolve();results=[]
    cpu=next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),'unknown')
    target=subprocess.Popen(['sleep','600'])
    try:
        for name in (('采集加单个SSE客户端','采集加SSE与Trace') if args.with_clients_only else ('采集无客户端','采集加单个SSE客户端','采集加SSE与Trace')):
            daemon=Daemon(str(binary),Path('/proc'),frontend_debug=False,origin='')
            received=[0];stream_errors=[];samples=[];response=None;worker=None;stop=threading.Event();disconnects=[]
            try:
                if name!='采集无客户端':
                    def read_stream():
                        while not stop.is_set():
                            opened=time.monotonic()
                            try:
                                with urllib.request.urlopen(urllib.request.Request(daemon.url+'/api/v1/stream',headers={'Authorization':'Bearer '+TOKEN}),timeout=20) as stream:
                                    while not stop.is_set():
                                        data=stream.readline()
                                        if not data:
                                            disconnects.append(time.monotonic()-opened);break
                                        received[0]+=len(data)
                                        if data.startswith(b'event: error'):stream_errors.append('SSE error event')
                            except Exception as error:
                                if not stop.is_set():stream_errors.append(str(error))
                            stop.wait(2)
                    worker=threading.Thread(target=read_stream,daemon=True);worker.start()
                if name=='采集加SSE与Trace':
                    assert daemon.request('/api/v1/trace','POST',{'pid':target.pid,'extended':True,'threads':True})[0]==202
                start=time.monotonic();baseline=None
                for second in range(90):
                    time.sleep(1);u=usage(daemon.process.pid);samples.append(u)
                    if second==59:baseline=(time.monotonic(),u,received[0])
                elapsed=time.monotonic()-baseline[0];last=samples[-1]
                health=daemon.data('/api/v1/health');current=daemon.data('/api/v1/current')
                counts={b['group']:{'samples':len(b['samples']),'processes':len(b['processes']),'complete':b['complete']} for b in current['batches']}
                row={'scenario':name,'duration_s':time.monotonic()-start,'steady_seconds':elapsed,'cpu_percent_one_core':100*(last['cpu_ticks']-baseline[1]['cpu_ticks'])/os.sysconf('SC_CLK_TCK')/elapsed,'rss_end_bytes':last['rss_bytes'],'rss_sampled_peak_bytes':max(s['rss_bytes'] for s in samples),'rss_hwm_bytes':last['peak_bytes'],'steady_rss_min_bytes':min(s['rss_bytes'] for s in samples[60:]),'threads':last['threads'],'sse_bytes_per_second':(received[0]-baseline[2])/elapsed,'stream_errors':stream_errors.copy(),'disconnect_after_seconds':disconnects.copy(),'batches':counts,'health':health}
                # 不保存目标进程的具体内容或凭据。
                row['health']['trace']={'state':health['trace']['state']}
                if name!='采集无客户端':assert row['sse_bytes_per_second']>0,'测量区间未收到流量'
                results.append(row);print(json.dumps(row,ensure_ascii=False),flush=True)
            finally:
                stop.set();daemon.close()
                if worker:worker.join(timeout=5)
                if response:response.close()
            report={'environment':platform.platform(),'cpu':cpu,'logical_cpus':os.cpu_count(),'mem_total':Path('/proc/meminfo').read_text().splitlines()[0],'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'binary_bytes':binary.stat().st_size,'sampling_seconds':1,'warmup_seconds':60,'measurement_seconds':30,'trace_target':'sleep，extended+threads，非敏感普通版','fixture':False,'results':results}
            args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    finally:
        target.terminate();target.wait()

if __name__=='__main__':main()
