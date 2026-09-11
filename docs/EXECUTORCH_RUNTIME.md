# ExecuTorch deployment

SonArcan's target inference architecture embeds a selectively built native
ExecuTorch runtime. Python, PyTorch, CUDA development tools, model exporters,
and package managers are build-time dependencies only and must not be present
in a desktop bundle.

This is a migration contract, not permission to remove the qualified Python
workers prematurely. A release may switch a feature to ExecuTorch only after
the exported program, its native preprocessing, and its native postprocessing
pass the same audio corpus as the current production worker on every advertised
backend.

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
- A failed runtime or model probe disables analysis for the session and leaves
  playback and project editing available.

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

Production inference already uses 1500-frame log-mel windows. The network
strictly exports with that fixed shape after disabling its opportunistic rotary
frequency cache. Cache removal is inference-neutral and avoids a mutable model
state. Audio loading, log-mel calculation, window aggregation, and minimal/DBN
postprocessing remain native stages.

### SCNet

The full network strictly exports, but the portable ExecuTorch lowering rejects
its complex-valued STFT/FFT boundary. The model must therefore be partitioned
at real-valued boundaries: native STFT, encoder, six dual-path blocks separated
by native real FFT/IFFT conversions, decoder, then native inverse STFT. This
also avoids embedding a second FFT implementation.

### HTDemucs

The pinned HTDemucs network strictly exports after two inference-neutral
adaptations: `randrange(1)` becomes the constant zero, and development-only
tensor equality assertions are excluded from the graph. Portable lowering has
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

The startup order preserves the existing qualified Apple stem path: MLX stays
first on Apple Silicon until an end-to-end replacement is qualified. A Core ML
ExecuTorch program can then be introduced per feature, with CPU XNNPACK as the
final fallback. Core ML may schedule supported work on the CPU, GPU, or Neural
Engine. NVIDIA builds prefer a qualified CUDA AOTI program and fall back to the
same CPU XNNPACK programs when CUDA probing fails. ExecuTorch currently has no
production ROCm delegate;
AMD releases must use the CPU fallback until a native AMD delegate passes the
same qualification rather than silently restoring the multi-gigabyte PyTorch
ROCm distribution.

## Export audit results

The following measurements use the pinned production checkpoints, ExecuTorch
1.4.1 and PyTorch 2.14.0. They measure model programs, not the final selective
runtime. PTE files stay in the model cache and are not Tauri resources.

| Model program | Backend | PTE size | Export/reconstruction drift |
| --- | --- | ---: | ---: |
| Beat This | Core ML | 41,787,877 bytes | 0 |
| Beat This | XNNPACK | 81,343,112 bytes | 0 |
| LV-Chordia, five chunked members | XNNPACK | 15,385,360 bytes | ≤ 3.51e-5 end-to-end |
| HTDemucs neural core | Core ML | 109,717,922 bytes | 0 |
| HTDemucs neural core | portable CPU | 168,275,400 bytes | 0 |
| SCNet encoders | XNNPACK | 10,702,592 bytes | 0 |
| SCNet dual-path blocks | portable CPU | 157,466,112 bytes | 0 |
| SCNet decoders | XNNPACK | 16,276,928 bytes | 0 |

SCNet uses the portable CPU library only for its unrolled LSTM graphs. Running
the XNNPACK partition search for those graphs takes several minutes per block
without producing a useful runtime advantage; its convolutional encoders and
decoders remain delegated to XNNPACK. `SONARCAN_EXECUTORCH_PARTITIONS` can
select `encoder`, `dual-path`, `decoder`, and `mask` during targeted audits.

These numbers are feasibility evidence, not a production switch. Release
packaging remains on the existing worker until the selectively linked native
runner and the native audio boundaries pass the end-to-end corpus and device
gates above.

## Native runtime measurement

The four exported model families were loaded and executed successfully by one
selectively linked arm64 macOS ExecuTorch runtime built from the 1.4.1 release.
The current 89-program union contains 47 root operators. The
reproducible portable/XNNPACK probe is 2,395,400 bytes before stripping and
approximately 2.2 MB after stripping. The PTE programs and weights are not part of
that number and remain first-use downloads.

The measured all-ones audit inputs completed in approximately 1.7 seconds for
Beat This, under 10 milliseconds for one LV-Chordia member, 41 seconds for one
SCNet dual-path CPU partition, and 133 seconds for the HTDemucs neural core.
These timings are correctness probes rather than corpus benchmarks. They show
that the CPU path is functional without claiming it is the normal accelerated
path.

`npm run executorch:probe` reproduces the selective native build without
modifying the ExecuTorch checkout. It requires `SONARCAN_EXECUTORCH_SOURCE`,
`SONARCAN_EXECUTORCH_PTE_DIR`, and a build-time Python containing ExecuTorch.
An installation made with `pip --target` can be exposed through
`SONARCAN_EXECUTORCH_PYTHONPATH`.
The resulting probe is an audit executable; production packaging still waits
for the SonArcan audio protocol and end-to-end corpus gates above.

`npm run executorch:runtime` builds the same selective operator union into the
production tensor worker. Its `SACTEN01` protocol accepts only bounded float32
tensors from regular, non-symlink files and publishes outputs atomically. The
worker contains inference only: model export and dependency installation stay
in CI, while audio preprocessing and postprocessing remain native SonArcan
code. Building the worker alone does not authorize switching a feature away
from its qualified production path.

The reviewed operator union is pinned in
`tools/sonarcan-executorch-worker/operators.txt`, so platform builders do not
need PyTorch or the model checkpoints. Supplying `SONARCAN_EXECUTORCH_PTE_DIR`
turns the build into a drift check and fails if the release programs require a
different operator set.

Windows builds use the runner's `clang-cl`/Ninja toolchain and prebuild its host
schema compilers before the selective runtime. ExecuTorch 1.4.1 omits the
Windows executable suffix from those generated-file dependencies, so the
explicit build order avoids a Ninja dependency gap without patching vendored
sources. ExecuTorch 1.4.1 also pins an
XNNPACK C source macro that fails under the MSVC preprocessor; compiling the
same reviewed source with the Clang engine and MSVC-compatible command-line
interface preserves XNNPACK acceleration while accepting ExecuTorch's Windows
`/Gy` and `/Gw` flags. Its non-MSVC defaults also inject the POSIX `-fPIC`
option, which is disabled for the Windows PE build where it is neither
supported nor required.
