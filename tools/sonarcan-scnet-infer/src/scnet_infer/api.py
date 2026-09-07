"""One-shot facade and independent reusable SCNet lifecycle.

Sessions own checkpoint resolution, resident model memory, and terminal state;
one-shot calls intentionally build and dispose their own session. Model
construction, weight loading, and separation itself are delegated to a
`backends.SeparationBackend` -- Torch by default, MLX when requested -- so
this module stays framework-agnostic; see `backends/base.py`.
Reads: audio boundary, checkpoint resolver, device validator, backends seam.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import numpy as np
from typing_extensions import Self

from .audio import load_audio
from .backends import get_backend, resolve_backend_name
from .checkpoints import CheckpointSpec, get_spec, resolve_checkpoint
from .checkpoints import cache_info as inspect_cache
from .device import resolve_device


@dataclass(frozen=True)
class SeparationResult:
    """Named channel-first float32 source waveforms sharing one sample rate."""

    stems: Mapping[str, np.ndarray]
    sample_rate: int


class SCNetSession:
    """Load-once SCNet model with explicit, observable lifecycle state."""

    def __init__(
        self,
        model_id: str | None = None,
        *,
        device: str | None = "auto",
        backend: str | None = None,
        checkpoint_path: str | Path | None = None,
        checkpoint_url: str | None = None,
        checkpoint_sha256: str | None = None,
        cache_dir: str | Path | None = None,
        manifest_path: str | Path | None = None,
        download_progress: Callable[[int, int], None] | None = None,
    ) -> None:
        self._spec: CheckpointSpec = get_spec(model_id, manifest_path)
        self._device_request = device
        self._backend_request = backend
        self._checkpoint_path = checkpoint_path
        self._checkpoint_url = checkpoint_url
        self._checkpoint_sha256 = checkpoint_sha256
        self._cache_dir = cache_dir
        self._download_progress = download_progress
        self._backend = None
        self._status = "new"

    @property
    def status(self) -> str:
        """Return new, ready, released, failed, or closed."""

        return self._status

    @property
    def model_id(self) -> str:
        """Return the stable checkpoint identifier."""

        return self._spec.model_id

    @property
    def device(self) -> str | None:
        """Return the concrete resolved compute target, if ready."""

        return self._backend.resolved_device if self._backend is not None else None

    @property
    def backend(self) -> str | None:
        """Return the resolved backend name (`"torch"` or `"mlx"`), if ready."""

        return self._backend.name if self._backend is not None else None

    def cache_info(self) -> dict[str, object]:
        """Inspect the same checkpoint target used by load, without downloading."""

        return inspect_cache(
            self._spec,
            cache_dir=self._cache_dir,
            checkpoint_path=self._checkpoint_path,
            checkpoint_url=self._checkpoint_url,
        )

    def load(self) -> SCNetSession:
        """Resolve and construct the model once; repeated ready loads are no-ops."""

        if self._status == "closed":
            raise RuntimeError("cannot load a closed SCNetSession")
        if self._status == "ready":
            return self
        try:
            backend_name = resolve_backend_name(self._backend_request, family=self._spec.family)
            checkpoint = resolve_checkpoint(
                self._spec,
                cache_dir=self._cache_dir,
                checkpoint_path=self._checkpoint_path,
                checkpoint_url=self._checkpoint_url,
                checkpoint_sha256=self._checkpoint_sha256,
                progress=self._download_progress,
            )
            backend_cls = get_backend(backend_name)
            if backend_name == "torch":
                device = resolve_device(self._device_request)
                backend = backend_cls.from_checkpoint(spec=self._spec, checkpoint_path=checkpoint, device=device)
            else:
                backend = backend_cls.from_checkpoint(
                    spec=self._spec, checkpoint_path=checkpoint, device=self._device_request
                )
        except Exception:
            self._backend = None
            self._status = "failed"
            raise
        self._backend = backend
        self._status = "ready"
        return self

    def infer(
        self,
        audio: str | Path | np.ndarray,
        *,
        sample_rate: int | None = None,
        progress: Callable[[int, int], None] | None = None,
    ) -> SeparationResult:
        """Separate audio using the already-loaded resident model."""

        if self._status != "ready" or self._backend is None:
            raise RuntimeError("infer() requires a ready SCNetSession; call load() first")
        mixture = load_audio(audio, sample_rate, self._spec.sample_rate)
        stems = self._backend.separate(mixture, progress=progress)
        return SeparationResult(stems=stems, sample_rate=self._spec.sample_rate)

    def release(self) -> None:
        """Release resident model/device memory while retaining disk cache."""

        if self._status == "closed":
            return
        if self._backend is not None:
            self._backend.release()
        self._backend = None
        self._status = "released"

    def close(self) -> None:
        """Terminally close this session; repeated calls are safe."""

        if self._status == "closed":
            return
        self.release()
        self._status = "closed"

    def __enter__(self) -> Self:
        return self.load()

    def __exit__(self, exc_type: object, exc: object, traceback: object) -> None:
        self.close()


def separate(
    audio: str | Path | np.ndarray,
    *,
    sample_rate: int | None = None,
    model_id: str | None = None,
    device: str | None = "auto",
    backend: str | None = None,
    checkpoint_path: str | Path | None = None,
    checkpoint_url: str | None = None,
    checkpoint_sha256: str | None = None,
    cache_dir: str | Path | None = None,
    manifest_path: str | Path | None = None,
    download_progress: Callable[[int, int], None] | None = None,
) -> SeparationResult:
    """Separate one input with a disposable, non-global session."""

    with SCNetSession(
        model_id,
        device=device,
        backend=backend,
        checkpoint_path=checkpoint_path,
        checkpoint_url=checkpoint_url,
        checkpoint_sha256=checkpoint_sha256,
        cache_dir=cache_dir,
        manifest_path=manifest_path,
        download_progress=download_progress,
    ) as session:
        return session.infer(audio, sample_rate=sample_rate)
