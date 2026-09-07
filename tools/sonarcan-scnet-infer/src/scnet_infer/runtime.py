"""Faithful model construction and overlap-add inference runtime.

The overlap, padding, float32 accumulation, and autocast choices mirror MSST
revision 83d495d while hiding training configuration entirely.
Reads: checkpoint spec, private SCNet models, and torch.
"""

from __future__ import annotations

from contextlib import nullcontext
from pathlib import Path

import numpy as np
import torch
from torch import nn

from .checkpoints import CheckpointSpec


def build_model(spec: CheckpointSpec) -> nn.Module:
    """Construct the architecture named by a validated family."""

    params = dict(spec.model)
    params["sources"] = list(spec.sources)
    if spec.family == "scnet":
        from ._model.scnet import SCNet
        return SCNet(**params)
    if spec.family == "scnet_masked":
        from ._model.scnet_masked import SCNet
        return SCNet(**params)
    if spec.family == "scnet_tran":
        from ._model.scnet_tran import SCNet_Tran
        return SCNet_Tran(**params)
    raise ValueError(f"unsupported SCNet family: {spec.family}")


def load_runtime_model(spec: CheckpointSpec, checkpoint: Path, device: torch.device) -> nn.Module:
    """Load one checkpoint and make the model inference-ready."""

    model = build_model(spec)
    state = torch.load(checkpoint, map_location="cpu", weights_only=True)
    if isinstance(state, dict) and "state_dict" in state:
        state = state["state_dict"]
    model.load_state_dict(state)
    return model.eval().to(device)


def _window(size: int, fade: int) -> torch.Tensor:
    value = torch.ones(size)
    value[:fade] = torch.linspace(0, 1, fade)
    value[-fade:] = torch.linspace(1, 0, fade)
    return value


def demix(
    spec: CheckpointSpec,
    model: nn.Module,
    mixture: np.ndarray,
    device: torch.device,
    progress=None,
) -> dict[str, np.ndarray]:
    """Run MSST-compatible chunked inference and return channel-first stems."""

    mix = torch.tensor(mixture, dtype=torch.float32)
    chunk_size = spec.chunk_size
    step = chunk_size // spec.num_overlap
    border = chunk_size - step
    length = mix.shape[-1]
    fade = chunk_size // 10
    windowing = _window(chunk_size, fade)
    if length > 2 * border and border > 0:
        mix = nn.functional.pad(mix, (border, border), mode="reflect")
    shape = (len(spec.sources),) + tuple(mix.shape)
    result = torch.zeros(shape, dtype=torch.float32)
    counter = torch.zeros(shape, dtype=torch.float32)
    batches: list[torch.Tensor] = []
    locations: list[tuple[int, int]] = []
    position = 0
    total_chunks = max(1, (mix.shape[1] + step - 1) // step)
    completed_chunks = 0
    amp = torch.amp.autocast(device_type="cuda", enabled=spec.use_amp) if device.type == "cuda" else nullcontext()
    with amp, torch.inference_mode():
        while position < mix.shape[1]:
            part = mix[:, position:position + chunk_size].to(device)
            segment_length = part.shape[-1]
            mode = "reflect" if segment_length > chunk_size // 2 else "constant"
            part = nn.functional.pad(part, (0, chunk_size - segment_length), mode=mode, value=0)
            batches.append(part)
            locations.append((position, segment_length))
            position += step
            if len(batches) >= spec.batch_size or position >= mix.shape[1]:
                estimated = model(torch.stack(batches))
                overlap_window = windowing.clone()
                if position - step == 0:
                    overlap_window[:fade] = 1
                elif position >= mix.shape[1]:
                    overlap_window[-fade:] = 1
                for index, (start, segment_length) in enumerate(locations):
                    weighted = estimated[index, ..., :segment_length].cpu() * overlap_window[..., :segment_length]
                    result[..., start:start + segment_length] += weighted
                    counter[..., start:start + segment_length] += overlap_window[..., :segment_length]
                completed_chunks += len(locations)
                if progress is not None:
                    progress(min(completed_chunks, total_chunks), total_chunks)
                batches.clear()
                locations.clear()
    estimated_sources = (result / counter).numpy()
    np.nan_to_num(estimated_sources, copy=False, nan=0.0)
    if length > 2 * border and border > 0:
        estimated_sources = estimated_sources[..., border:-border]
    return {name: value for name, value in zip(spec.sources, estimated_sources)}
