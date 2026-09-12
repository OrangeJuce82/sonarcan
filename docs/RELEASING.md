# Releasing SonArcan

SonArcan publishes one native application contract for three targets:

| Package | Target | Accelerator |
| --- | --- | --- |
| SonArcan | macOS Apple Silicon | MLX |
| SonArcan Linux | Debian-compatible Linux x86_64 with NVIDIA GPU | CUDA |
| SonArcan Linux | Debian-compatible Linux arm64 with NVIDIA GPU | CUDA |

Every analysis feature remains unavailable until its complete native pipeline
passes the production probe. There is no Python/PyTorch runtime fallback.

## Release contract

The tag workflow is authoritative. It must:

1. verify that `package.json`, `src-tauri/tauri.conf.json`, and
   `src-tauri/Cargo.toml` contain the tag version;
2. recreate only the matching draft release;
3. build and exercise the selective ExecuTorch worker for Apple Silicon and
   Linux x86_64 and arm64;
4. assemble each selective native runtime and verify its model/backend contract;
5. run `npm run quality:shared` before packaging;
6. verify the resources from the assembled app or DEB;
7. upload checksums and leave the GitHub release as a draft.

Hosted runners prove packaging integrity. Final hardware smoke tests still run
on representative Apple Silicon and NVIDIA Linux machines; an additional
CPU-only Linux check must prove that startup is blocked
before publication.

## Pinned runtimes

- macOS selectively links MLX;
- Linux packages link CUDA and require NVIDIA hardware at startup;
- no release runtime exposes an XNNPACK or portable CPU inference fallback;
- backend-specific ExecuTorch programs remain first-use model assets;
- FFmpeg, yt-dlp, and every native executable are verified before packaging.

Python workers are development references for export and corpus comparison
only. See [ExecuTorch deployment](EXECUTORCH_RUNTIME.md).

## Package formats

macOS produces an ad-hoc-signed `.app` and DMG. The signature ensures internal
bundle consistency but is neither notarization nor Apple identity. Verify the
deep signature after every bundled executable has been exercised.

Debian produces one DEB per architecture. A 512 MiB hard gate rejects oversized
packages, and each DEB is uploaded as one file with a SHA-256 manifest.

Use the commands in `RELEASE_NOTES.md`, then test both paths:

- compatible GPU: the production model probe succeeds and analysis is exposed;
- absent or rejected accelerator: startup remains on the incompatibility screen.

## Cutting a release

1. Update the version in all three manifests and update `RELEASE_NOTES.md`.
2. Run `npm run quality` locally. Run `npm run security` only if dependencies
   changed.
3. Push `dev` and wait for every CI job to pass.
4. Merge the exact qualified commit into `main`.
5. Tag that commit, for example `v0.1.5`, and push the tag.
6. Wait for every release job and inspect the draft assets and checksums.
7. Smoke-test both supported hardware targets and rejected-hardware startup
   before manually publishing.

Never move a tag to retry a release. Fix the cause, increment the version, and
create a new tag.
