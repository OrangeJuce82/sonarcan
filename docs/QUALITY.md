# Quality baseline and improvement plan

This document turns the repository engineering rules into an incremental plan.
It is intentionally not a rewrite roadmap.

## Current baseline

The frontend uses strict TypeScript and Svelte diagnostics. Rust formatting and
Clippy warnings are hard gates. Domain tests cover project persistence, imports,
waveforms, tempo, stems, and audio rendering. The security suite audits both
lockfiles with npm and OSV.

Known structural hotspots are `src/App.svelte`, `audio_engine.rs`, `importer.rs`,
and `project.rs`. Their size alone is not a reason to rewrite them. Extract a
responsibility only when its inputs, outputs, invariants, and regression tests are
clear. Pure presentation calculations are the first frontend extraction because
they can be tested without changing component behavior.

## Next extractions

1. Move import-center state and orchestration behind a focused frontend service
   and component boundary while preserving cancellation and project snapshots.
2. Separate playlist editing and waveform interaction components from the shell.
3. Split Rust command registration from application services without moving
   business rules into commands.
4. Isolate cache serialization and process execution helpers in Rust after their
   safety contracts are covered by tests.
5. Introduce performance measurements for startup-to-interactive, track-select to
   audible-ready, callback underruns, UI frame time, cache memory, and IPC rate.

Each extraction is a separate behavior-preserving change. Do not combine broad
visual changes, functional work, dependency upgrades, and architecture changes in
one patch.

## Performance budgets to establish

Measurements must state hardware, build profile, fixture, and sample count. Track:

- startup to first usable interaction;
- selected-track load latency, warm and cold;
- audio callback underruns and maximum callback time;
- main-thread long tasks and visualization update rate;
- decoded and stem cache memory high-water marks;
- bundle size and startup JavaScript execution.

No fixed numeric promise is made until repeatable baselines exist. Regressions
must be justified or fixed before release.
