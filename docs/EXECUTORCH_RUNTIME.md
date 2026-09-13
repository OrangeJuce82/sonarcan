# ExecuTorch deployment

SonArcan targets macOS Apple Silicon only. Its application bundle embeds a
selectively built native ExecuTorch 1.4.1 worker with the MLX delegate. Python,
PyTorch, exporters, package managers, and training checkpoints remain external
development inputs.

## Runtime topology

```text
SonArcan (Tauri / Rust)
  │
  ├─ audio I/O, DSP, preprocessing and postprocessing in Rust/C++
  │
  └─ InferenceBackend
       │ minimal native process protocol
       └─ ExecuTorch C++
            └─ MLX → integrated Apple GPU
                 └─ backend-specific .pte programs
```

No code outside `InferenceBackend` depends on a delegate API. There is no CPU,
Python, or PyTorch production fallback. Startup remains blocked unless the
worker reports MLX and its bounded on-device probe succeeds.

## Production model boundary

| Model | Native program | Native audio boundary | Qualification |
| --- | --- | --- | --- |
| Beat This | fixed MLX program | Rust log-mel, window aggregation, minimal and DBN decoding | passed on Apple Silicon |
| LV-Chordia | 95 fully delegated MLX programs | Rust CQT, recurrent orchestration, ensemble and XHMM decoding | passed on Apple Silicon |
| HTDemucs | fully delegated MLX neural core | native STFT/ISTFT, masking, normalization and overlap-add | passed on Apple Silicon |
| SCNet | 12 fully delegated MLX partitions | native STFT/ISTFT, real FFT boundaries and overlap-add | passed on Apple Silicon |

All production PTEs are fully delegated and contain no portable ATen operator.
The model packs are installed under the application-data directory, verified by
exact byte count and SHA-256, and atomically published. They are not Tauri
resources and do not increase the DMG size.

## Qualification and size gates

Every promoted model records audio/timeline parity, inference and load time,
RTF, peak RSS, runtime size, PTE size, precision, and evidence that MLX actually
executed. `npm run verify:executorch-qualification -- REPORT.json` accepts only
the `macos-arm64` target, the `mlx` backend, FP32 or FP16, a passed comparison,
and complete positive measurements.

Release verification rejects embedded interpreters, `site-packages`, libtorch,
development checkpoints, and an unqualified native delegate. The selective
worker is capped at 96 MiB and the complete app at 512 MiB.

Current measured artifacts:

| Program set | PTE size | Export/reconstruction drift |
| --- | ---: | ---: |
| Beat This | 81,123,584 bytes | 0 |
| LV-Chordia | 25,972,240 bytes | ≤ 5.066e-5 end-to-end |
| HTDemucs | 168,208,896 bytes | 0 graph and split drift |
| SCNet | 172,901,912 bytes | 0 export drift |

The stripped worker plus MLX metallib measured 6,901,776 bytes. The immutable
model packs measure 204,178,291 bytes for chord/rhythm and 341,114,095 bytes for
stem separation.

On the bounded native corpus, HTDemucs matched reference probes within `1e-7`;
its complete native STFT/ISTFT pipeline differed by 0.35% in aggregate energy
and completed in 7.01 seconds. SCNet matched reference probes within `8e-7`,
peak amplitude within `3e-7`, and aggregate energy within 0.05%. Its complete
warm 11-second validation took about 329.5 seconds and reached 2,069,200,896
bytes maximum RSS. This is the accepted cost of the explicit HQ profile.

## Development export

`npm run executorch:audit` uses real checkpoints to compare eager PyTorch and
exported programs. `npm run executorch:probe` reproduces the selective native
build. Both require an external ExecuTorch checkout and build-time Python via
`SONARCAN_EXECUTORCH_SOURCE`, `SONARCAN_EXECUTORCH_PTE_DIR`, and
`SONARCAN_EXECUTORCH_PYTHON`. Nothing from that environment is copied into the
desktop application.
