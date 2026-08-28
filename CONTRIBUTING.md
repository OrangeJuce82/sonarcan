# Contributing to SonArcan

SonArcan is maintained as a professional desktop audio application. The guiding
sequence is **stabilize → clean → simplify → extract → harmonize → optimize**.
Changes should be incremental, reviewable, and behavior-preserving unless the
task explicitly changes product behavior.

## Architecture and ownership

The Svelte webview is a control surface. Components own rendering, accessibility,
and local interaction state. Reusable controls, services, types, stores, and pure
calculations belong in focused modules under `src/lib`; avoid adding unrelated
state or business logic to `App.svelte`.

Rust owns projects, imports, decoding, analysis, playback, DSP, and persistence.
Tauri command handlers remain thin and typed. Long-running or blocking work uses
bounded workers outside commands and outside the real-time callback. Extract a
new Rust module when it gives a concrete testing or ownership boundary; do not
create speculative workspace crates.

## Svelte and TypeScript

- Keep strict TypeScript enabled. Avoid `any`, unchecked casts, mutable global
  state, hidden side effects, and duplicate DTO definitions.
- Prefer small components with one clear UI responsibility. Extract pure logic
  before introducing stores or service abstractions.
- Use semantic HTML, labels, keyboard operation, visible focus, ARIA state where
  semantics need it, and `prefers-reduced-motion` for non-essential animation.
- Preserve current shortcuts and interaction behavior during visual cleanup.
- Use the existing house components and CSS custom properties. Add a shared
  component only when the same concept genuinely recurs.

## Rust

- Use typed errors for expected failures and return actionable messages at the
  IPC boundary. Avoid `unwrap()` and `expect()` outside tests and unrecoverable
  application startup.
- Keep modules cohesive, state transitions explicit, and shared mutable state
  bounded. Use immutable data or atomics on audio paths.
- Treat filesystem paths and serialized manifests as untrusted. Canonicalize
  paths before security-sensitive reads, writes, or deletions.
- Pass child-process arguments separately; never build a shell command from user
  input. Bound captured output and clean temporary artifacts.
- Add unit tests beside domain code and regression tests for every important bug.

## UI system

Global color, spacing, radii, typography, timing, and interactive states belong
in `src/styles.css` as CSS custom properties where reuse is real. Avoid new magic
values and one-off variants when an existing token or state applies. Shared
controls must cover hover, focus-visible, active/selected, and disabled states.

Visual refactoring must not silently change navigation, audio behavior, project
layout, shortcuts, or user workflows.

## Performance and loading

The UI must remain responsive during playback. Do not put decoding, analysis,
model work, synchronous filesystem access, or high-frequency IPC in the webview.
Throttle visualization snapshots to the lowest useful rate and update local UI
state locally.

Load only the selected audio immediately. Preload imminent work deliberately and
bound caches by both count and memory. Optional models and expensive analysis are
lazy, cancellable where practical, and never block initial interaction. Record a
baseline and a before/after result for performance changes.

## Security

Follow `SECURITY.md`. Minimize Tauri permissions and CSP sources. Validate IPC,
manifest, file, URL, clipboard, and external-tool inputs. Lock dependencies and
never silence an advisory without a written reason, owner, and review date.

## Validation

Before submitting a change:

```bash
npm run quality
```

This runs Svelte/TypeScript diagnostics, frontend unit tests, the production
bundle, Rust formatting, Clippy with warnings denied, and all Rust tests.

For dependency, import, IPC, filesystem, networking, process, or release changes:

```bash
npm run security
```

The change is complete only when relevant tests pass, diagnostics are clean,
documentation matches reality, accessibility is preserved, performance-critical
paths are not regressed, and no unreviewed known vulnerability remains.
