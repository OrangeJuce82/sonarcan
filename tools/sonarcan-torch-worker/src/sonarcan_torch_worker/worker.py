from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import random
import re
import struct
import subprocess
import sys
import tempfile
import time
from fractions import Fraction
from pathlib import Path
from typing import Any, Sequence

MODEL_NAME = "htdemucs_6s"
STEM_NAMES = ("vocals", "drums", "bass", "other", "guitar", "piano")
MODEL_SOURCE = [{"signature": "5c90dfd2", "checksum": "34c22ccb"}]
MAX_MODEL_BYTES = 256 * 1024 * 1024
MAX_CONFIG_BYTES = 256 * 1024
CPU_SHIFTS = 0
ACCELERATOR_SHIFTS = 1
INFERENCE_OVERLAP = 0.10


def emit(event_type: str, **fields: Any) -> None:
    payload = {"type": event_type, **fields}
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=True) + "\n")
    sys.stdout.flush()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_model_files(model_dir: Path) -> tuple[Path, dict[str, Any]]:
    weights = model_dir / f"{MODEL_NAME}.safetensors"
    config_path = model_dir / f"{MODEL_NAME}_config.json"
    for path, limit in ((weights, MAX_MODEL_BYTES), (config_path, MAX_CONFIG_BYTES)):
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f"the bundled model file is invalid: {path.name}")
        if path.stat().st_size > limit:
            raise RuntimeError(f"the bundled model file is too large: {path.name}")
    config = json.loads(config_path.read_text(encoding="utf-8"))
    if config.get("model_name") != MODEL_NAME or config.get("source_artifacts") != MODEL_SOURCE:
        raise RuntimeError("the bundled model is not the pinned htdemucs_6s artifact")
    expected_hash = config.get("safetensors_sha256")
    if not isinstance(expected_hash, str) or _sha256(weights) != expected_hash:
        raise RuntimeError("the bundled model checksum is invalid")
    sources = config.get("per_model_kwargs", [{}])[0].get("sources")
    if not isinstance(sources, list) or set(sources) != set(STEM_NAMES):
        raise RuntimeError("the bundled model does not expose the expected six stems")
    return weights, config


def mlx_parameter_name(torch_name: str, module: Any) -> str:
    """Map an upstream Torch parameter to the converted MLX state name."""
    name = torch_name.replace(".self_attn.", ".attn.")
    name = name.replace(".norm_out.", ".norm_out.gn.")
    name = re.sub(r"(\.dconv\.layers\.\d+)\.(\d+)\.", r"\1.layers.\2.", name)
    if module.__class__.__name__ in {
        "Conv1d",
        "ConvTranspose1d",
        "Conv2d",
        "ConvTranspose2d",
    }:
        prefix, parameter = name.rsplit(".", 1)
        name = f"{prefix}.conv.{parameter}"
    return f"model_0.{name}"


def _attention_projection_names(torch_name: str) -> list[str] | None:
    if ".in_proj_" not in torch_name:
        return None
    prefix, parameter = torch_name.rsplit(".in_proj_", 1)
    prefix = prefix.replace(".self_attn", ".attn")
    return [f"model_0.{prefix}.{part}_proj.{parameter}" for part in ("query", "key", "value")]


def _torch_value(torch: Any, module: Any, parameter: str, value: Any) -> Any:
    if parameter != "weight":
        return value
    if isinstance(module, torch.nn.Conv1d):
        return value.permute(0, 2, 1)
    if isinstance(module, torch.nn.ConvTranspose1d):
        return value.permute(2, 0, 1)
    if isinstance(module, torch.nn.Conv2d):
        return value.permute(0, 3, 1, 2)
    if isinstance(module, torch.nn.ConvTranspose2d):
        return value.permute(3, 0, 1, 2)
    return value


def load_model(model_dir: Path, device: str) -> Any:
    import torch
    from demucs.htdemucs import HTDemucs
    from safetensors.torch import load_file

    weights_path, config = validate_model_files(model_dir)
    kwargs = dict(config["per_model_kwargs"][0])
    segment = kwargs.get("segment")
    if isinstance(segment, dict) and segment.get("__type__") == "fraction":
        kwargs["segment"] = Fraction(segment["numerator"], segment["denominator"])
    model = HTDemucs(**kwargs)
    mlx_state = load_file(str(weights_path), device="cpu")
    modules = dict(model.named_modules())
    torch_state: dict[str, Any] = {}
    for name, expected in model.state_dict().items():
        projection_names = _attention_projection_names(name)
        if projection_names is not None:
            try:
                value = torch.cat([mlx_state[key] for key in projection_names], dim=0)
            except KeyError as error:
                raise RuntimeError(f"the bundled model is missing {error.args[0]}") from error
        else:
            module_name, parameter = name.rsplit(".", 1)
            module = modules[module_name]
            key = mlx_parameter_name(name, module)
            try:
                value = _torch_value(torch, module, parameter, mlx_state[key])
            except KeyError as error:
                raise RuntimeError(f"the bundled model is missing {key}") from error
        if value.shape != expected.shape:
            raise RuntimeError(f"the bundled model tensor has an invalid shape: {name}")
        torch_state[name] = value
    model.load_state_dict(torch_state, strict=True)
    model.eval()
    model.to(device)
    return model


def choose_device(torch: Any) -> str:
    requested = os.environ.get("SONARCAN_TORCH_DEVICE", "auto").lower()
    available = {"cpu"}
    if torch.cuda.is_available():
        available.add("cuda")
    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        available.add("mps")
    if requested == "auto":
        return "cuda" if "cuda" in available else "mps" if "mps" in available else "cpu"
    if requested not in available:
        raise RuntimeError(f"the requested Torch device is unavailable: {requested}")
    return requested


def inference_settings(device: str) -> tuple[int, float]:
    # Upstream Demucs explicitly discourages the shift trick on CPU: one shift
    # adds work for at most 0.2 SDR. A 10% overlap is its documented fast path.
    shifts = CPU_SHIFTS if device == "cpu" else ACCELERATOR_SHIFTS
    return shifts, INFERENCE_OVERLAP


def decode_audio(ffmpeg: Path, input_path: Path, raw_path: Path, sample_rate: int) -> Any:
    import numpy as np
    import torch

    command = [
        str(ffmpeg), "-hide_banner", "-loglevel", "error", "-nostdin", "-y",
        "-i", str(input_path), "-map_metadata", "-1", "-vn", "-ac", "2",
        "-ar", str(sample_rate), "-f", "f32le", str(raw_path),
    ]
    result = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, check=False)
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace")[-4096:].strip()
        raise RuntimeError(f"FFmpeg could not decode the source audio: {detail}")
    samples = np.fromfile(raw_path, dtype="<f4")
    if samples.size == 0 or samples.size % 2:
        raise RuntimeError("FFmpeg returned invalid stereo PCM")
    return torch.from_numpy(samples.reshape(-1, 2).T.copy()).unsqueeze(0)


def write_float_wave(path: Path, samples: Any, sample_rate: int) -> None:
    import numpy as np

    audio = np.asarray(samples.detach().cpu(), dtype="<f4").T
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


def separate(input_path: Path, output_dir: Path, model_dir: Path, ffmpeg: Path) -> None:
    import torch
    from demucs.apply import apply_model

    if not input_path.is_file() or input_path.is_symlink():
        raise RuntimeError("the input audio is not a regular file")
    if not ffmpeg.is_file():
        raise RuntimeError("the bundled FFmpeg executable is unavailable")
    output_dir.mkdir(parents=True, exist_ok=False)
    device = choose_device(torch)
    shifts, overlap = inference_settings(device)
    started = time.perf_counter()
    emit("stage", stage="loadingModel", progress=0.03)
    model = load_model(model_dir, device)
    model_loaded = time.perf_counter()
    emit(
        "log",
        level="info",
        message=(
            f"HTDemucs portable backend: Torch {torch.__version__} on {device}; "
            f"shifts={shifts}, overlap={overlap:.2f}, threads={torch.get_num_threads()}"
        ),
    )
    with tempfile.TemporaryDirectory(prefix="sonarcan-torch-", dir=output_dir.parent) as temporary:
        raw_path = Path(temporary) / "input.f32le"
        emit("stage", stage="loadingAudio", progress=0.10)
        mix = decode_audio(ffmpeg, input_path, raw_path, int(model.samplerate))
    audio_loaded = time.perf_counter()
    segment_length = int(float(model.segment) * model.samplerate)
    stride = int((1 - overlap) * segment_length)
    inference_frames = int(mix.shape[-1]) + (int(0.5 * model.samplerate) if shifts else 0)
    segment_count = max(1, math.ceil(inference_frames / stride))
    completed_segments: set[tuple[int, int]] = set()

    def progress(event: dict[str, Any]) -> None:
        if event.get("state") != "end":
            return
        key = (int(event.get("shift_idx", 0)), int(event.get("segment_offset", 0)))
        completed_segments.add(key)
        completed = min(segment_count, len(completed_segments))
        emit(
            "progress",
            stage="separating",
            progress=0.15 + 0.72 * completed / segment_count,
            completed=completed,
            total=segment_count,
        )

    emit("stage", stage="separating", progress=0.15)
    random.seed(0)
    with torch.inference_mode():
        estimates = apply_model(
            model,
            mix,
            shifts=shifts,
            split=True,
            overlap=overlap,
            progress=False,
            device=device,
            num_workers=0,
            callback=progress,
        )[0]
    if device == "cuda":
        torch.cuda.synchronize()
    elif device == "mps":
        torch.mps.synchronize()
    inference_finished = time.perf_counter()
    by_name = {name: estimates[index] for index, name in enumerate(model.sources)}
    emit("stage", stage="writingStems", progress=0.90)
    for index, name in enumerate(STEM_NAMES):
        write_float_wave(output_dir / f"{name}.wav", by_name[name], int(model.samplerate))
        emit(
            "progress",
            stage="writingStems",
            progress=0.90 + 0.08 * (index + 1) / len(STEM_NAMES),
            completed=index + 1,
            total=len(STEM_NAMES),
        )
    finished = time.perf_counter()
    emit(
        "log",
        level="info",
        message=(
            f"HTDemucs timings: model={model_loaded - started:.2f}s, "
            f"decode={audio_loaded - model_loaded:.2f}s, "
            f"inference={inference_finished - audio_loaded:.2f}s, "
            f"write={finished - inference_finished:.2f}s"
        ),
    )
    emit("complete", stage="complete", progress=1.0, stems=list(STEM_NAMES))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="sonarcan-torch-worker")
    subcommands = parser.add_subparsers(dest="command")
    health = subcommands.add_parser("self-test")
    health.add_argument("--model-dir", type=Path, required=True)
    command = subcommands.add_parser("separate")
    command.add_argument("--input", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--model-dir", type=Path, required=True)
    command.add_argument("--ffmpeg", type=Path, required=True)
    return parser


def run(arguments: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(arguments)
    try:
        if args.command == "self-test":
            import torch

            device = choose_device(torch)
            model = load_model(args.model_dir, device)
            if tuple(model.sources) != ("drums", "bass", "other", "vocals", "guitar", "piano"):
                raise RuntimeError("the reconstructed model has an invalid stem order")
            emit("ready", model=MODEL_NAME, backend="Torch", device=device, stems=list(STEM_NAMES))
            return 0
        if args.command == "separate":
            separate(args.input, args.output, args.model_dir, args.ffmpeg)
            return 0
        raise RuntimeError("a worker command is required")
    except Exception as error:
        emit("error", stage="failed", message=str(error)[:8192])
        return 1


def main() -> int:
    return run()


if __name__ == "__main__":
    raise SystemExit(main())
