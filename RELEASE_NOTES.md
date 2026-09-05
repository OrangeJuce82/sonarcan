# SonArcan 0.1.0-beta.22

This beta introduces platform-specific Full GPU and Light releases. Choose the
installer whose name matches your computer; all editions share the same `.sac`
project format.

## Which file should I download?

| Release name | Computer | Beat, Chords, Mix | Included compute runtime |
| --- | --- | --- | --- |
| SonArcan | Apple-silicon Mac (M1 or newer) | Yes | Apple MLX and MPS |
| SonArcan NVIDIA GPU | Windows x64 or Linux x64 with a compatible NVIDIA GPU | Yes, after the startup probe succeeds | PyTorch CUDA 12.6 |
| SonArcan AMD GPU | Linux x64 with a ROCm 7.2-compatible AMD GPU | Yes, after the startup probe succeeds | PyTorch ROCm 7.2 |
| SonArcan Light | Apple-silicon Mac, Intel Mac, Windows x64, or Linux x64 | No | No ML runtime or models |

There is no AMD GPU edition for Windows in this beta. AMD's Windows support is
currently limited to selected recent GPUs and requires a separate Python 3.12
runtime, which has not yet completed SonArcan's release qualification. Use
SonArcan Light on an AMD-only Windows computer. Intel GPUs are not qualified yet.

Full GPU releases never run Beat, Chords, or Mix silently on the CPU. At every
application launch, SonArcan exercises the actual production model graphs on the detected
accelerator. If the driver, device, runtime, model, memory, or inference result
is incompatible, SonArcan enters safe degraded mode for that session. Beat,
Chords, Mix, BPM, the analysis metronome, and the piano/guitar/ukulele chord
views are hidden; playback, time navigation, lyrics, spectrum, and stereo meters
remain available. The explanation is shown once per user profile.

Light is the smallest and safest download for older hardware. It physically
excludes Torch, MLX, the analysis models, and the chord-instrument frontend
assets rather than merely hiding them.

## GPU download format

CUDA and ROCm runtimes are too large for GitHub's 2 GiB limit per release file.
Each GPU package is therefore portable and split into numbered `part-000`,
`part-001`, … files, accompanied by a platform/backend-specific `SHA256SUMS`
file. Download every part for one edition, verify every checksum, and concatenate
the parts in filename order. The result is a `.deb` on Linux or a `.zip`
on Windows. The README contains copy-and-paste reconstruction commands. Light
and macOS downloads remain conventional single-file installers.

## Other fixes

- Windows desktop builds no longer leave a console window open.
- Intel macOS FFmpeg assembly now falls back safely when NASM is unavailable.
- Full and Light editions use distinct product names and bundle identifiers.

See the README for detailed minimum configurations and installation guidance.
