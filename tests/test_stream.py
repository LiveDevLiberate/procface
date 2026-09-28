#!/usr/bin/env python3
"""真实 HTTP SSE、并发限制、进程最后一次读取超时和导出释放测试。"""
import argparse
import json
import os
import socket
from pathlib import Path
import tempfile
import threading
import time
import urllib.request
import urllib.error
from urllib.parse import urlsplit
from test_e2e import Daemon, fixture, cli, TOKEN


def stream(daemon, path="/api/v1/stream"):
    return urllib.request.urlopen(urllib.request.Request(daemon.url+path, headers={
        "Authorization": "Bearer "+TOKEN, "Origin": "http://localhost:8000"}), timeout=20)


def event(response):
    name, data = "", []
    while True:
        line = response.readline().decode()
        if not line:
            raise AssertionError("SSE 意外断开")
        if line == "\n" and data:
            return name, json.loads("\n".join(data))
        if line.startswith("event:"):
            name = line[6:].strip()
        if line.startswith("data:"):
            data.append(line[5:].strip())


def unread_client(daemon, path):
    """完成 HTTP 握手后停止读，利用小接收窗口制造真实 TCP 背压。"""
    address = urlsplit(daemon.url)
    sock = socket.socket()
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
    sock.settimeout(5)
    sock.connect((address.hostname, address.port))
    sock.sendall((f"GET {path} HTTP/1.1\r\nHost: {address.netloc}\r\n"
                  f"Authorization: Bearer {TOKEN}\r\nConnection: close\r\n\r\n").encode())
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        part = sock.recv(1)
        assert part, "未收到 HTTP 响应头"
        header.extend(part)
    assert header.startswith(b"HTTP/1.1 200"), header
    return sock


def slow_clients(binary, root):
    # 使用非敏感 extended 文件扩大合法 trace 批次，不依赖 diagnostic feature。
    for name in ("sched", "schedstat", "smaps_rollup", "limits", "loginuid", "oom_score", "oom_score_adj"):
        (root/"100"/name).write_text("x"*(200*1024))
    daemon = Daemon(binary, root, "--trace-memory-bytes", 8*1024*1024)
    clients = []
    try:
        clients.append(unread_client(daemon, "/api/v1/stream"))
        assert daemon.request("/api/v1/trace", "POST", {"pid":100,"extended":True})[0]==202
        before = daemon.data("/api/v1/health")["sequence"]
        deadline = time.monotonic()+18
        saw_limit = False
        while True:
            try:
                response = stream(daemon)
                assert event(response)[0]=="connected"
                response.close()
                break
            except urllib.error.HTTPError as e:
                assert e.code==429
                saw_limit = True
                assert time.monotonic()<deadline, "慢 SSE 客户端未释放连接槽"
                assert daemon.data("/api/v1/trace")["state"]=="running"
                time.sleep(.3)
        assert saw_limit
        assert daemon.data("/api/v1/health")["sequence"]>before+4
        # 两个慢导出占满名额，随后必须自动释放，普通采样继续。
        clients.extend(unread_client(daemon, "/api/v1/export?format=jsonl&follow=1&group=trace") for _ in range(2))
        assert daemon.request("/api/v1/export?limit=1")[0]==429
        before = daemon.data("/api/v1/health")["sequence"]
        deadline = time.monotonic()+18
        while daemon.request("/api/v1/export?limit=1")[0]==429:
            assert time.monotonic()<deadline, "慢导出客户端未释放名额"
            time.sleep(.3)
        assert daemon.data("/api/v1/health")["sequence"]>before
    finally:
        for client in clients:client.close()
        daemon.close()


def main():
    p=argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    args=p.parse_args()
    binary=str(Path(args.binary).resolve())
    with tempfile.TemporaryDirectory(prefix="procface-stream-") as tmp:
        root=Path(tmp)/"proc"
        root.mkdir()
        fixture(root)
        daemon=Daemon(binary,root)
        try:
            response=stream(daemon)
            assert event(response)[0]=="connected"
            assert daemon.request("/api/v1/stream")[0]==429
            name, batch=event(response)
            assert name=="sample" and batch["sequence"]>0
            sequence=batch["sequence"]
            response.close()
            deadline=time.monotonic()+5
            while True:
                try:
                    response=stream(daemon)
                    break
                except urllib.error.HTTPError as e:
                    assert e.code==429
                    if time.monotonic()>=deadline:raise
                    time.sleep(.1)
            assert event(response)[0]=="connected"
            start=time.monotonic()
            while True:
                name, data=event(response)
                if name=="sample":
                    assert data["sequence"]>sequence
                    sequence=data["sequence"]
                if name=="heartbeat":
                    assert time.monotonic()-start>=13
                    break
            response.close()
            result=daemon.data("/api/v1/series?limit=1")
            assert len(result["batches"])==1 and result["has_more"]
            follow=stream(daemon,"/api/v1/export?format=jsonl&follow=1")
            row=json.loads(follow.readline())
            assert row["schema_version"]==1
            follow.close()
            follow=stream(daemon,"/api/v1/export?format=tsv&follow=1")
            assert follow.headers.get("Transfer-Encoding")=="chunked"
            assert follow.readline().decode().startswith("schema_version\tsession_id\t")
            assert len(follow.readline().decode().rstrip("\n").split("\t"))==11
            follow.close()
            before=daemon.data("/api/v1/health")["sequence"]
            time.sleep(1.1)
            assert daemon.data("/api/v1/health")["sequence"]>before
        finally:
            daemon.close()
        # 停止 trace 后，其旧数据仍必须遵循内存时间窗口。
        daemon=Daemon(binary,root,"--history-seconds",1)
        try:
            assert daemon.request("/api/v1/trace","POST",{"pid":100})[0]==202
            deadline=time.monotonic()+3
            while daemon.request("/api/v1/processes/100/current")[0]!=200:
                assert time.monotonic()<deadline
                time.sleep(.05)
            original=daemon.data("/api/v1/health")["sequence"]
            assert daemon.request("/api/v1/trace","DELETE")[0]==200
            time.sleep(.1)
            (root/"uptime").write_text("20 0\n")
            time.sleep(1.2)
            assert daemon.request("/api/v1/processes/100/current")[0]==404
            assert daemon.data("/api/v1/series?after=0")["history_gap"]
            assert not daemon.data("/api/v1/series?after="+str(daemon.data("/api/v1/health")["sequence"]))["history_gap"]
        finally:
            daemon.close()
        slow_clients(binary, root)
        # 单个进程最后一次阻塞读取超过预算，整轮必须被丢弃。
        status=root/"100/status"
        contents=status.read_text()
        status.unlink()
        os.mkfifo(status)
        def delayed_status():
            with status.open("w") as writer:
                time.sleep(.1)
                writer.write(contents)
        worker=threading.Thread(target=delayed_status,daemon=True)
        worker.start()
        result=cli(binary,"trace",100,"--proc-root",root,"--basic","--budget-ms",1,"--count",1,"--format","json")
        assert json.loads(result.stdout)==[], result.stdout
        assert "超时" in result.stderr
        worker.join(2)
        assert not worker.is_alive()
    print("实时流验证通过：SSE 序号、15 秒心跳、连接释放、窗口缺口、JSONL/TSV follow、TCP 背压下 SSE/导出释放及采集隔离、PID 读取预算。")


if __name__=="__main__":
    main()

