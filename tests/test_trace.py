#!/usr/bin/env python3
"""Trace 分层、阻塞读取隔离、合作式停止和 PID 切换验证。"""
import argparse
import json
import os
from pathlib import Path
import tempfile
import threading
import time
from test_e2e import Daemon, fixture, cli, stat
from test_stream import stream, event


def until(check, timeout=4):
    deadline=time.monotonic()+timeout
    while True:
        value=check()
        if value:return value
        assert time.monotonic()<deadline,'等待 Trace 状态超时'
        time.sleep(.02)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);parser.add_argument('--diagnostic',action='store_true');args=parser.parse_args()
    binary=str(Path(args.binary).resolve())
    with tempfile.TemporaryDirectory(prefix='procface-trace-') as tmp:
        root=Path(tmp)/'proc';root.mkdir();fixture(root,2)
        target=root/'100'
        (target/'sched').write_text('scheduled\n')
        (target/'task/100').mkdir(parents=True);(target/'task/100/stat').write_text(stat(100))
        # FIFO 没有写入者：普通/基础 Trace 若误读敏感文件会阻塞并使测试失败。
        os.mkfifo(target/'environ')
        common=('trace',100,'--proc-root',root,'--count',1,'--format','json')
        basic=json.loads(cli(binary,*common,'--basic').stdout)
        extended=json.loads(cli(binary,*common,'--threads').stdout)
        assert not any(s['metric']=='trace.sched' for s in basic)
        assert any(s['metric']=='trace.sched' and s['value']=='scheduled\n' for s in extended)
        assert any(s['metric']=='trace.thread.100.stat' for s in extended)
        assert not any(s['metric']=='trace.environ' for s in extended)
        (target/'environ').unlink();(target/'environ').write_text('TRACE_TEST=value\0')
        if args.diagnostic:
            sensitive=json.loads(cli(binary,*common,'--diagnostic-sensitive').stdout)
            assert any(s['metric']=='trace.environ' and 'TRACE_TEST=value' in s['value'] for s in sensitive)
        else:assert cli(binary,*common,'--diagnostic-sensitive',ok=False).stdout==''
        # 权限与高成本跳过有明确状态，不用零值伪装。
        (target/'fd').mkdir()
        for number in range(4097):(target/'fd'/str(number)).touch()
        limited=cli(binary,*common)
        assert 'skipped_expensive' in limited.stderr
        assert any(s['metric']=='process.fd_count' and s['status']=='stale' and s['value'] is None for s in json.loads(limited.stdout))
        if os.geteuid()!=0:
            (target/'io').chmod(0)
            denied=json.loads(cli(binary,*common).stdout)
            assert any(s['metric']=='trace.io' and s['status']=='permission_denied' for s in denied)
            (target/'io').chmod(0o600)
        # 低成本列表不打开 sched；阻塞深度读取时主采样仍增长。
        (target/'sched').unlink();os.mkfifo(target/'sched')
        entered=threading.Event();release=threading.Event()
        def blocked_read():
            with (target/'sched').open('w') as writer:
                entered.set();release.wait(8);writer.write('delayed\n')
        worker=threading.Thread(target=blocked_read,daemon=True);worker.start()
        daemon=Daemon(binary,root)
        try:
            ordinary=until(lambda: daemon.data('/api/v1/series?group=system')['batches'])
            saved_sequence=ordinary[0]['sequence']
            cap=daemon.data('/api/v1/processes/100/capabilities')
            assert cap['pid']==100 and cap['starttime_ticks']==123 and cap['basic'] and cap['extended']
            assert daemon.request('/api/v1/processes/999/capabilities')[0]==404
            assert daemon.request('/api/v1/trace','POST',{'pid':100})[0]==202
            assert entered.wait(3)
            before=daemon.data('/api/v1/health')['sequence']
            time.sleep(1.1)
            assert daemon.data('/api/v1/health')['sequence']>before
            nice=[int(p.read_text().rsplit(')',1)[1].split()[16]) for p in Path(f'/proc/{daemon.process.pid}/task').glob('*/stat')]
            assert 10 in nice, nice
            assert daemon.request('/api/v1/trace','DELETE')[0]==200
            assert daemon.data('/api/v1/trace')['state']=='stopping'
            assert daemon.request('/api/v1/trace','POST',{'pid':101})[0]==409
            release.set();worker.join(3);assert not worker.is_alive()
            until(lambda: daemon.data('/api/v1/trace')['state']=='idle')
            assert daemon.request('/api/v1/processes/100/stream')[0]==404
            assert daemon.data('/api/v1/health')['skipped_rounds']>=1
            assert any(b['sequence']==saved_sequence for b in daemon.data('/api/v1/series?group=system')['batches'])
            # 留下 PID 100 的有效旧历史，再切换，按 PID 分页须先过滤再 limit。
            (target/'sched').unlink();(target/'sched').write_text('normal\n')
            assert daemon.request('/api/v1/trace','POST',{'pid':100})[0]==202
            until(lambda: daemon.request('/api/v1/processes/100/current')[0]==200)
            assert daemon.request('/api/v1/trace','DELETE')[0]==200
            until(lambda: daemon.data('/api/v1/trace')['state']=='idle')
            assert daemon.request('/api/v1/trace','POST',{'pid':101,'interval':2})[0]==202
            until(lambda: daemon.request('/api/v1/processes/101/current')[0]==200)
            result=daemon.data('/api/v1/processes/101/series?limit=1')
            assert len(result['batches'])==1 and result['batches'][0]['processes'][0]['identity']['pid']==101
            assert daemon.data('/api/v1/processes/100/current')['processes'][0]['identity']['pid']==100
            response=stream(daemon,'/api/v1/processes/101/stream');assert event(response)[0]=='connected'
            name,batch=event(response);assert name=='sample' and batch['processes'][0]['identity']['pid']==101
            response.close()
            current=daemon.data('/api/v1/processes/101/current')['sequence']
            until(lambda: daemon.data('/api/v1/processes/101/current')['sequence']>current,5)
            assert daemon.data('/api/v1/trace')['state']=='running'
            # PID 复用结束旧任务，新 trace 的 CPU/I/O 不继承旧基线。
            (root/'101/stat').write_text(stat(101,start=456,user=999))
            until(lambda: daemon.data('/api/v1/trace')['state']=='exited',5)
            assert daemon.request('/api/v1/trace','POST',{'pid':101})[0]==202
            until(lambda: daemon.data('/api/v1/processes/101/current')['processes'][0]['identity']['starttime_ticks']==456)
            batch=daemon.data('/api/v1/processes/101/current')
            assert all(s['status']=='stale' for s in batch['samples'] if s['metric'] in ('process.cpu_usage','process.io.read_bytes_per_second'))
            (root/'101').rename(root/'exited-101')
            until(lambda: daemon.data('/api/v1/trace')['state']=='exited')
            assert daemon.request('/api/v1/processes/101/stream')[0]==404
        finally:
            release.set();worker.join(3);daemon.close()
    print('Trace 验证通过：字段分层、敏感隔离、线程、低优先级 worker、阻塞读取隔离、超时跳轮、stopping 拒绝、切换与 PID 分页、断连继续及复用基线。')


if __name__=='__main__':main()
