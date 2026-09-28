#!/usr/bin/env python3
"""验证发布文件、正式签名握手，以及显式未签名调试边界。"""
import argparse
import base64
import json
from pathlib import Path
import subprocess
import tempfile
from test_e2e import Daemon, fixture, TOKEN, ROOT


def run(*args, success=True):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True, timeout=20)
    assert (result.returncode == 0) == success, result.stderr + result.stdout
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="procface-signing-") as tmp:
        tmp = Path(tmp)
        key = tmp / "ephemeral-test.key"
        run("openssl", "genpkey", "-algorithm", "ED25519", "-out", key)
        output = tmp / "release"
        run("python3", ROOT / "tools/release_frontend.py", "build", "--key", key,
            "--version", "9.8.7", "--build-id", "test-build", "--output", output)
        public = (output / "frontend-public-key.txt").read_text().strip()
        verify = ["python3", ROOT / "tools/release_frontend.py", "verify",
                  "--html", output / "procface-web.html",
                  "--manifest", output / "frontend-manifest.json", "--public-key", public]
        run(*verify)
        html = (output / "procface-web.html").read_text()
        build = json.loads(html.split("const BUILD = /*PROCFACE_BUILD*/")[1].split(";", 1)[0])
        (output / "procface-web.html").write_text(html + "\n<!-- modified -->")
        run(*verify, success=False)
        (output / "procface-web.html").write_text(html)
        envelope = json.loads((output / "frontend-manifest.json").read_text())
        envelope["payload"] += " "
        (output / "frontend-manifest.json").write_text(json.dumps(envelope))
        run(*verify, success=False)
        proc = tmp / "proc"
        proc.mkdir()
        fixture(proc)
        daemon = Daemon(str(Path(args.binary).resolve()), proc, "--frontend-public-key", public)
        try:
            # 软件版本不同仍可建链，使用真正的 Ed25519 签名。
            assert daemon.request("/api/v1/frontend/handshake", "POST", build)[0] == 200
            mismatch = {**build, "build_id": "other"}
            assert daemon.request("/api/v1/frontend/handshake", "POST", mismatch)[0] == 403
            tampered = {**build, "payload": build["payload"] + " "}
            assert daemon.request("/api/v1/frontend/handshake", "POST", tampered)[0] == 403
            downgrade = {**tampered, "development": True}
            assert daemon.request("/api/v1/frontend/handshake", "POST", downgrade)[0] == 403
            assert daemon.request("/api/v1/frontend/handshake", "POST", build,
                                  headers={"Authorization": "Bearer wrong"})[0] == 401
        finally:
            daemon.close()
        assert not any(p.suffix == ".key" for p in output.iterdir())
    print("发布签名验证通过：文件篡改、清单篡改、版本不一致、无效 token 和签名降级均被拒绝。测试密钥已销毁。")


if __name__ == "__main__":
    main()

