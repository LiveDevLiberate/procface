#!/usr/bin/env python3
"""Linux 端到端验证；只依赖 Python 标准库与构建好的 procface。"""
import argparse
import json
import os
import re
from pathlib import Path
import signal
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.request
import urllib.error
from compact_wire import decode

TOKEN = "test-token-01234567890123456789"
ROOT = Path(__file__).resolve().parents[1]


def stat(pid, start=123, user=10, rss=7):
    return f"{pid} (worker ) name) R 1 0 0 0 0 0 0 0 0 0 {user} 20 0 0 0 0 2 0 {start} 4096 {rss}\n"


def fixture(path, count=1):
    (path / "net").mkdir()
    (path / "uptime").write_text("10.0 0\n")
    (path / "stat").write_text("cpu 10 0 10 80 0 0 0 0\ncpu0 10 0 10 80 0 0 0 0\nctxt 100\nprocesses 200\nprocs_running 2\nprocs_blocked 0\n")
    (path / "meminfo").write_text("MemTotal: 1024 kB\nMemAvailable: 512 kB\nMemFree: 128 kB\nBuffers: 8 kB\nCached: 32 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n")
    (path / "vmstat").write_text("pgpgin 100\npgpgout 200\npswpin 0\npswpout 0\npgfault 400\npgmajfault 10\n")
    (path / "loadavg").write_text("0.10 0.20 0.30 1/20 123\n")
    (path / "pressure").mkdir()
    (path / "pressure/cpu").write_text("some avg10=1.00 avg60=2.00 avg300=3.00 total=100\nfull avg10=0.10 avg60=0.20 avg300=0.30 total=10\n")
    (path / "pressure/memory").write_text("some avg10=4.00 avg60=5.00 avg300=6.00 total=200\n")
    (path / "pressure/io").write_text("some avg10=7.00 avg60=8.00 avg300=9.00 total=300\n")
    (path / "interrupts").write_text("           CPU0 CPU1\n  1:       10   20 timer\n ERR         2    3\n")
    (path / "softirqs").write_text("                    CPU0       CPU1\nHI                    4          5\nTIMER                 6          7\n")
    (path / "diskstats").write_text("8 0 sda 1 0 4 6 2 0 8 9 0 10 11\n")
    (path / "net/dev").write_text("lo: 10 2 0 0 0 0 0 0 20 4 0 0 0 0 0 0\n")
    for pid in range(100, 100+count):
        p = path / str(pid)
        p.mkdir()
        (p / "stat").write_text(stat(pid))
        (p / "status").write_text(f"Name:\tworker\nUid:\t{os.geteuid()}\t{os.geteuid()}\t{os.geteuid()}\t{os.geteuid()}\n")
        (p / "io").write_text("read_bytes: 100\nwrite_bytes: 200\nsyscr: 10\nsyscw: 20\n")
        (p / "statm").write_text("1 1 0 0 0 0 0\n")
    (path / "self").symlink_to(path / "100", target_is_directory=True)


def cli(binary, *args, ok=True):
    p = subprocess.run([binary, *map(str,args)], capture_output=True, text=True, timeout=15)
    if ok:
        assert p.returncode == 0, p.stderr
    else:
        assert p.returncode != 0, p.stdout
    return p


def prometheus(text):
    """检查每个数值序列的声明、类型、名称与 label 语法。"""
    types={};helps=set();samples={}
    for line in text.splitlines():
        if line.startswith("# HELP "):
            name=line.split()[2];assert name not in helps;helps.add(name)
        elif line.startswith("# TYPE "):
            _,_,name,kind=line.split();assert name not in types;types[name]=kind
        else:
            match=re.fullmatch(r'(procface_[a-zA-Z0-9_]+)\{((?:[a-z_]+="(?:[^"\\\n]|\\[\\"n])*"(?:,|(?=})))*?)\} (-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)',line)
            assert match,line
            name=match[1];assert name in helps and name in types
            assert types[name] in ("counter","gauge")
            samples.setdefault(name,[]).append(line)
    return types,samples


class Daemon:
    def __init__(self, binary, root, *extra, frontend_debug=True, origin="http://localhost:8000"):
        sock=socket.socket();sock.bind(("127.0.0.1",0));port=sock.getsockname()[1];sock.close()
        self.url=f"http://127.0.0.1:{port}"
        self.origin=origin
        self.log=tempfile.TemporaryFile(mode="w+t")
        options=[]
        if origin:options.extend(["--allow-origin",origin])
        if frontend_debug:options.append("--allow-unsigned-frontend")
        self.process=subprocess.Popen([binary,"daemon","--listen",f"127.0.0.1:{port}","--token",TOKEN,"--proc-root",str(root),*options,*map(str,extra)],stdout=subprocess.PIPE,stderr=self.log)
        for _ in range(100):
            if self.process.poll() is not None:
                self.log.seek(0);raise AssertionError(self.log.read())
            try:
                if self.request("/api/v1/health")[0]==200:return
            except (OSError,urllib.error.URLError):pass
            time.sleep(.05)
        raise AssertionError("daemon 启动超时")

    def request(self,path,method="GET",data=None,headers=None):
        h={"Authorization":"Bearer "+TOKEN,**({"Origin":self.origin} if self.origin else {}),**(headers or {})}
        if data is not None:h["Content-Type"]="application/json"
        r=urllib.request.Request(self.url+path, data=None if data is None else json.dumps(data).encode(),method=method,headers=h)
        try:
            with urllib.request.urlopen(r,timeout=5) as resp:return resp.status,resp.headers,resp.read()
        except urllib.error.HTTPError as e:return e.code,e.headers,e.read()

    def data(self,path):
        status,_,data=self.request(path);assert status==200,(status,data)
        value=json.loads(data)
        endpoint=path.split('?')[0]
        if endpoint in ('/api/v1/current','/api/v1/series','/api/v1/processes') or endpoint.endswith(('/current','/series')):
            value=decode(value)
            if endpoint=='/api/v1/processes' or endpoint.startswith('/api/v1/processes/') and endpoint.endswith('/current'):
                return value['batches'][0] if value['batches'] else {'processes':[], 'complete':False}
        return value

    def close(self):
        self.process.send_signal(signal.SIGINT)
        try:self.process.wait(timeout=8)
        except subprocess.TimeoutExpired:self.process.kill();self.process.wait();raise AssertionError("daemon 未及时退出")
        self.log.close()


def main():
    parser=argparse.ArgumentParser();parser.add_argument("--binary",required=True);parser.add_argument("--diagnostic",action="store_true");args=parser.parse_args()
    binary=str(Path(args.binary).resolve())
    with tempfile.TemporaryDirectory(prefix="procface-e2e-") as tmp:
        root=Path(tmp)/"proc";root.mkdir();fixture(root,300)
        cap=json.loads(cli(binary,"capabilities","--proc-root",root).stdout)
        assert cap['features']['sqlite']==args.diagnostic
        if not args.diagnostic:
            forbidden=Path(tmp)/'disabled.db'
            assert cli(binary,'daemon','--sqlite-path',forbidden,ok=False).stdout==''
            assert not forbidden.exists()
        for fmt in ("json","jsonl","tsv","table"):
            p=cli(binary,"sample","--proc-root",root,"--count",1,"--format",fmt,"--metrics","time,memory,network")
            assert "\x1b" not in p.stdout
            if fmt=="json":assert len(json.loads(p.stdout))>0
            if fmt=="jsonl":assert all(json.loads(line)["schema_version"]==1 for line in p.stdout.splitlines())
            if fmt=="tsv":assert all(len(line.split("\t"))==11 for line in p.stdout.splitlines())
        p1=json.loads(cli(binary,"sample","--proc-root",root,"--count",1,"--metrics","pressure,interrupts,softirq","--format","json").stdout)
        assert any(s["metric"]=="pressure.some.avg10" and s["entity"]=="cpu" for s in p1)
        assert any(s["metric"]=="interrupts.count" and s["entity"]=="interrupts:1:CPU0" for s in p1)
        assert any(s["metric"]=="softirq.count" for s in p1), p1
        p=cli(binary,"sample","--proc-root",root,"--count",1,"--metrics","process","--format","json")
        assert sum(s["metric"]=="process.pid" for s in json.loads(p.stdout))==300
        assert cli(binary,"sample","--interval","0.5",ok=False).stdout==""
        assert cli(binary,"sample","--unknown",ok=False).stdout==""
        cli(binary,"sample","--format","json",ok=False)
        # 时间、机器输出、有限轮数与选定 PID 均复用本地采集核心。
        common=("sample","--proc-root",root,"--count",1,"--metrics","memory","--format")
        plain=cli(binary,*common,"tsv").stdout
        human=cli(binary,*common,"tsv","--human","--colors").stdout
        def without_session(text):
            return [row.split("\t")[:1]+row.split("\t")[2:] for row in text.splitlines()]
        assert without_session(human)==without_session(plain)
        assert "MiB" in cli(binary,*common,"table","--human").stdout
        assert "\x1b" in cli(binary,*common,"table","--colors").stdout
        stamp=json.loads(cli(binary,*common,"json","--timestamp","unix").stdout)
        assert all(s["uptime_s"]==10 and abs(s["timestamp_unix"]-time.time())<10 for s in stamp)
        assert all(s["timestamp_unix"] is None for s in json.loads(cli(binary,*common,"json").stdout))
        rounds=json.loads(cli(binary,"sample","--proc-root",root,"--count",2,"--metrics","time","--format","json").stdout)
        assert len({s["sequence"] for s in rounds})==2
        assert cli(binary,"sample","--metrics","not-a-group",ok=False).stdout==""
        chosen=json.loads(cli(binary,"sample","--proc-root",root,"--count",1,"--metrics","process","--pids","100,102","--user",os.geteuid(),"--format","json").stdout)
        assert {s["value"] for s in chosen if s["metric"]=="process.pid"}=={100,102}
        assert not json.loads(cli(binary,"sample","--proc-root",root,"--count",1,"--metrics","process","--user",os.geteuid()+1,"--format","json").stdout)
        trace=json.loads(cli(binary,"trace",100,"--proc-root",root,"--basic","--count",1,"--format","json").stdout)
        assert any(s["metric"]=="process.pid" and s["value"]==100 for s in trace)
        (root/"meminfo").rename(root/"meminfo.saved")
        missing=cli(binary,*common,"json")
        assert "meminfo" in missing.stderr
        assert all(s["status"]=="unsupported" for s in json.loads(missing.stdout))
        (root/"meminfo.saved").rename(root/"meminfo")
        if not args.diagnostic:cli(binary,"trace",100,"--proc-root",root,"--count",1,"--diagnostic-sensitive",ok=False)
        d=Daemon(binary,root)
        try:
            assert d.request("/api/v1/current",headers={"Authorization":"Bearer wrong"})[0]==401
            assert d.request("/api/v1/current",headers={"Origin":"https://untrusted.example"})[0]==403
            assert d.request("/api/v1/current",headers={"Origin":"null"})[0]==403
            pre=d.request("/api/v1/current",method="OPTIONS",headers={"Authorization":""});assert pre[0]==204 and not pre[2]
            h={"frontend_version":"9.8.7","build_id":"local","api_compatibility":{"min":1,"max":1},"wire_schema":"procface-compact-v1","development":True}
            assert d.request("/api/v1/frontend/handshake",method="POST",data=h)[0]==200
            h["api_compatibility"]={"min":2,"max":2};assert d.request("/api/v1/frontend/handshake",method="POST",data=h)[0]==403
            time.sleep(1.2)
            assert len(d.data("/api/v1/processes")["processes"])==300
            performance=d.data("/api/v1/health")["performance"]
            assert set(performance['sampling']) == {'system','process','trace'}
            for group in ('system','process'):
                stats=performance['sampling'][group]
                assert stats['rounds']>0 and stats['max_sample_us']>=stats['last_sample_us']>=0
            assert performance['sampling']['trace']['last_sample_us'] is None
            assert d.data('/api/v1/capabilities')['performance']['health'] is True
            for query in ('limit=0','limit=10001','from=-1','to=NaN','from=20&to=10','follow=2','group=unknown','metric='+','.join(['x']*33),'entity='+'x'*4097):
                assert d.request('/api/v1/series?'+query)[0]==400,query
            filtered=d.data('/api/v1/series?group=system&metric=memory.total_bytes&entity=system&from=10&to=10&limit=1')
            assert len(filtered['batches'])==1
            assert len(filtered['batches'][0]['samples'])==1
            assert filtered['batches'][0]['samples'][0]['metric']=='memory.total_bytes'
            assert not d.data('/api/v1/series?from=11&to=12')['batches']
            assert d.request("/api/v1/series?limit=10001")[0]==400
            assert d.request("/api/v1/export?format=csv")[0]==400
            metrics=d.request("/metrics")[2].decode()
            types,values=prometheus(metrics)
            assert not any(name.startswith("procface_process_") for name in values)
            assert types["procface_memory_total_bytes"]=="gauge"
            assert types["procface_network_rx_bytes_total"]=="counter"
            assert 'interface="lo"' in values["procface_network_rx_bytes_total"][0]
            assert 'device="sda"' in values["procface_disk_read_bytes_total"][0]
            (root/"uptime").write_text("12 0\n")
            (root/"net/dev").write_text("lo: 30 4 0 0 0 0 0 0 40 6 0 0 0 0 0 0\n")
            deadline=time.monotonic()+3
            while True:
                types,values=prometheus(d.request("/metrics")[2].decode())
                if "procface_network_rx_bytes_per_second" in values:break
                assert time.monotonic()<deadline,"未发布网络速率"
                time.sleep(.02)
            assert types["procface_network_rx_bytes_per_second"]=="gauge"
            assert values["procface_network_rx_bytes_per_second"][0].endswith(" 10.0")
            for fmt in ('tsv','jsonl'):
                before_stats=d.data('/api/v1/health')['performance']['transport'][fmt]
                response=d.request('/api/v1/export?format='+fmt)[2]
                header=len(response.split(b'\n',1)[0])+1 if fmt=='tsv' else 0
                if fmt=='tsv': assert response.startswith(b'schema_version\t')
                after_stats=d.data('/api/v1/health')['performance']['transport'][fmt]
                assert after_stats['batches']>before_stats['batches']
                assert after_stats['payload_bytes']-before_stats['payload_bytes']==len(response)-header
            assert d.data('/api/v1/health')['performance']['transport']['sse']['batches']==0
            before=d.data("/api/v1/health")["sequence"]
            assert d.request("/api/v1/trace",method="POST",data={"pid":100})[0]==202
            assert d.request("/api/v1/trace",method="POST",data={"pid":101})[0]==409
            time.sleep(.3);assert d.data("/api/v1/trace")["state"]=="running"
            assert d.data('/api/v1/health')['performance']['sampling']['trace']['rounds']>0
            assert d.data("/api/v1/processes/100/current")["group"]=="trace"
            assert d.request("/api/v1/trace",method="DELETE")[0]==200
            for _ in range(100):
                if d.data("/api/v1/trace")["state"]=="idle":break
                time.sleep(.02)
            assert d.request("/api/v1/trace",method="POST",data={"pid":101})[0]==202
            time.sleep(.2)
            (root/"101/stat").write_text(stat(101,start=456))
            time.sleep(1.1);assert d.data("/api/v1/trace")["state"]=="exited"
            assert d.data("/api/v1/health")["sequence"]>before
            assert d.data("/api/v1/series")["batches"]
        finally:d.close()
        d=Daemon(binary,root,'--process-budget-ms',1)
        try:
            deadline=time.monotonic()+3
            while True:
                processes=d.data('/api/v1/processes')
                if 'diagnostics' in processes and not processes['complete']:break
                assert time.monotonic()<deadline
                time.sleep(.02)
            assert processes['processes']==[] and any('超时' in s for s in processes['diagnostics'])
            before=d.data('/api/v1/health')['sequence']
            time.sleep(1.1)
            assert d.data('/api/v1/health')['sequence']>before
            assert d.data('/api/v1/health')['skipped_rounds']>0
            stats=d.data('/api/v1/health')['performance']['sampling']['process']
            assert stats['budget_us']==1000 and stats['over_budget_rounds']>0
            assert any(b['group']=='system' and b['complete'] for b in d.data('/api/v1/current')['batches'])
        finally:d.close()
        (root/"100/stat").write_text(stat(100).replace("worker ) name", 'quote"slash\\'))
        d=Daemon(binary,root,"--prometheus-process")
        try:
            deadline=time.monotonic()+3
            while True:
                types,values=prometheus(d.request("/metrics")[2].decode())
                if "procface_process_cpu_seconds_total" in types:break
                assert time.monotonic()<deadline,"未发布进程指标"
                time.sleep(.02)
            assert types["procface_process_cpu_seconds_total"]=="counter"
            assert any('pid="100"' in row and 'comm="quote\\"slash\\\\"' in row for row in values["procface_process_rss_bytes"])
        finally:d.close()
        if args.diagnostic:
            saved=Path(tmp)/"flush.db"
            d=Daemon(binary,root,"--sqlite-path",saved,"--sqlite-flush-seconds",30)
            try:
                time.sleep(.2)
                assert d.data("/api/v1/health")["persistence"]["state"]=="enabled"
            finally:d.close()
            with sqlite3.connect(saved) as db:
                assert db.execute("SELECT COUNT(*) FROM samples").fetchone()[0]>0, "退出前未提交待写入样本"
            # 退出时须等待正在读取的 trace，再停止 SQLite，不能丢弃最后一批。
            fifo=root/"100/sched";os.mkfifo(fifo)
            entered=threading.Event();release=threading.Event()
            def blocked_trace():
                with fifo.open("w") as output:
                    entered.set();release.wait(5);output.write("shutdown trace\n")
            worker=threading.Thread(target=blocked_trace,daemon=True);worker.start()
            d=Daemon(binary,root,"--sqlite-path",saved)
            try:
                assert d.request("/api/v1/trace","POST",{"pid":100,"budget_ms":10000})[0]==202
                assert entered.wait(3)
                d.process.send_signal(signal.SIGINT)
                time.sleep(.2)
                assert d.process.poll() is None, "未等待活动 trace 即退出"
                release.set();worker.join(3)
                assert not worker.is_alive()
                assert d.process.wait(timeout=5)==0
            finally:
                release.set();d.close();fifo.unlink()
            with sqlite3.connect(saved) as db:
                assert db.execute("SELECT COUNT(*) FROM samples WHERE payload LIKE '%shutdown trace%'").fetchone()[0]>0
            db=Path(tmp)/"bad.db";db.write_text("not a database")
            d=Daemon(binary,root,"--sqlite-path",db,"--sqlite-flush-seconds",1)
            try:
                time.sleep(.5);assert d.data("/api/v1/health")["persistence"]["state"]=="error"
                assert d.data("/api/v1/current")["batches"]
            finally:d.close()
    print("端到端验证通过：CLI 格式、全量 300 进程、鉴权/CORS、握手、导出、trace 隔离与 PID 复用"+("、SQLite 损坏隔离" if args.diagnostic else ""))


if __name__=="__main__":main()

