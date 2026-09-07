"""Verified HTDemucs four-stem profile for the SonArcan worker protocol."""

from __future__ import annotations

import contextlib
import math
import os
import shutil
from dataclasses import replace
from pathlib import Path
from typing import Any, Callable, Iterator

from .checkpoints import get_spec, resolve_checkpoint

MODEL_ID = "htdemucs-v4"
MODEL_NAME = "HTDemucs 4 stems"
MODEL_FILENAME = "955717e8-8726e21a.th"
MODEL_URL = "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/955717e8-8726e21a.th"
MODEL_SHA256 = "8726e21a993978c7ba086d3872e7608d7d5bfca646ca4aca459ffda844faa8b4"
MODEL_BYTES = 84_141_911
SAMPLE_RATE = 44_100
STEM_NAMES = ("vocals", "drums", "bass", "other")
OVERLAP = 0.25
SHIFTS = 0
MLX_BATCH_SIZE = 2

Progress = Callable[[str, float, int | None, int | None], None]


def _spec() -> Any:
    return replace(
        get_spec(),
        model_id=MODEL_ID,
        family="htdemucs",
        url=MODEL_URL,
        filename=MODEL_FILENAME,
        size=MODEL_BYTES,
        sha256=MODEL_SHA256,
        license="NOASSERTION",
        provenance="facebookresearch/demucs",
        source_revision="955717e8",
        sample_rate=SAMPLE_RATE,
        sources=STEM_NAMES,
    )


def validate_contract() -> None:
    spec = _spec()
    if (
        spec.url != MODEL_URL
        or spec.filename != MODEL_FILENAME
        or spec.size != MODEL_BYTES
        or spec.sha256 != MODEL_SHA256
        or spec.sample_rate != SAMPLE_RATE
        or set(spec.sources) != set(STEM_NAMES)
    ):
        raise RuntimeError("the pinned HTDemucs checkpoint contract is invalid")


def resolve_model(model_dir: Path, progress: Progress) -> tuple[Path, bool]:
    spec = _spec()
    target = model_dir / MODEL_ID / MODEL_FILENAME
    cached = target.is_file()

    def download(done: int, total: int) -> None:
        fraction = done / total if total > 0 else 0.0
        progress("downloadingModel", 0.02 + 0.13 * min(max(fraction, 0.0), 1.0), done, total)

    checkpoint = resolve_checkpoint(spec, cache_dir=model_dir, progress=download)
    return checkpoint, cached


def _normalise(mix: np.ndarray) -> tuple[np.ndarray, float, float]:
    import numpy as np

    reference = mix.mean(axis=0)
    mean = float(reference.mean())
    std = float(reference.std()) + 1e-8
    return np.ascontiguousarray((mix - mean) / std, dtype=np.float32), mean, std


def _validated_sources(model: Any) -> None:
    sources = tuple(str(source) for source in model.sources)
    if len(sources) != len(STEM_NAMES) or set(sources) != set(STEM_NAMES):
        raise RuntimeError(f"HTDemucs returned unexpected sources: {sources}")
    if int(model.samplerate) != SAMPLE_RATE:
        raise RuntimeError(f"HTDemucs returned an unexpected sample rate: {model.samplerate}")


def _load_safe_torch_model(checkpoint: Path, torch: Any) -> Any:
    import numpy as np
    from demucs.demucs import Demucs
    from demucs.hdemucs import HDemucs
    from demucs.htdemucs import HTDemucs
    from demucs.states import load_model
    from fractions import Fraction

    allowed: list[Any] = [Demucs, HDemucs, HTDemucs, Fraction, np.dtype]
    try:
        import numpy._core.multiarray as np_multiarray
    except ImportError:
        import numpy.core.multiarray as np_multiarray
    allowed.extend([
        (np_multiarray.scalar, "numpy.core.multiarray.scalar"),
        (np_multiarray.scalar, "numpy._core.multiarray.scalar"),
    ])
    allowed.extend({type(np.dtype(kind)) for kind in (
        np.bool_, np.int8, np.int16, np.int32, np.int64, np.uint8, np.uint16,
        np.uint32, np.uint64, np.float16, np.float32, np.float64, np.complex64,
        np.complex128,
    )})
    with torch.serialization.safe_globals(allowed):
        package = torch.load(checkpoint, map_location="cpu", weights_only=True)
    if not isinstance(package, dict) or not {"klass", "args", "kwargs", "state"}.issubset(package):
        raise RuntimeError("the HTDemucs checkpoint has an invalid package structure")
    if package["klass"] is not HTDemucs:
        raise RuntimeError("the HTDemucs checkpoint contains an unexpected model class")
    if not isinstance(package["args"], (list, tuple)) or not isinstance(package["kwargs"], dict):
        raise RuntimeError("the HTDemucs checkpoint contains invalid constructor metadata")
    state = package["state"]
    if not isinstance(state, dict) or not state or len(state) > 100_000:
        raise RuntimeError("the HTDemucs checkpoint contains an invalid model state")
    if not all(isinstance(key, str) and isinstance(value, torch.Tensor) for key, value in state.items()):
        raise RuntimeError("the HTDemucs checkpoint state must contain only named tensors")
    trusted = {key: package[key] for key in ("klass", "args", "kwargs", "state")}
    model = load_model(trusted, strict=True)
    model.eval()
    _validated_sources(model)
    return model


@contextlib.contextmanager
def _torch_home(checkpoint: Path) -> Iterator[None]:
    root = checkpoint.parent / "torch-home"
    torch_checkpoint = root / "hub" / "checkpoints" / MODEL_FILENAME
    torch_checkpoint.parent.mkdir(parents=True, exist_ok=True)
    if not torch_checkpoint.is_file():
        try:
            os.link(checkpoint, torch_checkpoint)
        except OSError:
            shutil.copy2(checkpoint, torch_checkpoint)
    previous = os.environ.get("TORCH_HOME")
    os.environ["TORCH_HOME"] = str(root)
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop("TORCH_HOME", None)
        else:
            os.environ["TORCH_HOME"] = previous


def _load_mlx_model(checkpoint: Path) -> Any:
    import demucs_mlx.mlx_convert as converter
    import demucs_mlx.secure_demucs as secure

    cache = checkpoint.parent / "mlx"
    weights = cache / "htdemucs.safetensors"
    config = cache / "htdemucs_config.json"
    if weights.is_symlink() or config.is_symlink() or cache.is_symlink():
        raise RuntimeError("HTDemucs MLX cache paths must not be symbolic links")
    if not (weights.is_file() and config.is_file()):
        cache.mkdir(parents=True, exist_ok=True)
        original_validate = secure._validate_package

        def compatible_validate(package: Any, torch: Any) -> Any:
            if isinstance(package, dict):
                package = dict(package)
                package.pop("training_args", None)
                package.pop("metrics", None)
            return original_validate(package, torch)

        secure._validate_package = compatible_validate
        try:
            with _torch_home(checkpoint):
                converter.convert_htdemucs_weights("htdemucs", output_dir=str(cache), verify=False, verbose=False)
        finally:
            secure._validate_package = original_validate
    model = converter.load_mlx_model("htdemucs", cache_dir=str(cache), auto_convert=False, verbose=False)
    _validated_sources(model)
    return model


@contextlib.contextmanager
def _mlx_progress(progress: Progress) -> Iterator[None]:
    import tqdm

    original = tqdm.tqdm

    class Reporter:
        def __init__(self, total: int, **_: Any) -> None:
            self.total = max(int(total), 1)
            self.completed = 0

        def update(self, count: int = 1) -> None:
            self.completed = min(self.total, self.completed + int(count))
            progress("separating", 0.22 + 0.66 * self.completed / self.total, self.completed, self.total)

        def close(self) -> None:
            return None

    tqdm.tqdm = Reporter
    try:
        yield
    finally:
        tqdm.tqdm = original


def separate_mlx(mix: np.ndarray, checkpoint: Path, progress: Progress) -> dict[str, np.ndarray]:
    import mlx.core as mx
    import numpy as np
    from demucs_mlx.apply_mlx import apply_model

    model = _load_mlx_model(checkpoint)
    normalised, mean, std = _normalise(mix)
    with _mlx_progress(progress):
        output = apply_model(
            model,
            mx.array(normalised[None]),
            shifts=SHIFTS,
            split=True,
            overlap=OVERLAP,
            progress=True,
            batch_size=MLX_BATCH_SIZE,
            seed=0,
        )
    output = np.asarray(output[0], dtype=np.float32) * std + mean
    return {name: output[index] for index, name in enumerate(model.sources)}


def separate_torch(
    mix: np.ndarray,
    checkpoint: Path,
    device: str,
    progress: Progress,
) -> dict[str, np.ndarray]:
    import torch
    from demucs.apply import apply_model

    model = _load_safe_torch_model(checkpoint, torch).to(device)
    normalised, mean, std = _normalise(mix)
    tensor = torch.from_numpy(normalised).unsqueeze(0)
    segment = float(model.segment)
    segment_samples = max(1, int(segment * SAMPLE_RATE * (1.0 - OVERLAP)))
    total = max(1, math.ceil(tensor.shape[-1] / segment_samples))
    completed = 0

    def callback(event: dict[str, Any]) -> None:
        nonlocal completed
        if event.get("state") == "end":
            completed = min(total, completed + 1)
            progress("separating", 0.22 + 0.66 * completed / total, completed, total)

    with torch.inference_mode():
        output = apply_model(
            model,
            tensor,
            shifts=SHIFTS,
            split=True,
            overlap=OVERLAP,
            device=device,
            callback=callback,
        )[0]
    output = output.cpu().float().numpy() * std + mean
    return {name: output[index] for index, name in enumerate(model.sources)}


def separate(
    mix: np.ndarray,
    model_dir: Path,
    backend: str,
    progress: Progress,
) -> tuple[dict[str, np.ndarray], bool, str]:
    validate_contract()
    checkpoint, cached = resolve_model(model_dir, progress)
    progress("loadingModel", 0.20, None, None)
    if backend == "mlx":
        stems = separate_mlx(mix, checkpoint, progress)
        device = "mps"
    else:
        import torch
        from .sonarcan_worker import choose_torch_device

        device = choose_torch_device(torch)
        stems = separate_torch(mix, checkpoint, device, progress)
    if set(stems) != set(STEM_NAMES):
        raise RuntimeError("HTDemucs did not return the required four stems")
    return stems, cached, device
