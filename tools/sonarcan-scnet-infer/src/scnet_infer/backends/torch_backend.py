"""PyTorch backend -- the shipped path, unchanged, behind the seam.

Holds an already-constructed, already-loaded model plus its resolved device and
turns a mixture into stems by calling the same `runtime.demix()` the package has
always used; it is a wrapper, never a second implementation, so the Torch path
cannot drift from `SCNetSession`'s pre-seam behaviour.

Reads: ..runtime (build_model, load_runtime_model, demix), .base
(SeparationBackend), torch, numpy
"""

from __future__ import annotations

import numpy as np
import torch

from ..runtime import demix, load_runtime_model


class TorchBackend:
    """Wraps a loaded SCNet model and its device as a SeparationBackend."""

    name = "torch"

    def __init__(self, model, spec, device: torch.device):
        self._model = model.eval()
        self._spec = spec
        self._device = device

    @classmethod
    def is_available(cls) -> bool:
        return True

    @classmethod
    def from_checkpoint(cls, *, spec, checkpoint_path, device: torch.device) -> TorchBackend:
        """Build a Torch model from this package's own checkpoint and spec."""
        model = load_runtime_model(spec, checkpoint_path, device)
        return cls(model, spec, device)

    @property
    def resolved_device(self) -> str:
        return str(self._device)

    @property
    def model(self):
        """The resident model, for callers composing the advanced path directly."""
        return self._model

    def separate(self, mix: np.ndarray, progress=None) -> dict[str, np.ndarray]:
        return demix(self._spec, self._model, mix, self._device, progress=progress)

    def release(self) -> None:
        self._model = None
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
        backend = getattr(torch.backends, "mps", None)
        if backend is not None and backend.is_available():
            torch.mps.empty_cache()
