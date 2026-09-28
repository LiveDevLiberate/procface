#!/usr/bin/env python3
"""默认来源拒绝、显式文件来源、开发模式边界及随机 token。"""
import argparse
import json
import re
import signal
import socket
import subprocess
import tempfile
import urllib.request
from pathlib import Path
from test_e2e import Daemon, fixture


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);args=parser.parse_args()
    binary=str(Path(args.binary).resolve())
    with tempfile.TemporaryDirectory(prefix='procface-security-') as tmp:
        root=Path(tmp)/'proc';root.mkdir();fixture(root)
        declaration={'frontend_version':'0.1.0','build_id':'development','api_compatibility':{'min':1,'max':1},'development':True}
        daemon=Daemon(binary,root,frontend_debug=False,origin=None)
        try:
            cap=daemon.data('/api/v1/capabilities')
            assert cap['allow_unsigned_frontend'] is False
            assert cap['sensitive_enabled'] is False and cap['persistence_enabled'] is False
            assert daemon.request('/api/v1/frontend/handshake','POST',declaration)[0]==403
            for origin in ('null','https://example.github.io','http://localhost:8000'):
                result=daemon.request('/api/v1/current',headers={'Origin':origin})
                assert result[0]==403 and not result[1].get('Access-Control-Allow-Origin')
            assert daemon.request('/api/v1/current',headers={'Authorization':''})[0]==401
        finally:daemon.close()
        daemon=Daemon(binary,root,'--allow-file-origin',origin='https://example.github.io')
        try:
            cap=daemon.data('/api/v1/capabilities');assert cap['allow_unsigned_frontend']
            assert not cap['sensitive_enabled'] and not cap['persistence_enabled']
            for origin in ('null','https://example.github.io'):
                response=daemon.request('/api/v1/frontend/handshake','POST',declaration,headers={'Origin':origin})
                assert response[0]==200 and response[1]['Access-Control-Allow-Origin']==origin
                assert daemon.request('/api/v1/current',headers={'Origin':origin,'Authorization':'wrong'})[0]==401
                pre=daemon.request('/api/v1/current','OPTIONS',headers={'Origin':origin,'Authorization':''})
                assert pre[0]==204 and pre[2]==b''
            bad={**declaration,'api_compatibility':{'min':2,'max':2}}
            assert daemon.request('/api/v1/frontend/handshake','POST',bad)[0]==403
            assert daemon.request('/api/v1/current',headers={'Origin':'https://example.github.io.evil.test'})[0]==403
            daemon.log.seek(0);assert '开发调试已启用' in daemon.log.read()
        finally:daemon.close()
        # 不传 token 时仅在 stderr 给出随机值，可用于实际鉴权；不会混入 stdout。
        with socket.socket() as sock:
            sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
        process=subprocess.Popen([binary,'daemon','--listen',f'127.0.0.1:{port}','--proc-root',str(root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            line=process.stderr.readline();match=re.fullmatch(r'ProcFace token: ([0-9a-f]{32,})\n',line);assert match,line
            token=match[1]
            assert 'ProcFace daemon:' in process.stderr.readline()
            request=urllib.request.Request(f'http://127.0.0.1:{port}/api/v1/health',headers={'Authorization':'Bearer '+token})
            with urllib.request.urlopen(request,timeout=5) as response:assert json.load(response)['sequence']>=0
        finally:
            process.send_signal(signal.SIGINT)
            stdout,stderr=process.communicate(timeout=8)
            assert not stdout
    print('安全默认值验证通过：来源默认拒绝、null 显式允许、预检无数据、调试不绕过 token/API、不启用敏感或持久化、随机 token。')


if __name__=='__main__':main()
