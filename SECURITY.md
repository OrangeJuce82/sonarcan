# Security policy

SonArcan is local-first, not trust-free. Project packages, imported media,
clipboard text, remote URLs, model downloads, external-tool output, and every IPC
argument are untrusted inputs. A crafted project or media file must not gain
access outside the selected project, execute a shell command, broaden webview
permissions, or exhaust resources without a bound.

## Required controls

- Keep Tauri capabilities least-privileged and the CSP closed to remote scripts.
  Remote images are limited to YouTube's fixed thumbnail origin, and the asset
  protocol stays disabled while the webview does not load media files.
- Browser links for search results receive only a validated YouTube video
  identifier; Rust constructs the fixed HTTPS watch URL before opening it.
- Canonicalize project media before reads or deletions and verify by filesystem
  identity that it remains below the project's `Audio` directory. This preserves
  equivalent Unicode path spellings on macOS while preventing symlinks and `..`
  components from bypassing the boundary.
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
- On macOS, the native document picker grants access to the selected `.sac`
  package. Verify read/write access to that package and read access to every
  referenced media file before opening it; never request its parent directory as
  a second authorization step. Verify Save As parents with a uniquely named
  create/remove probe before copying, and remove an incomplete destination
  package if the copy cannot finish.
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
  `glib 0.18` iterator advisory. The GTK path is required by Tauri's supported
  Linux WebView and is reviewed as a maintenance risk through the deadline
  above; none of these notices currently reports an exploitable vulnerability.
  A reachable memory-safety or code-execution advisory blocks Linux releases.
- Tauri's `urlpattern` graph carries five unmaintained `unic` crates.
- A build-time `proc-macro-error` maintenance notice is inherited transitively.
- PyTorch no longer publishes current Intel macOS wheels, while LV-Chordia
  requires Torch 2.13 or newer. Intel macOS release artifacts are therefore not
  produced. Windows and Linux use the pinned Torch 2.13.0 CPU runtime.

These are maintenance advisories, not permission to ignore a vulnerability.
Any advisory reporting memory safety, code execution, path escape, data loss, or
denial of service in a reachable supported-target path blocks release.

The MLX and portable Torch workers, Python interpreters, packages, and shared
model are fixed release inputs. Release assembly validates each target lockfile
and model checksum; the application does not install uv or resolve Python
packages at runtime. Linux and Windows FFmpeg archives are accepted only after
validating a checksum manifest whose own SHA-256 is pinned in source. Dependency
and model updates require a fresh audit and regenerated release resources.

## Reporting and response

Do not publish suspected exploitable details in a public issue. Report them
privately to the maintainers with the affected version, reproduction, impact, and
suggested mitigation. Maintainers should acknowledge the report, reproduce it,
classify supported targets, prepare a focused regression test and fix, rotate any
affected material, and publish an advisory before disclosing full details.
