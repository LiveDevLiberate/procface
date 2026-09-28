#!/usr/bin/env python3
"""默认紧凑 API、独立解码、分页和动态实体的真实 HTTP 验证。"""
import argparse
import json
import tempfile
import time
from pathlib import Path
from compact_wire import decode
from test_e2e import Daemon, fixture
from test_stream import stream, event


def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',required=True)
    args=p.parse_args()
    with tempfile.TemporaryDirectory() as tmp:
        root=Path(tmp)/'proc';root.mkdir();fixture(root)
        daemon=Daemon(str(Path(args.binary).resolve()),root)
        try:
            time.sleep(1.1)
            def raw(path):
                status,_,body=daemon.request(path);assert status==200,(status,body)
                return json.loads(body)
            current=raw('/api/v1/current')
            expanded=decode(current)
            assert {b['group'] for b in expanded['batches']}=={'system','process'}
            for b in current['batches']:
                assert isinstance(b,list) and len(b)==9
                decode({**current,'batches':[b]})  # 不依赖此前的响应。
            page=raw('/api/v1/series?limit=1')
            assert page['has_more'] and page['next_after']==page['batches'][0][0]
            assert all(k in page for k in ('history_gap','lost_through_sequence','oldest_sequence','sequence'))
            next_page=raw('/api/v1/series?after='+str(page['next_after']))
            assert next_page['batches'][0][0]>page['next_after']
            status,_,body=daemon.request('/api/v1/export?format=jsonl')
            assert status==200
            for line in body.splitlines():
                assert decode(json.loads(line))['samples']
            response=stream(daemon)
            try:
                assert event(response)[0]=='connected'
                (root/'net/dev').write_text('eth9: 1 2 0 0 0 0 0 0 3 4 0 0 0 0 0 0\n')
                for _ in range(6):
                    name,b=event(response)
                    if name=='sample' and any(s['entity']=='eth9' for s in b['samples']):break
                else:raise AssertionError('新增网卡无法独立解码')
            finally:response.close()
            filtered=decode(raw('/api/v1/series?group=process&metric=process.rss_bytes&limit=1'))
            assert {s['metric'] for s in filtered['batches'][0]['samples']}=={'process.rss_bytes'}
            assert daemon.request('/api/v1/current?wire=legacy')[0]==400
        finally:daemon.close()
    print('紧凑 HTTP 验证通过：默认数组、独立解码、动态网卡、进程筛选、分页和 JSONL。')


if __name__=='__main__':main()
