# Releasing SonArcan

SonArcan publishes one target: macOS on Apple Silicon. The application uses
MPS for chord/rhythm analysis and MLX for stem separation. There is no Intel
macOS, Windows, Linux, CUDA, ROCm, portable Torch, or CPU inference release.

## Release contract

The tag workflow must:

1. verify that `package.json`, `src-tauri/tauri.conf.json`, and
   `src-tauri/Cargo.toml` match the tag;
2. recreate only the draft associated with that tag;
3. assemble the pinned Apple-Silicon Python, MLX/MPS, FFmpeg, and yt-dlp
   resources;
4. run `npm run quality` and the runtime contract checks;
5. build only `aarch64-apple-darwin` application and DMG bundles;
6. ad-hoc sign embedded executables before signing the outer bundle;
7. exercise MPS and MLX from the assembled application;
8. upload the DMG checksum and leave the release as a draft.

The hosted runner proves packaging integrity. A human smoke test on an
Apple-Silicon Mac is still required before publication.

## Pinned inputs

- Node.js 22 and stable Rust;
- uv 0.9.26 and CPython 3.13.5 for release assembly;
- the committed npm, Cargo, and uv lockfiles;
- the pinned LV-Chordia and Beat This! revisions and hashes;
- the pinned HTDemucs and SCNet Large URLs, sizes, and SHA-256 digests;
- FFmpeg 8.0.3 and LAME 3.100 built from verified source archives;
- the pinned yt-dlp zipimport artifact.

The application does not install uv or resolve Python packages at runtime.
Beat This!, HTDemucs, and SCNet Large checkpoints are downloaded and verified
on the first qualified launch instead of being bundled in the installer.

## Local qualification

On macOS Apple Silicon:

```bash
npm ci
uv sync --project tools/sonarcan-mlx-worker --locked --reinstall-package sonarcan-scnet-infer
uv sync --project tools/sonarcan-chord-worker --locked
npm run chords:downbeat-model
npm run python:runtime
npm run verify:stem-release
npm run verify:chord-release
npm run ytdlp:search
npm run verify:ytdlp-search-release
npm run ffmpeg:runtime
npm run verify:ffmpeg-release
npm run quality
npm run verify:release-version
```

Run `npm run security` only when a library or package was added, as required by
the repository security policy.

Before publication, exercise first-run model installation, chord/rhythm
analysis, both stem profiles, import/conversion, MP3 export, cancellation, cache
reload, sleep/wake, project save/reopen, and the rejected-accelerator path.

## Cutting a release

1. Update the version in the three manifests and `RELEASE_NOTES.md`.
2. Complete local qualification and commit the exact reviewed changes.
3. Push the branch and wait for both macOS CI jobs to pass.
4. Merge or fast-forward the qualified commit to `main`.
5. Create and push an annotated matching tag, for example:

   ```bash
   git tag -a v0.1.7 -m "SonArcan 0.1.7"
   git push origin v0.1.7
   ```

6. Inspect the draft workflow, DMG, checksum, signature, and bundled resources.
7. Smoke-test the draft on Apple Silicon, then publish it manually.

GitHub builds are ad-hoc signed, not notarized or identified by Apple. Release
notes must retain the **System Settings → Privacy & Security → Open Anyway**
instruction. Never move an existing tag to retry a failed release: fix the
cause, increment the version, and create a new tag.
