#!/usr/bin/env python3
"""将已验证的四架构产物打成开发预览包；不发布到远端，也不伪造正式签名。"""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile
import tomllib

TARGETS = ("armv7-unknown-linux-musleabihf", "aarch64-unknown-linux-musl",
           "riscv64gc-unknown-linux-musl", "x86_64-unknown-linux-musl")
ROOT = Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT/"Cargo.toml").read_text())["package"]["version"]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--release-root", type=Path, required=True)
    p.add_argument("--diagnostic-root", type=Path, required=True)
    p.add_argument("--output", type=Path, default=ROOT/"dist")
    args=p.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    index=[]
    for flavor, root in (("release",args.release_root),("diagnostic",args.diagnostic_root)):
        for target in TARGETS:
            binary=root/target/"release/procface"
            if not binary.is_file():
                raise SystemExit("缺少产物："+str(binary))
            name=f"procface-{VERSION}-preview-{target}-{flavor}"
            metadata={"procface_version":VERSION,"api_version":1,"schema_version":1,
                      "target":target,"flavor":flavor,"channel":"unsigned-development-preview",
                      "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
                      "frontend_sha256":hashlib.sha256((ROOT/"web/procface-web.html").read_bytes()).hexdigest()}
            with tempfile.TemporaryDirectory() as temp:
                manifest=Path(temp)/"build.json"
                manifest.write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+"\n")
                archive=args.output/(name+".tar.gz")
                with tarfile.open(archive,"w:gz") as tar:
                    for source, dest in ((binary,"procface"),(ROOT/"LICENSE","LICENSE"),(ROOT/"README.md","README.md"),
                                         (ROOT/"web/procface-web.html","procface-web.html"),(manifest,"build.json")):
                        tar.add(source,arcname=name+"/"+dest)
            index.append({"filename":archive.name,"sha256":hashlib.sha256(archive.read_bytes()).hexdigest(),**metadata})
    (args.output/"preview-index.json").write_text(json.dumps(index,indent=2)+"\n")
    print(f"已生成 {len(index)} 个未签名开发预览包；正式签名和远端发布尚未执行。")


if __name__=="__main__":
    main()

