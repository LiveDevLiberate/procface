#!/usr/bin/env python3
"""Linux 上启动真实 daemon，验证静态 HTTP 和本地 HTML 两种浏览器入口。"""
import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests'))
from test_e2e import Daemon


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--node', default='node', help='Node 路径；WSL 可指定 node.exe 复用 Windows 浏览器')
    parser.add_argument('--file-url', help='浏览器能访问的 HTML file URL，默认使用本机路径')
    args = parser.parse_args()
    handler = functools.partial(SimpleHTTPRequestHandler, directory=str(ROOT / 'web'))
    with ThreadingHTTPServer(('127.0.0.1', 0), handler) as server:
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        origin = f'http://127.0.0.1:{server.server_port}'
        daemon = None
        try:
            daemon = Daemon(str(args.binary.resolve()), Path('/proc'), '--allow-file-origin', origin=origin)
            for url in (origin + '/procface-web.html', args.file_url or (ROOT / 'web/procface-web.html').as_uri()):
                env = {**os.environ, 'PROCFACE_WEB_URL': url, 'PROCFACE_DAEMON_URL': daemon.url}
                env.setdefault('PROCFACE_BROWSER_CHANNEL', 'chromium')
                subprocess.run([args.node, 'tools/test_browser.cjs'], cwd=ROOT, env=env, check=True, timeout=90)
            print('真实 daemon 联调通过：HTTP 与 file 入口、鉴权、SSE、Trace 接管及导出。')
        finally:
            try:
                if daemon is not None:
                    daemon.close()
            finally:
                server.shutdown()
                worker.join()


if __name__ == '__main__':
    main()
