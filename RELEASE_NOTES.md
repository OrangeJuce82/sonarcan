# SonArcan 0.1.0-beta.21

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

There is no AMD GPU edition for Windows in this beta because the official
PyTorch ROCm packages are Linux-only. Use SonArcan Light on an AMD-only Windows
computer. Intel GPUs are not qualified yet.

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

## Other fixes

- Windows desktop builds no longer leave a console window open.
- Intel macOS FFmpeg assembly now falls back safely when NASM is unavailable.
- Full and Light editions use distinct product names and bundle identifiers.

See the README for detailed minimum configurations and installation guidance.
