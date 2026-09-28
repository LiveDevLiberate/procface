#!/usr/bin/env python3
"""使用临时签名密钥与现有四架构二进制验证发布组装，不发布到远端。"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

ROOT=Path(__file__).resolve().parents[1]


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--release-root',required=True);parser.add_argument('--diagnostic-root',required=True);args=parser.parse_args()
    def run(*command):return subprocess.run(command,check=True,cwd=ROOT,capture_output=True,text=True)
    with tempfile.TemporaryDirectory(prefix='procface-package-') as folder:
        root=Path(folder);key=root/'test.key';frontend=root/'frontend';output=root/'output'
        run('openssl','genpkey','-algorithm','ED25519','-out',str(key))
        version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
        run('python3','tools/release_frontend.py','build','--key',str(key),'--version',version,'--build-id','package-test','--output',str(frontend))
        public=(frontend/'frontend-public-key.txt').read_text().strip()
        command=['python3','tools/package_release.py','--release-root',args.release_root,'--diagnostic-root',args.diagnostic_root,'--frontend',str(frontend),'--public-key',public,'--output',str(output)]
        run(*command)
        assets=output/'assets';pages=output/'pages'
        assert (pages/'index.html').read_bytes()==(assets/'procface-web.html').read_bytes()==(pages/'procface-web.html').read_bytes()
        for line in (assets/'SHA256SUMS').read_text().splitlines():
            digest,name=line.split('  ');assert hashlib.sha256((assets/name).read_bytes()).hexdigest()==digest
        index=json.loads((assets/'release-index.json').read_text());assert len(index)==8
        for item in index:
            with tarfile.open(assets/item['filename']) as tar:
                prefix=item['filename'].removesuffix('.tar.gz')+'/'
                assert tar.extractfile(prefix+'LICENSE').read()==(ROOT/'LICENSE').read_bytes()
                assert tar.extractfile(prefix+'procface-web.html').read()==(assets/'procface-web.html').read_bytes()
                assert hashlib.sha256(tar.extractfile(prefix+'procface').read()).hexdigest()==item['binary_sha256']
                assert not any(name.endswith('.key') for name in tar.getnames())
        # 拒绝覆盖发布目录以及修改后的 HTML。
        assert subprocess.run(command,cwd=ROOT,capture_output=True).returncode!=0
        (frontend/'procface-web.html').write_bytes((frontend/'procface-web.html').read_bytes()+b' modified')
        command[-1]=str(root/'rejected')
        assert subprocess.run(command,cwd=ROOT,capture_output=True).returncode!=0
        assert not (root/'rejected').exists()
    print('发布组装验证通过：八个归档、架构、签名、摘要、LICENSE、Pages 字节一致；篡改与覆盖被拒绝。临时测试密钥已删除。')


if __name__=='__main__':main()
