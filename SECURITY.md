# Security policy

SonArcan is local-first, not trust-free. Project packages, imported media,
clipboard text, remote URLs, model downloads, external-tool output, and every IPC
argument are untrusted inputs. A crafted project or media file must not gain
access outside the selected project, execute a shell command, broaden webview
permissions, or exhaust resources without a bound.

## Required controls

- Keep Tauri capabilities least-privileged and the CSP closed to remote scripts.
  The asset protocol stays disabled while the webview does not load media files.
- Canonicalize project media before reads or deletions and verify that it remains
  below the project's `Audio` directory. Symlinks and `..` components must not
  bypass this boundary.
- Use direct process invocation with separate arguments and `--` before user
  values. Never interpolate user input into a shell command.
- Use HTTPS, verify downloaded executables or models against pinned hashes or a
  publisher checksum, write to a temporary file, then rename atomically.
- Bound queues, concurrency, captured logs, parsed metadata, caches, and files
  read into memory. Never expose secrets, private URLs, raw media, or personal
  file contents in logs.
- Treat analysis caches as disposable untrusted data. Acoustic fingerprint files
  have a strict read-size limit and invalid or unsupported cache versions are
  ignored and rebuilt from canonicalized project media.
- Create temporary projects with unpredictable package names below the platform
  temporary directory. Canonicalize Save As parents and reject destinations
  nested inside the source package to prevent recursive copies or path escapes.
- Keep `package-lock.json` and `Cargo.lock` committed and review dependency graph
  changes explicitly.

## Dependency audit

Run the network-backed audit only when adding a new library or package:

```bash
npm run security
```

It combines npm advisories with OSV scanning for npm and Cargo lockfiles. New or
expired advisories fail the command. Exceptions live in `osv-scanner.toml`; each
must have a concrete reason and near-term expiry.

Do not run this audit for ordinary code or documentation changes. Validate
security-sensitive code changes with focused tests and checks for the affected
trust boundary instead.

### Reviewed transitive maintenance notices

Review deadline: **2026-11-30**.

- Tauri's Linux GTK3 graph carries ten unmaintained GTK binding notices and the
  `glib 0.18` iterator advisory. That graph is not compiled into the currently
  supported macOS build. Before distributing Linux builds, update/migrate the
  upstream GUI stack or demonstrate that the affected API is unreachable.
- Tauri's `urlpattern` graph carries five unmaintained `unic` crates.
- Burn/Demucs carries unmaintained `bincode` and `paste` crates. SonArcan does not
  deserialize bincode input; these are still upgrade debt and must be reassessed
  with the pinned Demucs/Burn stack.
- A build-time `proc-macro-error` maintenance notice is inherited transitively.

These are maintenance advisories, not permission to ignore a vulnerability.
Any advisory reporting memory safety, code execution, path escape, data loss, or
denial of service in a reachable supported-target path blocks release.

Cargo's repeated future-incompatibility summary for `block 0.1.6` is disabled in
`.cargo/config.toml`; this crate is inherited from Metal/wgpu through Burn. Rust
warnings in SonArcan are still denied globally. Revisit the exception whenever
Burn/wgpu changes and no later than the review deadline above.

## Reporting and response

Do not publish suspected exploitable details in a public issue. Report them
privately to the maintainers with the affected version, reproduction, impact, and
suggested mitigation. Maintainers should acknowledge the report, reproduce it,
classify supported targets, prepare a focused regression test and fix, rotate any
affected material, and publish an advisory before disclosing full details.
