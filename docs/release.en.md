# Release operations

[简体中文](release.md) | English

The public repository is [LiveDevLiberate/procface](https://github.com/LiveDevLiberate/procface). Development Release `v0.1.0` includes release/diagnostic archives for four architectures, a signed frontend and verification manifests. [GitHub Pages](https://livedevliberate.github.io/procface/) deploys the Release HTML unchanged. See the Chinese counterpart for dated deployment records and checksums. Target-board startup, sampling, Trace and resource measurements still need acceptance.

## Prerequisites

- Test and cross-build all four architectures from the same clean commit. Record the commit, toolchain and results.
- Access the project's long-lived Ed25519 private key only in a controlled release environment. Distribute the public key through an independently trusted channel.
- Match the release tag to `Cargo.toml`. API and schema versions are independent of software versions.
- Deploy a static Pages directory without rewriting or injecting HTML.

The configured public key is `U30NiFFqHG7qq96W1V34mHuPxs23qFiVp7R6eYw6gkQ=`. Confirm this trust root with the maintainer before configuring a daemon.

## Local assembly

Python 3.11+ and OpenSSL run on the development machine only. Replace example paths, key and commit ID:

```sh
python3 tools/release_frontend.py build \
  --key /secure/path/release.key \
  --version 0.1.0 --build-id COMMIT_ID \
  --output /release-work/frontend

python3 tools/package_release.py \
  --release-root /cross-build/release \
  --diagnostic-root /cross-build/diagnostic \
  --frontend /release-work/frontend \
  --public-key TRUSTED_BASE64_KEY \
  --output /release-work/assembled
```

Each cross-build root must contain `TARGET/release/procface` for ARMv7 hard-float, ARM64, RISC-V64 and x86_64. The assembler checks ELF architecture, absence of a dynamic loader, matching external and embedded frontend signatures, frontend/Cargo version agreement and API v1 compatibility. It does not prove binary provenance: retain same-commit build and test evidence rather than relying on self-reported `build.json` fields.

The output directory must not already exist. Failed assembly leaves no partial release directory. Successful output contains:

- `assets/`: eight archives, standalone HTML, external signature manifest, public key, LICENSE, version index and SHA256SUMS for Release upload. Archives include Chinese and English READMEs, release guides, compact protocol notes and internationalization guides.
- `pages/`: identical HTML as `index.html` and `procface-web.html`, external manifest, LICENSE and `.nojekyll` for static hosting.

Configure the daemon's trusted key using `--frontend-public-key`, or embed it at build time with `PROCFACE_RELEASE_KEY`. The daemon does not automatically trust arbitrary keys found beside downloaded files. Do not enable `--allow-unsigned-frontend` for a signed release.

## Upload, deployment and acceptance

`.github/workflows/pages.yml` downloads the frontend from Release, verifies signatures and hashes against the previously confirmed repository variable `PROCFACE_RELEASE_PUBLIC_KEY`, and deploys the original bytes. Only the public key is stored in that variable; Pages does not need the private key. Publishing a Release triggers deployment; after uploading all attachments, manual deployment is also available:

```sh
gh variable set PROCFACE_RELEASE_PUBLIC_KEY --body 'TRUSTED_BASE64_KEY'
gh workflow run pages.yml -f tag=v0.1.0
```

The `github-pages` environment allows deployment from `main` and `v*` tags. Release must include `procface-web.html`, `frontend-manifest.json` and `LICENSE`. Missing keys/assets or verification failures stop deployment without falling back to unsigned HTML. The workflow downloads the deployed HTML and compares it with Release bytes. Prefer uploading complete assets to a draft Release before publication.

1. Create a Release matching the Cargo version and upload all files from `assets/`.
2. Deploy `pages/` unchanged to the fixed Pages entry.
3. Compare SHA-256 of downloaded online HTML with the Release's `procface-web.html`.
4. Run `release_frontend.py verify` with a previously trusted public key. Save a downloaded `index.html` copy as `procface-web.html`, as required by the manifest.
5. Connect a test daemon from the hosted frontend; check API-range rejection, signature rejection, explicit Origin permissions and Trace takeover after reconnection. Local file access requires `--allow-file-origin` and still requires a token.

Successful upload or deployment does not establish target-device compatibility. ARM32, ARM64 and RISC-V64 still need runtime sampling, SSE, Trace and resource measurements. Rust's RISC-V target baseline of Linux 4.20 differs from the planned common 4.19 baseline; confirm platform boundaries separately.

## Repeatable local checks

```sh
python3 tests/test_package.py \
  --release-root /cross-build/release \
  --diagnostic-root /cross-build/diagnostic
```

This check uses temporary keys and removes temporary signed output afterward. It does not create a Release or deploy Pages.
