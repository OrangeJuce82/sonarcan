"""Bounded SonArcan worker protocol around the vendored SCNet runtime."""

from __future__ import annotations

import argparse
import json
import os
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Sequence

import numpy as np

from .api import SCNetSession
from .checkpoints import get_spec

MODEL_ID = "scnet-large-starrytong-v1.0.9"
MODEL_NAME = "SCNet Large by starrytong"
MODEL_URL = "https://github.com/ZFTurbo/Music-Source-Separation-Training/releases/download/v1.0.9/SCNet-large_starrytong_fixed.ckpt"
MODEL_SHA256 = "65900dfa07d6b6e5d784c0f143920200a4bd281d6e78a806c549d0b912d5885e"
MODEL_BYTES = 168_852_258
SAMPLE_RATE = 44_100
STEM_NAMES = ("vocals", "drums", "bass", "other")


def emit(event_type: str, **fields: Any) -> None:
    payload = {"type": event_type, **fields}
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=True) + "\n")
    sys.stdout.flush()


def validate_contract() -> None:
    spec = get_spec(MODEL_ID)
    if (
        spec.sample_rate != SAMPLE_RATE
        or set(spec.sources) != set(STEM_NAMES)
        or spec.url != MODEL_URL
        or spec.sha256 != MODEL_SHA256
        or spec.size != MODEL_BYTES
    ):
        raise RuntimeError("the pinned SCNet Large checkpoint contract is invalid")
    if spec.license != "MIT":
        raise RuntimeError("the pinned SCNet checkpoint license metadata drifted")


def choose_torch_device(torch: Any) -> str:
    requested = os.environ.get("SONARCAN_TORCH_DEVICE", "auto").lower()
    available = {"cpu"}
    if torch.cuda.is_available():
        available.add("cuda")
    if requested == "auto":
        return "cuda" if "cuda" in available else "cpu"
    if requested not in available:
        raise RuntimeError(f"the requested Torch device is unavailable: {requested}")
    return requested


def decode_audio(ffmpeg: Path, input_path: Path, raw_path: Path) -> np.ndarray:
    command = [
        str(ffmpeg), "-hide_banner", "-loglevel", "error", "-nostdin", "-y",
        "-i", str(input_path), "-map_metadata", "-1", "-vn", "-ac", "2",
        "-ar", str(SAMPLE_RATE), "-f", "f32le", str(raw_path),
    ]
    result = subprocess.run(
        command,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace")[-4096:].strip()
        raise RuntimeError(f"FFmpeg could not decode the source audio: {detail}")
    samples = np.fromfile(raw_path, dtype="<f4")
    if samples.size == 0 or samples.size % 2:
        raise RuntimeError("FFmpeg returned invalid stereo PCM")
    return np.ascontiguousarray(samples.reshape(-1, 2).T, dtype=np.float32)


def write_float_wave(path: Path, samples: Any, sample_rate: int = SAMPLE_RATE) -> None:
    audio = np.asarray(samples, dtype="<f4").T
    if audio.ndim != 2 or audio.shape[1] != 2:
        raise RuntimeError("SCNet returned an invalid stereo stem")
    frames = int(audio.shape[0])
    data = audio.tobytes(order="C")
    with path.open("wb") as output:
        output.write(b"RIFF")
        output.write(struct.pack("<I", 48 + len(data)))
        output.write(b"WAVEfmt ")
        output.write(struct.pack("<IHHIIHH", 16, 3, 2, sample_rate, sample_rate * 8, 8, 32))
        output.write(b"fact")
        output.write(struct.pack("<II", 4, frames))
        output.write(b"data")
        output.write(struct.pack("<I", len(data)))
        output.write(data)


def separate(
    input_path: Path,
    output_dir: Path,
    model_dir: Path,
    ffmpeg: Path,
    backend: str,
    profile: str,
) -> None:
    if not input_path.is_file() or input_path.is_symlink():
        raise RuntimeError("the input audio is not a regular file")
    if not ffmpeg.is_file() or ffmpeg.is_symlink():
        raise RuntimeError("the bundled FFmpeg executable is unavailable")
    if not model_dir.is_dir() or model_dir.is_symlink():
        raise RuntimeError("the SCNet model cache is unavailable")
    validate_contract()
    output_dir.mkdir(parents=True, exist_ok=False)

    if profile == "fast":
        from .fast_demucs import validate_contract as validate_fast_contract

        validate_fast_contract()
    device = "mps"
    if backend == "torch":
        import torch

        device = choose_torch_device(torch)

    started = time.perf_counter()
    def download_progress(received: int, total: int) -> None:
        fraction = received / total if total > 0 else 0.0
        emit(
            "progress",
            stage="downloadingModel",
            progress=0.02 + 0.13 * min(max(fraction, 0.0), 1.0),
            completed=received,
            total=total,
        )

    if profile == "fast":
        from .fast_demucs import separate as separate_fast

        emit("stage", stage="loadingModel", progress=0.15)
        with tempfile.TemporaryDirectory(prefix="sonarcan-demucs-", dir=output_dir.parent) as temporary:
            raw_path = Path(temporary) / "input.f32le"
            emit("stage", stage="loadingAudio", progress=0.18)
            mix = decode_audio(ffmpeg, input_path, raw_path)

        def fast_progress(stage: str, fraction: float, completed: int | None, total: int | None) -> None:
            fields: dict[str, Any] = {"stage": stage, "progress": fraction}
            if completed is not None:
                fields["completed"] = completed
            if total is not None:
                fields["total"] = total
            emit("progress", **fields)

        result, cached, device = separate_fast(mix, model_dir, backend, fast_progress)
        emit(
            "log",
            level="info",
            message=f"HTDemucs 4 stems: backend={backend}, device={device}, checkpoint={'cached' if cached else 'downloaded'}, overlap=25%",
        )
        emit("stage", stage="writingStems", progress=0.90)
        for index, name in enumerate(STEM_NAMES):
            write_float_wave(output_dir / f"{name}.wav", result[name])
            emit("progress", stage="writingStems", progress=0.90 + 0.08 * (index + 1) / len(STEM_NAMES), completed=index + 1, total=len(STEM_NAMES))
        emit("complete", stage="complete", progress=1.0, stems=list(STEM_NAMES))
        return

    session = SCNetSession(
        MODEL_ID,
        backend=backend,
        device=device,
        cache_dir=model_dir,
        download_progress=download_progress,
    )
    cache = session.cache_info()
    if not cache["exists"]:
        emit("stage", stage="downloadingModel", progress=0.02)
    else:
        emit("stage", stage="loadingModel", progress=0.15)
    session.load()
    emit("stage", stage="loadingModel", progress=0.17)
    model_loaded = time.perf_counter()
    emit(
        "log",
        level="info",
        message=(
            f"{MODEL_NAME}: backend={backend}, device={session.device}, "
            f"checkpoint={'cached' if cache['exists'] else 'downloaded'}"
        ),
    )

    with tempfile.TemporaryDirectory(prefix="sonarcan-scnet-", dir=output_dir.parent) as temporary:
        raw_path = Path(temporary) / "input.f32le"
        emit("stage", stage="loadingAudio", progress=0.18)
        mix = decode_audio(ffmpeg, input_path, raw_path)
    audio_loaded = time.perf_counter()

    def inference_progress(completed: int, total: int) -> None:
        emit(
            "progress",
            stage="separating",
            progress=0.22 + 0.66 * completed / max(total, 1),
            completed=completed,
            total=total,
        )

    emit("stage", stage="separating", progress=0.22)
    result = session.infer(mix, sample_rate=SAMPLE_RATE, progress=inference_progress)
    inference_finished = time.perf_counter()
    emit("stage", stage="writingStems", progress=0.90)
    for index, name in enumerate(STEM_NAMES):
        write_float_wave(output_dir / f"{name}.wav", result.stems[name])
        emit(
            "progress",
            stage="writingStems",
            progress=0.90 + 0.08 * (index + 1) / len(STEM_NAMES),
            completed=index + 1,
            total=len(STEM_NAMES),
        )
    session.close()
    finished = time.perf_counter()
    emit(
        "log",
        level="info",
        message=(
            f"{MODEL_NAME} timings: model={model_loaded - started:.2f}s, "
            f"decode={audio_loaded - model_loaded:.2f}s, "
            f"inference={inference_finished - audio_loaded:.2f}s, "
            f"write={finished - inference_finished:.2f}s"
        ),
    )
    emit("complete", stage="complete", progress=1.0, stems=list(STEM_NAMES))


def accelerator_self_test(backend: str) -> None:
    validate_contract()
    if backend == "mlx":
        import mlx.core as mx

        value = mx.matmul(mx.ones((4, 4)), mx.ones((4, 4)))
        mx.eval(value)
        if not np.isfinite(np.asarray(value)).all():
            raise RuntimeError("MLX accelerator self-test produced invalid values")
        label = "MLX"
    else:
        import torch

        device = choose_torch_device(torch)
        if device != "cuda":
            raise RuntimeError("the qualified CUDA or ROCm accelerator is unavailable")
        value = torch.ones((4, 4), device=device) @ torch.ones((4, 4), device=device)
        if not torch.isfinite(value).all().item():
            raise RuntimeError("Torch accelerator self-test produced invalid values")
        torch.cuda.synchronize()
        label = "ROCm" if torch.version.hip else "CUDA"
    emit(
        "ready",
        model=MODEL_ID,
        backend=label,
        accelerated=True,
        stems=list(STEM_NAMES),
    )


def build_parser(program: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog=program)
    subcommands = parser.add_subparsers(dest="command")
    subcommands.add_parser("self-test")
    subcommands.add_parser("accelerator-self-test")
    command = subcommands.add_parser("separate")
    command.add_argument("--input", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--model-dir", type=Path, required=True)
    command.add_argument("--ffmpeg", type=Path, required=True)
    command.add_argument("--profile", choices=("fast", "hq"), required=True)
    return parser


def run(arguments: Sequence[str] | None, *, backend: str, program: str) -> int:
    args = build_parser(program).parse_args(arguments)
    try:
        if args.command == "self-test":
            validate_contract()
            from .fast_demucs import validate_contract as validate_fast_contract

            validate_fast_contract()
            emit("ready", model=MODEL_ID, backend=backend, stems=list(STEM_NAMES))
            return 0
        if args.command == "accelerator-self-test":
            accelerator_self_test(backend)
            return 0
        if args.command == "separate":
            separate(args.input, args.output, args.model_dir, args.ffmpeg, backend, args.profile)
            return 0
        raise RuntimeError("a worker command is required")
    except Exception as error:
        emit("error", stage="failed", message=str(error)[:8192])
        return 1
