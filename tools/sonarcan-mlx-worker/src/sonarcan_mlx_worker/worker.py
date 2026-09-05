from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import logging
import random
import sys
import time
from pathlib import Path
from typing import Any, Sequence

MODEL_NAME = "htdemucs_6s"
STEM_NAMES = ("vocals", "drums", "bass", "other", "guitar", "piano")
EXPECTED_VERSIONS = {
    "demucs-mlx": "1.4.6",
    "mlx": "0.31.2",
    "mlx-audio-io": "1.3.11",
    "mlx-spectro": "0.2.4",
}
MODEL_CONFIG_FIELDS = {
    "format_version", "model_name", "model_class", "sub_model_class", "args", "kwargs",
    "per_model_args", "per_model_kwargs", "per_model_classes", "num_models", "weights",
    "source_artifacts", "mlx_version", "conversion_date", "verification_passed",
    "safetensors_sha256",
}
INFERENCE_OVERLAP = 0.10
GIBIBYTE = 1024**3


def emit(event_type: str, **fields: Any) -> None:
    payload = {"type": event_type, **fields}
    sys.stdout.write(json.dumps(payload, separators=(",", ":"), ensure_ascii=True) + "\n")
    sys.stdout.flush()


class ProtocolLogHandler(logging.Handler):
    def emit(self, record: logging.LogRecord) -> None:
        level = record.levelname.lower()
        if level not in {"debug", "info", "warning", "error", "critical"}:
            level = "info"
        if level == "warning":
            level = "warn"
        if level == "critical":
            level = "error"
        emit("log", level=level, message=self.format(record)[:8_192])


class JsonProgress:
    """Small tqdm-compatible adapter emitting bounded SonArcan events."""

    def __init__(self, total: int | None = None, **_: Any) -> None:
        self.total = max(1, int(total or 1))
        self.completed = 0
        emit("stage", stage="separating", progress=0.15, completed=0, total=self.total)

    def update(self, amount: int = 1) -> None:
        self.completed = min(self.total, self.completed + int(amount))
        progress = 0.15 + 0.75 * self.completed / self.total
        emit(
            "progress",
            stage="separating",
            progress=progress,
            completed=self.completed,
            total=self.total,
        )

    def close(self) -> None:
        return None


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_model_files(model_dir: Path) -> tuple[Path, Path]:
    weights = model_dir / f"{MODEL_NAME}.safetensors"
    config = model_dir / f"{MODEL_NAME}_config.json"
    if not weights.is_file() or not config.is_file():
        raise RuntimeError("the bundled htdemucs_6s model is incomplete")
    if weights.is_symlink() or config.is_symlink():
        raise RuntimeError("model files must not be symbolic links")
    if weights.stat().st_size > 256 * 1024 * 1024 or config.stat().st_size > 256 * 1024:
        raise RuntimeError("the bundled model exceeds its safety limit")
    metadata = json.loads(config.read_text(encoding="utf-8"))
    if set(metadata) != MODEL_CONFIG_FIELDS:
        raise RuntimeError("the bundled model metadata is not the pinned demucs-mlx format")
    if metadata.get("format_version") != 1 or metadata.get("model_name") != MODEL_NAME:
        raise RuntimeError("the bundled model metadata is unsupported")
    sources = metadata.get("kwargs", {}).get("sources")
    if not isinstance(sources, list) or set(sources) != set(STEM_NAMES):
        raise RuntimeError("the bundled model does not expose the expected six stems")
    if metadata.get("per_model_classes") != ["HTDemucsMLX"]:
        raise RuntimeError("the bundled model class is unsupported")
    if metadata.get("source_artifacts") != [{"signature": "5c90dfd2", "checksum": "34c22ccb"}]:
        raise RuntimeError("the bundled model is not derived from the pinned official artifact")
    expected_hash = metadata.get("safetensors_sha256")
    if not isinstance(expected_hash, str) or _sha256(weights) != expected_hash:
        raise RuntimeError("the bundled model checksum is invalid")
    return weights, config


def validate_runtime_versions() -> None:
    for package, expected in EXPECTED_VERSIONS.items():
        actual = importlib.metadata.version(package)
        if actual != expected:
            raise RuntimeError(f"runtime mismatch for {package}: expected {expected}, found {actual}")


def _load_audio(path: Path, model: Any) -> Any:
    import mlx.core as mx
    import mlx_audio_io as mac

    audio, _ = mac.load(str(path), sr=int(model.samplerate), dtype="float32")
    wav = mx.transpose(audio, (1, 0))
    source_channels = int(wav.shape[0])
    target_channels = int(model.audio_channels)
    if source_channels == target_channels:
        return wav
    if target_channels == 1:
        return mx.mean(wav, axis=0, keepdims=True)
    if source_channels == 1 and target_channels == 2:
        return mx.broadcast_to(wav, (2, int(wav.shape[1])))
    if source_channels > target_channels:
        return wav[:target_channels, :]
    raise RuntimeError("the input channel layout is unsupported")


def _apply_with_one_shift(model: Any, mix: Any, seed: int, batch_size: int) -> Any:
    import mlx.core as mx
    import tqdm
    from demucs_mlx.apply_mlx import TensorChunk, apply_model

    original_tqdm = tqdm.tqdm
    tqdm.tqdm = JsonProgress
    try:
        length = int(mix.shape[-1])
        max_shift = int(0.5 * model.samplerate)
        padded = TensorChunk(mix).padded(length + 2 * max_shift)
        offset = random.Random(seed).randint(0, max_shift)
        shifted = TensorChunk(padded, offset, length + max_shift - offset)
        result = apply_model(
            model,
            shifted,
            shifts=0,
            split=True,
            overlap=INFERENCE_OVERLAP,
            progress=True,
            batch_size=batch_size,
            seed=seed,
        )
        result = result[..., max_shift - offset :]
        mx.eval(result)
        return result
    finally:
        tqdm.tqdm = original_tqdm


def separate(input_path: Path, output_dir: Path, model_dir: Path, batch_size: int) -> None:
    if not input_path.is_file() or input_path.is_symlink():
        raise RuntimeError("the input audio is not a regular file")
    validate_runtime_versions()
    validate_model_files(model_dir)
    output_dir.mkdir(parents=True, exist_ok=False)

    logging.getLogger().handlers = [ProtocolLogHandler()]
    logging.getLogger().setLevel(logging.INFO)
    started = time.perf_counter()
    emit("stage", stage="loadingModel", progress=0.03)

    from demucs_mlx.mlx_convert import load_mlx_model

    model = load_mlx_model(
        MODEL_NAME,
        cache_dir=str(model_dir),
        auto_convert=False,
        verbose=False,
    )
    if tuple(model.sources) != ("drums", "bass", "other", "vocals", "guitar", "piano"):
        raise RuntimeError("demucs-mlx returned an unexpected stem order")
    model_loaded = time.perf_counter()

    emit("stage", stage="loadingAudio", progress=0.10)
    wav = _load_audio(input_path, model)
    audio_loaded = time.perf_counter()
    estimates = _apply_with_one_shift(model, wav[None, ...], seed=0, batch_size=batch_size)
    inference_finished = time.perf_counter()

    import mlx.core as mx
    import numpy as np
    from demucs_mlx.audio import save_audio

    emit("stage", stage="writingStems", progress=0.90)
    host_stems = np.asarray(estimates[0])
    by_name = {name: host_stems[index] for index, name in enumerate(model.sources)}
    for index, name in enumerate(STEM_NAMES):
        save_audio(
            by_name[name],
            output_dir / f"{name}.wav",
            samplerate=int(model.samplerate),
            clip="none",
            bits_per_sample=32,
            as_float=True,
        )
        emit(
            "progress",
            stage="writingStems",
            progress=0.90 + 0.08 * (index + 1) / len(STEM_NAMES),
            completed=index + 1,
            total=len(STEM_NAMES),
        )
    finished = time.perf_counter()
    peak_gib = mx.get_peak_memory() / GIBIBYTE
    emit(
        "log",
        level="info",
        message=(
            f"HTDemucs MLX settings: batch={batch_size}, overlap={INFERENCE_OVERLAP:.2f}; "
            f"model={model_loaded - started:.2f}s, decode={audio_loaded - model_loaded:.2f}s, "
            f"inference={inference_finished - audio_loaded:.2f}s, "
            f"write={finished - inference_finished:.2f}s, peak={peak_gib:.2f}GiB"
        ),
    )
    emit("complete", stage="complete", progress=1.0, stems=list(STEM_NAMES))


def accelerator_self_test(model_dir: Path) -> None:
    """Load the production graph and execute a finite MLX operation."""
    validate_runtime_versions()
    validate_model_files(model_dir)

    import mlx.core as mx
    import numpy as np
    from demucs_mlx.mlx_convert import load_mlx_model

    model = load_mlx_model(
        MODEL_NAME,
        cache_dir=str(model_dir),
        auto_convert=False,
        verbose=False,
    )
    if tuple(model.sources) != ("drums", "bass", "other", "vocals", "guitar", "piano"):
        raise RuntimeError("demucs-mlx returned an unexpected stem order")
    value = mx.matmul(mx.ones((4, 4)), mx.ones((4, 4)))
    mx.eval(value)
    if not np.isfinite(np.asarray(value)).all():
        raise RuntimeError("MLX accelerator self-test produced invalid values")
    emit("ready", model=MODEL_NAME, stems=list(STEM_NAMES), backend="MLX")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="sonarcan-mlx-worker")
    parser.add_argument("--version", action="store_true")
    subcommands = parser.add_subparsers(dest="command")
    health = subcommands.add_parser("self-test")
    health.add_argument("--model-dir", type=Path, required=True)
    accelerator = subcommands.add_parser("accelerator-self-test")
    accelerator.add_argument("--model-dir", type=Path, required=True)
    command = subcommands.add_parser("separate")
    command.add_argument("--input", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--model-dir", type=Path, required=True)
    command.add_argument(
        "--batch-size",
        type=int,
        default=2,
        choices=range(1, 9),
    )
    return parser


def run(arguments: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(arguments)
    if args.version:
        emit("version", worker="0.1.0", python=sys.version.split()[0])
        return 0
    try:
        if args.command == "self-test":
            validate_runtime_versions()
            validate_model_files(args.model_dir)
            emit("ready", model=MODEL_NAME, stems=list(STEM_NAMES))
            return 0
        if args.command == "accelerator-self-test":
            accelerator_self_test(args.model_dir)
            return 0
        if args.command == "separate":
            separate(args.input, args.output, args.model_dir, args.batch_size)
            return 0
        raise RuntimeError("a worker command is required")
    except Exception as error:
        emit("error", stage="failed", message=str(error)[:8_192])
        return 1


def main() -> int:
    return run()
