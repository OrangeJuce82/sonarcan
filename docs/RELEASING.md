# Releasing SonArcan

SonArcan publishes one application contract in two hardware packages:

| Package | Target | Accelerator |
| --- | --- | --- |
| SonArcan | macOS Apple Silicon | MLX and MPS |
| SonArcan NVIDIA GPU | Debian-compatible Linux x64 | CUDA 12.6 |

Every package blocks startup when its production accelerator probe fails. Both
packages expose the complete product behavior.

## Release contract

The tag workflow is authoritative. It must:

1. verify that `package.json`, `src-tauri/tauri.conf.json`, and
   `src-tauri/Cargo.toml` contain the tag version;
2. recreate only the matching draft release;
3. build and exercise the selective ExecuTorch worker for Apple Silicon and
   Linux x64;
4. assemble each pinned hardware runtime and verify its model/backend contract;
5. run `npm run quality` before packaging;
6. verify the resources from the assembled app or DEB;
7. upload checksums and leave the GitHub release as a draft.

Hosted runners prove packaging integrity. Final hardware smoke tests still run
on representative Apple Silicon, NVIDIA, AMD, and no-compatible-GPU machines
before publication.

## Pinned runtimes

- macOS uses the shared Python 3.13 runtime with MLX and MPS workers;
- Debian NVIDIA resolves Torch 2.13.0 from the pinned CUDA 12.6 index;
- Beat This!, SCNet, and HTDemucs checkpoints install on first qualified launch
  after size and SHA-256 verification;
- LV-Chordia checkpoints, FFmpeg, and yt-dlp are verified before packaging;
- ExecuTorch programs remain first-use model assets, not installer resources.

The Python workers remain the production inference path until the native
ExecuTorch preprocessing, inference, and postprocessing pass the end-to-end
audio corpus. See [ExecuTorch deployment](EXECUTORCH_RUNTIME.md).

## Package formats

macOS produces an ad-hoc-signed `.app` and DMG. The signature ensures internal
bundle consistency but is neither notarization nor Apple identity. Verify the
deep signature after every bundled executable has been exercised.

Debian produces one DEB for each GPU backend. Because GPU runtimes exceed
GitHub's per-asset size limit, the workflow splits each DEB into 1.8 GB parts
and publishes a backend-specific SHA-256 manifest. The unsplit temporary DEB is
deleted before upload.

Reconstruct a Debian package with the commands included in
`RELEASE_NOTES.md`, then test both paths:

- compatible GPU: the production model probe succeeds and analysis is exposed;
- absent or rejected accelerator: startup remains on the incompatibility screen.

## Cutting a release

1. Update the version in all three manifests and update `RELEASE_NOTES.md`.
2. Run `npm run quality` locally. Run `npm run security` only if dependencies
   changed.
3. Push `dev` and wait for every CI job to pass.
4. Merge the exact qualified commit into `main`.
5. Tag that commit, for example `v0.1.2`, and push the tag.
6. Wait for every release job and inspect the draft assets and checksums.
7. Smoke-test both supported hardware targets and rejected-hardware startup
   before manually publishing.

Never move a tag to retry a release. Fix the cause, increment the version, and
create a new tag.
