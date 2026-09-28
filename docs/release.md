# 发布操作

当前版本尚未正式发布。以下步骤准备同一提交上的四架构二进制、签名单体 HTML 和 Pages 目录。实际仓库、发布密钥、目标板验收和最低内核验证仍须落实；不把测试密钥当作发布信任根。

## 前置条件

- 在同一干净提交上执行项目测试及四架构交叉构建。记录提交 ID、工具链版本和验收结果。
- 使用项目长期维护的 Ed25519 发布私钥。仅在受控发布环境访问私钥，通过独立可信渠道分发公钥。
- `Cargo.toml` 版本与准备发布的 tag 对应。API 与 schema 版本独立于软件版本。
- Pages 配置为部署静态目录；不要在部署时再次改写或注入 HTML。

## 本地组装

Python 3.11+ 与 OpenSSL 仅运行在开发机。示例中的目录、公钥和提交 ID 需要替换为真实值。

```sh
python3 tools/release_frontend.py build \
  --key /安全目录/release.key \
  --version 0.1.0 --build-id COMMIT_ID \
  --output /发布工作目录/frontend

python3 tools/package_release.py \
  --release-root /普通版交叉编译目录 \
  --diagnostic-root /诊断版交叉编译目录 \
  --frontend /发布工作目录/frontend \
  --public-key 可信Base64公钥 \
  --output /发布工作目录/assembled
```

每个交叉编译目录须包含 `TARGET/release/procface`，目标为 ARMv7 hard-float、ARM64、RISC-V64、x86_64。组装器验证 ELF 架构、没有动态加载器、前端外部签名与内嵌签名声明一致、前端软件版本匹配 Cargo、API v1 兼容。它不会证明二进制来自指定提交；同提交构建和测试证据由发布流程保留，不能用 build.json 自报字段代替。

输出目录必须不存在。组装失败不会留下部分发布目录。成功后包含：

- `assets/`：八个归档、独立 HTML、外部签名清单、公钥、LICENSE、版本索引和 SHA256SUMS，作为 Release 附件。
- `pages/`：同一 HTML 的 `index.html` 与 `procface-web.html`、外部签名清单、LICENSE 和 `.nojekyll`，作为静态站点部署目录。

归档内 daemon 可通过 `--frontend-public-key` 指定可信公钥，也可构建时设置 `PROCFACE_RELEASE_KEY` 嵌入公钥。没有配置受信任公钥的 daemon 不会自动信任下载目录里的任意公钥。不要为正式发布启用 `--allow-unsigned-frontend`。

## 上传与部署验收

1. 创建与 Cargo 版本对应的 Release，上传 `assets/` 内文件。
2. 将 `pages/` 原样部署到所选仓库的 GitHub Pages 固定入口。
3. 下载线上 HTML，比较原始字节 SHA-256 与 Release 的 `procface-web.html` 一致。
4. 用事先信任的公钥运行 `release_frontend.py verify`。对线上 `index.html` 下载副本，先保留为清单规定的 `procface-web.html` 文件名再校验。
5. 浏览器从固定入口连接测试 daemon，验证 API 范围拒绝、签名拒绝、显式 Origin 和 Trace 重连接管。本地 file 入口使用 `--allow-file-origin`，仍须 token。

上传或部署成功不代表设备运行验证完成。ARM32、ARM64、RISC-V64 仍需目标环境启动、采样、SSE、Trace 与资源测量结果。Rust RISC-V 官方目标最低 Linux 4.20 与规划的统一 4.19 尚有差异，需要确认平台支持边界。

## 可重复的本地检查

```sh
python3 tests/test_package.py \
  --release-root /普通版交叉编译目录 \
  --diagnostic-root /诊断版交叉编译目录
```

该检查只使用临时密钥，验证后删除所有临时签名产物；不创建 Release、不部署 Pages。
