# SonArcan engineering rules

These rules apply to every change in this repository. Read
`CONTRIBUTING.md`, `SECURITY.md`, `docs/ARCHITECTURE.md`, and
`docs/AUDIO_ENGINE.md` before modifying the corresponding subsystem.

## Priority order

Stabilize, clean, simplify, extract, harmonize, then optimize. Preserve working
behavior unless a requested change or demonstrated defect requires otherwise.
Do not trade real-time audio stability, security, accessibility, or project data
safety for delivery speed.

## Required boundaries

- Svelte components render and coordinate user interaction. Put reusable UI in
  `src/lib`, pure presentation or domain calculations in TypeScript modules, and
  all audio ownership in Rust.
- Tauri commands validate and translate IPC values, then delegate to focused
  Rust modules. They do not contain business rules or long-running work.
- The CPAL callback performs no I/O, logging, allocation, mutex locking, Tauri
  call, model inference, or UI work.
- Raw audio and full-resolution analysis data never cross JSON IPC. Send bounded
  state snapshots and visualization data.
- Keep dependencies explicit and interfaces typed. Do not introduce `any`, broad
  Tauri capabilities, shell interpolation, or an abstraction without a repeated
  concrete use.

## Change discipline

- Prefer small extractions with focused tests over broad rewrites.
- Reuse and improve the existing house UI components and CSS tokens. Preserve
  shortcuts, semantics, focus behavior, and reduced-motion support.
- Treat project manifests, media, clipboard contents, URLs, tool output, and IPC
  arguments as untrusted. Canonicalize paths before reading or deleting, bound
  resource use, use argument arrays for child processes, and keep permissions
  least-privileged.
- Do not log credentials, private URLs, raw media, or personal file contents.
- Document measured performance work; do not claim an optimization without a
  before/after measurement.

## Definition of done

Run `npm run quality`. For dependency or trust-boundary changes, also run
`npm run security`. Both commands must finish with no project warning, error,
test failure, or unreviewed advisory. Add a regression test for important fixes
and update documentation when a boundary, behavior, or accepted risk changes.
