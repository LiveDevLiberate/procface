#!/usr/bin/env python3
"""离线构建/校验正式前端。只在开发机使用 Python 3 和 OpenSSL。"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def command(*args):
    return subprocess.check_output(args, stderr=subprocess.PIPE)


def sign(key, payload):
    with tempfile.TemporaryDirectory() as tmp:
        message = Path(tmp) / "message"
        message.write_bytes(payload)
        return base64.b64encode(command("openssl", "pkeyutl", "-sign", "-rawin", "-inkey", str(key), "-in", str(message))).decode()


def verify_signature(public_key, payload, signature):
    raw = base64.b64decode(public_key, validate=True)
    if len(raw) != 32:
        raise ValueError("Ed25519 公钥必须为 32 字节")
    with tempfile.TemporaryDirectory() as tmp:
        key, message, sig = (Path(tmp) / n for n in ("key.der", "message", "signature"))
        key.write_bytes(bytes.fromhex("302a300506032b6570032100") + raw)
        message.write_bytes(payload)
        sig.write_bytes(base64.b64decode(signature, validate=True))
        command("openssl", "pkeyutl", "-verify", "-rawin", "-pubin", "-keyform", "DER", "-inkey", str(key), "-in", str(message), "-sigfile", str(sig))


def compact(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def build(args):
    declaration = {"frontend_version": args.version, "build_id": args.build_id, "api_compatibility": {"min": 1, "max": 1}}
    payload = compact(declaration)
    handshake = {**declaration, "payload": payload, "signature": sign(args.key, payload.encode())}
    source = args.source.read_text(encoding="utf-8")
    marker = "const BUILD = /*PROCFACE_BUILD*/"
    if source.count(marker) != 1:
        raise ValueError("前端构建标记缺失或重复")
    before, rest = source.split(marker)
    _, after = rest.split(";", 1)
    html = (before + marker + compact(handshake).replace("<", "\\u003c") + ";" + after).encode()
    args.output.mkdir(parents=True, exist_ok=True)
    target = args.output / "procface-web.html"
    target.write_bytes(html)
    manifest = {**declaration, "filename": target.name, "sha256": hashlib.sha256(html).hexdigest()}
    payload = compact(manifest)
    (args.output / "frontend-manifest.json").write_text(compact({"payload": payload, "signature": sign(args.key, payload.encode())}) + "\n", encoding="utf-8")
    der = command("openssl", "pkey", "-in", str(args.key), "-pubout", "-outform", "DER")
    prefix = bytes.fromhex("302a300506032b6570032100")
    if not der.startswith(prefix) or len(der) != 44:
        raise ValueError("签名密钥必须使用 Ed25519")
    public_key = base64.b64encode(der[-32:]).decode()
    (args.output / "frontend-public-key.txt").write_text(public_key + "\n", encoding="ascii")
    print("已生成 HTML、外部签名清单与发布公钥；私钥未复制。")


def verify(args):
    envelope = json.loads(args.manifest.read_text(encoding="utf-8"))
    verify_signature(args.public_key, envelope["payload"].encode(), envelope["signature"])
    manifest = json.loads(envelope["payload"])
    if args.html.name != manifest["filename"]:
        raise ValueError("HTML 文件名与清单不符")
    if hashlib.sha256(args.html.read_bytes()).hexdigest() != manifest["sha256"]:
        raise ValueError("HTML 摘要不匹配")
    print("发布文件签名及 SHA-256 校验通过。此结果不证明浏览器运行代码的完整性。")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="operation", required=True)
    p = sub.add_parser("build")
    p.add_argument("--key", required=True, type=Path)
    p.add_argument("--version", required=True)
    p.add_argument("--build-id", required=True)
    p.add_argument("--source", type=Path, default=Path("web/procface-web.html"))
    p.add_argument("--output", type=Path, default=Path("dist"))
    p.set_defaults(run=build)
    p = sub.add_parser("verify")
    p.add_argument("--html", required=True, type=Path)
    p.add_argument("--manifest", required=True, type=Path)
    p.add_argument("--public-key", required=True, help="通过可信渠道确认的 Base64 发布公钥")
    p.set_defaults(run=verify)
    args = parser.parse_args()
    try:
        args.run(args)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, "前端校验/构建失败：" + str(error) + "\n")


if __name__ == "__main__":
    main()

