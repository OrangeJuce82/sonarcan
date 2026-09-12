# ExecuTorch deployment

SonArcan's target inference architecture embeds a selectively built native
ExecuTorch runtime. Python, PyTorch, CUDA development tools, model exporters,
and package managers are build-time dependencies only and must not be present
in a desktop bundle.

Release builds are native-only. Python and PyTorch remain confined to external
development environments used for export and parity tests; their former runtime
workers and packagers have been removed. A native feature stays unavailable
until its exported program, preprocessing, and postprocessing pass the reference
corpus.

## Runtime topology

```text
macOS arm64                         Linux x86_64 / arm64
Tauri / Rust                       Tauri / Rust
  │ native DSP                       │ native DSP
  └─ InferenceBackend                └─ InferenceBackend
       └─ MLX                             └─ CUDA (NVIDIA required)
              │                                 │
              └──── minimal ExecuTorch C++ ─────┘
                              │
                     backend-specific .pte
```

The deprecated MPS delegate is removed. Apple Silicon uses MLX. Linux requires
NVIDIA and uses CUDA exclusively. A missing device,
driver, CUDA runtime, delegate, or qualified model leaves the startup screen in
its blocking incompatibility state. There is no XNNPACK, portable CPU, Python,
or PyTorch production fallback. No code outside
`InferenceBackend` may depend on a delegate name or API.

## Migration status

| Model | Export | Native graph runner | Native audio boundary | Corpus parity | Production |
| --- | --- | --- | --- | --- | --- |
| Beat This | fixed MLX program | validated on Apple Silicon GPU | native log-mel, fixed-window aggregation, and DBN/minimal decoding | macOS passed; CUDA pending | macOS candidate |
| LV-Chordia | 95 fully delegated MLX programs | validated on Apple Silicon GPU | native CQT bank, recurrent orchestration, ensemble, and XHMM decoder | macOS passed; CUDA pending | macOS candidate |
| HTDemucs | fully delegated MLX neural core | validated on Apple Silicon GPU | native STFT/ISTFT, masking, normalization, and overlap-add | macOS passed; CUDA pending | macOS candidate |
| SCNet | 12 fully delegated MLX partitions | validated on Apple Silicon GPU | native STFT/ISTFT, real FFT boundaries, and overlap-add | macOS numeric parity passed; expected HQ cost measured; CUDA pending | macOS candidate |

The first native runtime milestone records model load time, inference time,
wall time, peak RSS, runtime size, PTE size, and RTF when an audio duration is
known. The selected backend must come from the verified artifact manifest; the final
qualification report must additionally prove the delegate executed. Merely
linking or selecting a delegate is not accepted as that proof.

## Artifact boundary

- The desktop bundle contains the ExecuTorch core, the selected backend
  delegate, only the kernels and dtypes referenced by SonArcan programs, and
  the native SonArcan worker.
- Model programs and weights remain first-use downloads in the application data
  directory. They are versioned, size-bounded, and SHA-256 verified before an
  atomic install, like the existing model checkpoints.
- Exporting and lowering happen in CI. No Python code generation, compilation,
  dependency resolution, or network access occurs in an audio callback.
- Audio decoding, resampling, chunking, overlap-add, FFT boundaries, and result
  validation belong to the native worker. Existing Rust FFmpeg and FFT support
  should be reused instead of linking duplicate Python scientific stacks.
- A failed hardware, runtime, delegate, model-pack, or startup verification
  keeps the blocking startup screen visible. The workspace, playback, analysis,
  and project editing are not accessible in a reduced mode.

## Model adaptations

### LV-Chordia

The production network consumes the complete CQT sequence; exporting the whole
bidirectional LSTM from a 16-frame example would incorrectly freeze the song
length. Each ensemble member is therefore decomposed into ten fixed convolution
programs, fixed 16-frame and one-frame forward/backward recurrent programs, and
a fixed classifier program. Native orchestration computes InstanceNorm over the
complete sequence, carries recurrent states across chunks, and reverses the
backward stream. This preserves full-song context without a worst-case dynamic
memory arena. A 37-frame reconstruction of every member differs from eager
PyTorch by at most `3.51e-5` in logits; every individual program has zero export
drift. The CQT extractor and XHMM chord decoder stay outside the graph and must
be ported to bounded native code.

### Beat This!

Production inference uses a fixed 1500-frame MLX program. Rust pads short
inputs and splits long inputs into the exact reference windows before keep-first
aggregation. Disabling the opportunistic rotary frequency cache is
inference-neutral and avoids mutable model state. The native FP32 log-mel
implementation matches the pinned 22.05 kHz, 1024-point FFT, 441-sample hop,
128-band Slaney reference. Resampling and both minimal and DBN postprocessing
are native.

### SCNet

The full network strictly exports, but ExecuTorch delegates do not own its
complex-valued STFT/FFT boundary. The model is therefore partitioned
at real-valued boundaries: native STFT, encoder, six dual-path blocks separated
by native real FFT/IFFT conversions, decoder, then native inverse STFT. This
also avoids embedding a second FFT implementation.

### HTDemucs

The pinned HTDemucs network strictly exports after two inference-neutral
adaptations: `randrange(1)` becomes the constant zero, and development-only
tensor equality assertions are excluded from the graph. Delegate lowering has
the same complex-tensor limitation as SCNet. Spectrogram creation, complex mask
arithmetic, inverse spectrogram, split/overlap processing, normalization, and
stem writing must remain native while the real-valued neural blocks are lowered
to ExecuTorch.

## Qualification and size gates

`npm run executorch:audit` exercises strict `torch.export` capture with the real
architectures and checkpoints. It compares eager and exported outputs and fails
on any drift above `1e-5` per program. The LV-Chordia multi-program
reconstruction gate allows `5e-5` for floating-point convolution accumulation
order. Backend release jobs must additionally:

1. lower every program for the target backend;
2. build one runtime from the union of operators used by every release PTE,
   with dtype selection where the backend supports it;
3. run native and reference workers on the same bounded audio corpus;
4. compare timelines, labels, waveforms, NaN handling, cancellation, and error
   behavior;
5. record native runtime and packaged installer sizes;
6. reject any bundle containing a Python interpreter, `site-packages`, libtorch,
   exporter code, tests, training kernels, profilers, or model checkpoints.

Every promoted artifact must publish a versioned JSON report containing the
reference and candidate revisions, result comparison and threshold, precision,
inference time, RTF, peak RSS, load time, runtime size, PTE size, and evidence
for the backend that actually executed. `npm run verify:executorch-qualification
-- REPORT.json` rejects incomplete reports, failed parity, and precision other
than FP32 or FP16. The report gate is necessary but does not replace listening
tests for stem-separation changes.

The native fallback order is GPU-only as described above.
The selective worker is capped at 96 MiB and the complete package at 512 MiB;
either excess fails release. The worker links only the target GPU delegates,
Apple system frameworks where applicable, and the minimal native runtime.
The release verifier also inspects native dependencies and rejects `libpython`,
`libtorch`, and paths under `site-packages`. Analysis is enabled only after every required
native graph and audio pipeline pass the bounded startup probe.

## Export audit results

The following measurements use the pinned production checkpoints, ExecuTorch
1.4.1 and PyTorch 2.13.0. They measure model programs, not the final selective
runtime. PTE files stay in the model cache and are not Tauri resources.

| Model program | Backend | PTE size | Export/reconstruction drift |
| --- | --- | ---: | ---: |
| Beat This | MLX | 81,123,584 bytes | 0 |
| LV-Chordia, 95 chunked programs | MLX | 25,972,240 bytes | ≤ 5.066e-5 end-to-end |
| HTDemucs neural core | MLX | 168,208,896 bytes | 0 graph and split drift |
| SCNet, 12 partitions | MLX | 172,901,912 bytes | 0 export drift |

Every program in these macOS packs is fully delegated to MLX and contains no
portable ATen operator. These results qualify the model boundary and native
orchestration; CUDA exports and corpus measurements remain separately required
before Linux packages can be promoted.

## Native runtime measurement

All four model families were loaded and executed successfully by one
selectively linked arm64 macOS ExecuTorch 1.4.1 runtime. The stripped worker and
its MLX metallib total 6,901,776 bytes. The model programs remain first-use
downloads in two SHA-256-pinned packs: 204,178,291 bytes for chord/rhythm and
341,114,095 bytes for stem separation. Both include the applicable model
license notices and remain below the 512 MiB release
limit.

On the bounded native corpus, HTDemucs matched the reference probes within
`1e-7`; its complete Rust STFT/ISTFT pipeline differed by 0.35% in aggregate
energy and completed in 7.01 seconds. SCNet matched reference probes within
`8e-7`, peak amplitude within `3e-7`, and aggregate energy within 0.05%. Its
first 11-second HQ chunk took approximately 383 seconds including graph loading
and compilation; a complete warm validation took 329.5 seconds and reached
2,069,200,896 bytes maximum RSS in the qualification harness. This is accepted
as the expected cost of the explicitly selected HQ profile, not hidden by a
lower-quality model or CPU fallback. The production report must continue to
expose this timing, memory use, and the backend used.

`npm run executorch:probe` reproduces the selective native build without
modifying the ExecuTorch checkout. It requires `SONARCAN_EXECUTORCH_SOURCE`,
`SONARCAN_EXECUTORCH_PTE_DIR`, and a build-time Python containing ExecuTorch.
An installation made with `pip --target` can be exposed through
`SONARCAN_EXECUTORCH_PYTHONPATH`.
The resulting probe is an audit executable and does not replace the end-to-end
corpus gates above.

`npm run executorch:runtime` builds the same selective operator union into the
production tensor worker. Its `SACTEN01` protocol accepts only bounded float32
tensors from regular, non-symlink files and publishes outputs atomically. The
worker contains inference only: model export and dependency installation stay
in CI, while audio preprocessing and postprocessing remain native SonArcan
code.

The reviewed operator union is pinned in
`tools/sonarcan-executorch-worker/operators.txt`, so platform builders do not
need PyTorch or the model checkpoints. Supplying `SONARCAN_EXECUTORCH_PTE_DIR`
turns the build into a drift check and fails if the release programs require a
different operator set.
