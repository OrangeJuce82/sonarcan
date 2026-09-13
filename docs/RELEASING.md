# Releasing SonArcan

SonArcan publishes one application target: macOS Apple Silicon, using the MLX
delegate through a selective native ExecuTorch runtime. There is no CPU or
Python/PyTorch runtime fallback.

## Release contract

The tag workflow is authoritative. It must:

1. verify that `package.json`, `src-tauri/tauri.conf.json`, and
   `src-tauri/Cargo.toml` contain the tag version;
2. recreate only the draft matching that tag;
3. build and exercise the selective arm64 ExecuTorch worker with MLX;
4. run `npm run quality:shared` before packaging;
5. create an ad-hoc-signed `.app` and DMG for `aarch64-apple-darwin`;
6. verify the assembled resources, native linkage, signature, and 512 MiB cap;
7. upload `SHA256SUMS.txt` and leave the release as a draft.

The ad-hoc signature checks internal bundle consistency; it is neither Apple
identity nor notarization. FFmpeg, yt-dlp, and the native inference worker are
exercised after signing, followed by a second deep signature verification.

Model programs remain first-use downloads. Exporters and reference workers may
use external Python/PyTorch environments, but those environments cannot enter
application resources.

## Cutting a release

1. Update the version in all three manifests and in `RELEASE_NOTES.md`.
2. Run `npm run quality`. Run `npm run security` only if dependencies changed.
3. Push `dev` and wait for every macOS CI job to pass.
4. Fast-forward `main` to the exact qualified commit.
5. Tag that commit, for example `v0.1.6`, and push the tag.
6. Inspect the draft DMG, checksum, bundle size, and workflow result.
7. Smoke-test startup, model installation, analysis, and both stem profiles on
   Apple Silicon before publishing the draft.

Never move a tag to retry a release. Fix the cause, increment the version, and
create a new tag.
