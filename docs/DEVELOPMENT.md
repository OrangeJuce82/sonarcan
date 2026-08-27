# Development and debugging

## Definition of done

Every change must provide:

1. compiling code;
2. focused tests for business rules;
3. useful structured diagnostics;
4. English documentation for public behavior;
5. no known regression in the supported vertical slice.

## Debugging workflow

For a defect:

1. capture a minimal reproduction;
2. identify the failing boundary and root cause;
3. add a failing regression test where practical;
4. implement the smallest complete correction;
5. run formatting, linting, tests, and the reproduction;
6. record any remaining limitation.

Do not hide a crash by disabling a feature without understanding the cause.

## Logging

Rust uses `tracing`. Set the log filter with `RUST_LOG`, for example:

```bash
RUST_LOG=sonarcan=debug npm run tauri dev
```

Do not log raw audio contents, credentials, or private URLs. Real-time callbacks must only update lock-free or atomic counters; formatting log messages happens elsewhere.

## Performance workflow

Measure, identify the bottleneck, change one relevant variable, measure again, and verify stability. Performance changes require evidence and must not weaken project safety or diagnostics.

## Source conventions

- Source code, comments, tests, errors, and documentation are written in English.
- Avoid `unwrap()` in production paths.
- Keep Tauri command functions thin.
- Prefer typed errors and explicit state transitions.
- Add a regression test for every important fixed defect.

