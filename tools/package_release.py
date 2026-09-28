#!/usr/bin/env python3
"""校验签名前端并组装正式发布目录；不创建 tag、不上传、不部署。"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import tarfile
import tempfile
import tomllib

from release_frontend import verify, verify_signature
from package_preview import TARGETS, ROOT


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release-root',type=Path,required=True)
    parser.add_argument('--diagnostic-root',type=Path,required=True)
    parser.add_argument('--frontend',type=Path,required=True)
    parser.add_argument('--public-key',required=True,help='通过可信渠道确认的 Ed25519 Base64 公钥')
    parser.add_argument('--output',type=Path,required=True,help='必须尚不存在，避免混入旧产物')
    args=parser.parse_args()
    version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    html=args.frontend/'procface-web.html';manifest=args.frontend/'frontend-manifest.json'
    verify(argparse.Namespace(html=html,manifest=manifest,public_key=args.public_key))
    declaration=json.loads(json.loads(manifest.read_text())['payload'])
    embedded=json.loads(html.read_text().split('const BUILD = /*PROCFACE_BUILD*/')[1].split(';',1)[0])
    verify_signature(args.public_key,embedded['payload'].encode(),embedded['signature'])
    signed=json.loads(embedded['payload'])
    for field in ('frontend_version','build_id','api_compatibility','wire_schema'):
        if not (embedded[field]==signed[field]==declaration[field]):raise ValueError('签名声明字段不一致：'+field)
    if declaration['frontend_version']!=version:raise ValueError('前端与 Cargo 发布版本不匹配')
    if declaration['wire_schema']!='procface-compact-v1':raise ValueError('前端线格式不兼容')
    if not declaration['api_compatibility']['min']<=1<=declaration['api_compatibility']['max']:raise ValueError('前端不支持 API v1')
    binaries=[]
    for flavor,root in (('release',args.release_root),('diagnostic',args.diagnostic_root)):
        for target in TARGETS:
            binary=root/target/'release/procface'
            data=binary.read_bytes()
            machines={'armv7-unknown-linux-musleabihf':(1,40),'aarch64-unknown-linux-musl':(2,183),'riscv64gc-unknown-linux-musl':(2,243),'x86_64-unknown-linux-musl':(2,62)}
            elf_class,machine=machines[target]
            if data[:4]!=b'\x7fELF' or data[4]!=elf_class or data[5]!=1 or int.from_bytes(data[18:20],'little')!=machine:
                raise ValueError('ELF 架构不匹配：'+str(binary))
            offset=struct.unpack_from('<I' if elf_class==1 else '<Q',data,28 if elf_class==1 else 32)[0]
            entry_size,count=struct.unpack_from('<HH',data,42 if elf_class==1 else 54)
            if not count or entry_size<(32 if elf_class==1 else 56) or offset+entry_size*count>len(data):
                raise ValueError('ELF 程序头非法：'+str(binary))
            if any(struct.unpack_from('<I',data,offset+i*entry_size)[0]==3 for i in range(count)):
                raise ValueError('ELF 依赖动态加载器，拒绝作为静态产物：'+str(binary))
            binaries.append((flavor,target,binary,hashlib.sha256(data).hexdigest()))
    if args.output.exists():raise ValueError('输出目录已存在，请使用新的目录')
    args.output.parent.mkdir(parents=True,exist_ok=True)
    # 全部验证后在临时目录组装，失败不会留下看似可发布的不完整目录。
    with tempfile.TemporaryDirectory(dir=args.output.parent) as temp:
        staged=Path(temp)/'release';staged.mkdir();assets=staged/'assets';assets.mkdir();pages=staged/'pages';pages.mkdir()
        for name in ('procface-web.html','frontend-manifest.json'):
            shutil.copyfile(args.frontend/name,assets/name)
            shutil.copyfile(args.frontend/name,pages/name)
        shutil.copyfile(html,pages/'index.html')
        (pages/'.nojekyll').touch()
        shutil.copyfile(ROOT/'LICENSE',assets/'LICENSE')
        shutil.copyfile(ROOT/'LICENSE',pages/'LICENSE')
        (assets/'frontend-public-key.txt').write_text(args.public_key+'\n')
        index=[]
        for flavor,target,binary,digest in binaries:
            name=f'procface-{version}-{target}-{flavor}'
            metadata={'procface_version':version,'api_version':1,'schema_version':1,'wire_schema':declaration['wire_schema'],'build_id':declaration['build_id'],
                'target':target,'flavor':flavor,'binary_sha256':digest,'frontend_sha256':declaration['sha256']}
            build=Path(temp)/'build.json';build.write_text(json.dumps(metadata,indent=2)+'\n')
            with tarfile.open(assets/(name+'.tar.gz'),'w:gz') as tar:
                for source,dest in ((binary,'procface'),(ROOT/'LICENSE','LICENSE'),(ROOT/'README.md','README.md'),
                    (html,'procface-web.html'),(manifest,'frontend-manifest.json'),(assets/'frontend-public-key.txt','frontend-public-key.txt'),(build,'build.json')):
                    tar.add(source,arcname=name+'/'+dest)
            index.append({'filename':name+'.tar.gz',**metadata})
        (assets/'release-index.json').write_text(json.dumps(index,indent=2)+'\n')
        checksums=''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.name+'\n' for p in sorted(assets.iterdir()))
        (assets/'SHA256SUMS').write_text(checksums)
        assert (pages/'index.html').read_bytes()==(assets/'procface-web.html').read_bytes()
        staged.rename(args.output)
    print('已组装 assets 与 pages；两处 HTML 字节一致。尚未上传或部署。')


if __name__=='__main__':main()
