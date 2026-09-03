# Cross-platform release and GitHub deployment

## Supported releases

The tag workflow produces native bundles for Apple Silicon
(`aarch64-apple-darwin`), macOS Intel (`x86_64-apple-darwin`), Windows x64
(`x86_64-pc-windows-msvc`), and Linux x64 (`x86_64-unknown-linux-gnu`). Apple
Silicon bundles contain MLX; the other three contain portable CPU Torch. Both
backends load the same converted model resource and expose the complete six-stem
feature. Never copy a Python runtime between targets or combine them into a
universal macOS binary.

CI compiles the Tauri application on all four targets for every change. Tag
builds additionally assemble the target-native Python and FFmpeg resources,
execute their self-tests, and upload platform installers to one draft release.
After packaging, the release gate inspects the macOS applications, extracts both
Linux packages, and silently installs the Windows NSIS package in the disposable
runner. It then executes the embedded chord/downbeat, stem, FFmpeg, FFprobe, and
yt-dlp health checks from those packaged locations. A missing, foreign-architecture,
or non-relocatable runtime therefore fails the release while it is still a draft.

## What is pinned

- uv 0.9.26 is used only for development and release assembly;
- CPython 3.13.5 is recorded by `.python-version` for the MLX stem runtime;
- the chord worker embeds CPython 3.12.12 and the exact LV-Chordia revision
  `9d7de7bbf45efa6731ec8dc62d35280f141c0702`;
- the official `yt-dlp` zipimport artifact is pinned by version and SHA-256 in
  `src-tauri/resources/ytdlp-search/manifest.json` and runs through the shared
  Python 3.12 resolver;
- direct and transitive packages are locked in `uv.lock`;
- the official Demucs source signature and checksum are validated by
  `demucs-mlx` before conversion;
- the generated safetensors SHA-256 is embedded in its safe config and checked
  by the worker before every load;
- FFmpeg 8.0.3 and LAME 3.100 are built from their verified source archives as
  static ARM64 command-line tools; their source SHA-256 values are recorded in
  `scripts/build-ffmpeg-runtime.sh` and the generated runtime manifest;
- portable Torch 2.2.2 CPU wheels are resolved from PyTorch's pinned CPU index;
- BtbN Linux and Windows FFmpeg archives are selected from one immutable release
  tag and verified through a checksum manifest whose SHA-256 is pinned in source;
- target-native Python environments, the shared model, FFmpeg, and FFprobe are
  bundled. No package manager runs on an end-user machine.

The release workflow signs every Mach-O executable, dynamic library, and Python
extension in the embedded runtime before Tauri signs the outer application.
This explicit inner-to-outer order keeps the Apple Silicon bundle internally
consistent; ordinary resource copying alone does not sign nested executable code.
Because Apple signing changes the Mach-O bytes, the same step refreshes the
standard wheel `RECORD` hashes afterward. `demucs-mlx` can therefore retain its
native-extension integrity check; the outer ad-hoc signature then seals the
updated runtime and records together.

## Distribution trust model

GitHub releases use the ad-hoc signing identity (`-`). This does not require an
Apple Developer Program membership or repository secrets, and it prevents
Apple Silicon from treating the embedded executables as completely unsigned.
It does not identify the publisher to Apple and it cannot provide notarization.

Gatekeeper therefore blocks the first launch of a downloaded release. After
trying to open SonArcan, users must open **System Settings → Privacy & Security**,
scroll to **Security**, choose **Open Anyway**, and confirm. macOS remembers that
choice for the installed application. Release notes must disclose this step and
must never claim that Apple reviewed, verified, or notarized the build.

The workflow uploads a SHA-256 checksum next to the DMG so users can verify the
download independently. If the project later adopts Developer ID, replace the
ad-hoc identity with the certificate-backed signing and notarization flow before
removing the Gatekeeper disclosure.

## Local release qualification

Prepare the backend for the current build host (`mlx:*` on Apple Silicon,
`stems:*` elsewhere), then assemble the common analysis and media resources:

```bash
npm ci
npm run stems:sync
npm run stems:runtime
npm run verify:stem-release
npm run chords:runtime
npm run verify:chord-release
npm run ytdlp:search
npm run verify:ytdlp-search-release
npm run ffmpeg:runtime
npm run verify:ffmpeg-release
npm run quality
```

Run `npm run security` as well only when the release changes a dependency or
lockfile, in accordance with the repository security policy.

On Apple Silicon, replace the three portable-stem commands with `mlx:sync`,
`mlx:model`, and `mlx:runtime`. Build with the target overlay
`src-tauri/tauri.macos-arm.conf.json` or
`src-tauri/tauri.portable.conf.json`. macOS can still use
`npm run register:macos-app` for local Launch Services qualification.

Then run a real separation smoke test on representative music, inspect all six
outputs, import a YouTube result that requires conversion, export stems as MP3,
and test cancellation, cache reload, sleep/wake, and a fresh macOS user account.
Set `APPLE_SIGNING_IDENTITY=-` before a local release build so Tauri and the
embedded-runtime signing script use the same ad-hoc identity.

## Publishing a version

1. Update the same semantic version in `package.json`, `src-tauri/Cargo.toml`,
   and `src-tauri/tauri.conf.json`.
2. Update release notes and run the local qualification above.
3. Commit and merge the release changes.
4. Create and push the exact matching tag, for example:

   ```bash
   git tag -a v0.2.0-beta.1 -m "SonArcan 0.2.0 beta 1"
   git push origin v0.2.0-beta.1
   ```

5. The `Release desktop` workflow checks version consistency, converts the
   shared model once on Apple Silicon, then builds the MLX ARM bundle and the
   portable Intel, Windows, and Linux bundles. Every runtime and media tool is
   verified before packaging into a **draft** GitHub Release.
6. The workflow verifies the application icons, macOS `.sac` document-package
   declaration, shared-model identity, and bundled executables.
7. Download and smoke-test every draft installer. On macOS, verify with
   `codesign --verify --deep --strict --verbose=2 /Applications/SonArcan.app`,
   confirm that Gatekeeper initially blocks the unidentified build, authorize it
   with **System Settings → Privacy & Security → Open Anyway**, confirm that
   Finder and the Dock show the SonArcan icon, then exercise
   YouTube import/conversion, MP3 export, and six-stem separation without
   installing Homebrew or FFmpeg.
8. Exercise import, chord/downbeat analysis, stem separation, playback, save,
   and project reopening on each OS. Edit the generated notes and publish the
   draft. If any validation fails,
   delete the draft/tag, fix the versioned source, and create a new version; do
   not replace a public signed binary silently.

The workflow deliberately publishes a draft so a human validates the ad-hoc
signed artifact and its Gatekeeper instructions before users see it.
