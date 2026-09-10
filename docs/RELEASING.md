# Cross-platform release and GitHub deployment

## Supported releases

The tag workflow produces one SonArcan application with target-specific compute
runtimes: MLX/MPS for Apple Silicon, NVIDIA CUDA 12.6 for Windows/Linux, AMD
ROCm 7.2 for Linux, and standard Windows/Linux builds that run in simplified
mode without GPU analysis. The Beat This!, HTDemucs, and SCNet checkpoints are
downloaded and verified only when a qualified accelerator build starts; they are
not bundled. Never copy a runtime between targets or combine target
architectures into a universal macOS binary.
The pinned `madmom` revision omits its setuptools, NumPy, and Cython build
requirements, so release assembly bootstraps those exact inputs before building
it without isolation.

CI compiles the Tauri application on all three targets for every change. Tag
builds additionally assemble the target-native Python and FFmpeg resources,
execute their self-tests, and upload platform packages to one draft release.
Apple Silicon bundle verification also executes the same MLX
accelerator probes used at application startup. Hosted Windows/Linux runners do
not have production GPUs, so their release gate verifies the pinned CUDA/ROCm
runtime identity and importable model contracts. The complete selected graph is
first exercised when the user starts separation.
After packaging, the release gate inspects the macOS application, extracts all
Linux package formats, and silently installs the standard Windows NSIS package in the
disposable runner. The Windows GPU portable tree is verified before its Zip64
archive is split. It then executes the embedded chord/downbeat, stem, FFmpeg,
FFprobe, and yt-dlp health checks from those packaged locations. A missing,
foreign-architecture, or non-relocatable runtime therefore fails the release
while it is still a draft.

Standard Linux publishes verified DEB and AppImage bundles. Linux GPU builds
publish verified DEB bundles split into numbered volumes smaller than 2 GiB.
Windows GPU publishes a similarly split portable Zip64
archive because NSIS and GitHub Release assets both have 2 GiB limits. Each
multipart package has a format-specific checksum file with SHA-256 hashes for
all parts. Parts must be concatenated byte-for-byte in filename order before
use; the reconstructed package or archive is never uploaded as an oversized
asset.

## What is pinned

- uv 0.9.26 is used only for development and release assembly;
- CPython 3.13.5 is used by the shared chord/downbeat and stem runtime;
- the chord worker pins the exact LV-Chordia revision
  `9d7de7bbf45efa6731ec8dc62d35280f141c0702` and `audioop-lts` for pydub's
  Python 3.13 compatibility path;
- the official `yt-dlp` zipimport artifact is pinned by version and SHA-256 in
  `src-tauri/resources/ytdlp-search/manifest.json` and runs through the shared
  Python 3.13 resolver;
- direct and transitive packages are locked in `uv.lock`;
- SCNet inference source is vendored from `openmirlab/scnet-infer` at
  revision `a5437e37c8b942baf74529f35a719aa70dfa9bdc`;
- the SCNet Large by starrytong v1.0.9 checkpoint URL, 168,852,258-byte length,
  and SHA-256 `65900dfa07d6b6e5d784c0f143920200a4bd281d6e78a806c549d0b912d5885e`
  are pinned and checked before every tensor-only load;
- the official HTDemucs `955717e8-8726e21a.th` URL, 84,141,911-byte length,
  and full SHA-256 `8726e21a993978c7ba086d3872e7608d7d5bfca646ca4aca459ffda844faa8b4`
  are pinned and checked before every load;
- starrytong's public clarification licenses the SCNet and SCNet-large weights
  under MIT and permits redistribution with attribution; retain
  `https://github.com/starrytong/SCNet/issues/35` in release notices;
- FFmpeg 8.0.3 and LAME 3.100 are built from their verified source archives as
  static ARM64 command-line tools; their source SHA-256 values are recorded in
  `scripts/build-ffmpeg-runtime.sh` and the generated runtime manifest;
- NVIDIA releases resolve Torch 2.13.0 from PyTorch's pinned CUDA 12.6 index;
- AMD Linux releases resolve Torch 2.13.0 from PyTorch's pinned ROCm 7.2 index;
- BtbN Linux and Windows FFmpeg archives are selected from one immutable release
  tag and verified through a checksum manifest whose SHA-256 is pinned in source;
- target-native Python environments, LV-Chordia models, FFmpeg, and FFprobe
  are bundled. Beat This! and stem checkpoints are first-run network downloads; no
  package manager runs on an end-user machine.

The release workflow signs every Mach-O executable, dynamic library, and Python
extension in the embedded runtime before Tauri signs the outer application.
This explicit inner-to-outer order keeps the Apple Silicon bundle internally
consistent; ordinary resource copying alone does not sign nested executable code.
Because Apple signing changes the Mach-O bytes, the same step refreshes the
standard wheel `RECORD` hashes afterward. The outer ad-hoc signature then seals
the updated runtime and records together.

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

Assemble the shared Python runtime and common media resources:

```bash
npm ci
npm run python:runtime
npm run verify:stem-release
npm run verify:chord-release
npm run ytdlp:search
npm run verify:ytdlp-search-release
npm run ffmpeg:runtime
npm run verify:ffmpeg-release
npm run quality
```

Run `npm run security` as well only when the release changes a dependency or
lockfile, in accordance with the repository security policy.

On Apple Silicon, run `mlx:sync` before assembling the shared runtime. Build with
the target overlay `src-tauri/tauri.macos-arm.conf.json`. macOS can still use
`npm run register:macos-app` for local Launch Services qualification.

Then run a real separation smoke test on representative music, allow and verify
the first-use model download, inspect all four outputs, import a YouTube result
that requires conversion, export stems as MP3,
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

5. The `Release desktop` workflow checks version consistency, creates the
   **draft** GitHub Release, then runs the macOS, standard Windows/Linux, NVIDIA GPU, and AMD GPU
   jobs concurrently. Every runtime and media tool is verified before packaging.
6. The workflow verifies the application icons, macOS `.sac` document-package
   declaration, bundled executables, and absence of Beat This!, HTDemucs, and
   SCNet checkpoints.
7. Download and smoke-test every draft package. Reconstruct every multipart GPU
   DEB and Zip64 archive and verify its platform-specific part hashes first.
   Install both GPU DEBs on compatible systems and exercise the standard AppImage
   directly on a distribution without DEB support.
   On macOS, verify with
   `codesign --verify --deep --strict --verbose=2 /Applications/SonArcan.app`,
   confirm that Gatekeeper initially blocks the unidentified build, authorize it
   with **System Settings → Privacy & Security → Open Anyway**, confirm that
   Finder and the Dock show the SonArcan icon, then exercise
   YouTube import/conversion, MP3 export, and four-stem separation without
   installing Homebrew or FFmpeg.
8. Exercise import, chord/downbeat analysis, stem separation, playback, save,
   and project reopening on each OS. Edit the generated notes and publish the
   draft. If any validation fails,
   delete the draft/tag, fix the versioned source, and create a new version; do
   not replace a public signed binary silently.

The workflow deliberately publishes a draft so a human validates the ad-hoc
signed artifact and its Gatekeeper instructions before users see it.
