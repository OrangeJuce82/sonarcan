# macOS release and GitHub deployment

## Supported release

The complete SonArcan build targets Apple Silicon (`aarch64-apple-darwin`) and
macOS 14 or later. `demucs-mlx` uses Apple MLX, whose macOS backend requires
Apple Silicon. An Intel Tauri application can still be compiled, but it cannot
provide this stem engine and must be presented as a separate reduced edition.
Do not publish a universal binary by combining the ARM runtime with an Intel
binary: it would install successfully and fail when stems are enabled.

The direct-download DMG is the supported distribution channel. The MLX runtime
contains executable Python code, so this architecture is not suitable for a Mac
App Store build that downloads or modifies executable code after review.

## What is pinned

- uv 0.9.26 is used only for development and release assembly;
- CPython 3.13.5 is recorded by `.python-version` and the runtime builder;
- direct and transitive packages are locked in `uv.lock`;
- the official Demucs source signature and checksum are validated by
  `demucs-mlx` before conversion;
- the generated safetensors SHA-256 is embedded in its safe config and checked
  by the worker before every load;
- the relocatable Python/MLX environment and model are signed inside the final
  notarized application bundle. No package manager runs on the user's Mac.

The release workflow signs every Mach-O executable, dynamic library, and Python
extension in the embedded runtime before Tauri signs the outer application.
This explicit inner-to-outer order is required for hardened-runtime notarization;
ordinary resource copying alone does not sign nested executable code.
Because Apple signing changes the Mach-O bytes, the same step refreshes the
standard wheel `RECORD` hashes afterward. `demucs-mlx` can therefore retain its
native-extension integrity check; the outer Developer ID signature then seals
the updated runtime and records together.

## One-time Apple and GitHub setup

1. Enroll in the paid Apple Developer Program. Create a `Developer ID
   Application` certificate for distribution outside the App Store.
2. Install it in Keychain Access, expand it under **My Certificates**, and
   export the certificate plus private key as a password-protected `.p12`.
3. Convert it to one line with
   `openssl base64 -A -in DeveloperID.p12 -out DeveloperID.txt`.
4. Create an app-specific password for the Apple account used for notarization.
5. In GitHub, open **Settings → Secrets and variables → Actions** and create:
   `APPLE_CERTIFICATE` (the base64 text), `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_ID`, `APPLE_PASSWORD` (the app-specific password), and
   `APPLE_TEAM_ID`.
6. Protect the default branch and require the CI workflow before merging.

Secrets are never placed in repository files or logs. A later hardening step may
replace Apple-ID notarization with an App Store Connect API key.

## Local release qualification

Run on an Apple-silicon Mac:

```bash
npm ci
npm run mlx:sync
npm run mlx:model
npm run mlx:runtime
npm run verify:mlx-release
npm run quality
npm run security
```

Then run a real separation smoke test on representative music, inspect all six
outputs, and test cancellation, cache reload, sleep/wake, and a fresh macOS user
account. `npm run tauri build -- --target aarch64-apple-darwin --bundles app,dmg`
builds the local bundle when signing/notarization environment variables are set.

## Publishing a version

1. Update the same semantic version in `package.json`, `src-tauri/Cargo.toml`,
   and `src-tauri/tauri.conf.json`.
2. Update release notes and run the local qualification above.
3. Commit and merge the release changes.
4. Create and push the exact matching tag, for example:

   ```bash
   git tag -s v0.2.0 -m "SonArcan 0.2.0"
   git push origin v0.2.0
   ```

5. The `Release macOS Apple Silicon` workflow checks version consistency,
   restores or converts the official model, builds the pinned runtime, runs the
   complete quality gate, signs and notarizes the app, creates the DMG, and
   uploads everything to a **draft** GitHub Release.
6. Download the draft DMG on a different Apple-silicon Mac. Verify with
   `codesign --verify --deep --strict --verbose=2 /Applications/SonArcan.app`
   and `spctl --assess --type execute --verbose=4 /Applications/SonArcan.app`,
   then exercise six-stem separation.
7. Edit the generated notes and publish the draft. If any validation fails,
   delete the draft/tag, fix the versioned source, and create a new version; do
   not replace a public signed binary silently.

The workflow deliberately publishes a draft so a human validates the notarized
artifact before users see it.
