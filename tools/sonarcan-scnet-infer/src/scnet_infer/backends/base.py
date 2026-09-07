"""The backend seam -- one narrow protocol every compute backend implements.

A backend owns everything framework-specific: model construction, checkpoint
weights, tensor layout, device placement, and the chunked overlap-add itself.
Chunking accumulates on-device for speed, so the seam deliberately sits at a
whole mixture rather than a single chunk -- a per-chunk seam would drag every
accumulator back to the host and hand the acceleration straight back (same
reasoning as the sibling `bs-roformer-infer` / `mdxnet-infer` packages' seams;
see `bs_roformer/brain/architecture.md`).

Above the seam nothing knows a dtype, a tensor layout, or which chip is busy:
`api.SCNetSession` decides checkpoint resolution, device/backend requests, and
result packaging once, the same way for every backend.

Reads: numpy (boundary array type only)
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, runtime_checkable

import numpy as np


@dataclass(frozen=True)
class ChunkingPlan:
    """How one mixture is cut into overlapping chunks -- owner for the MLX path only.

    `MLXBackend` builds and consumes this (`from_spec`, then `.step`/
    `.fade_size`/`.border` in `separate()`). The Torch path does **not** go
    through it: `runtime.demix()` still computes the identical formula
    inline (`step = chunk_size // num_overlap`, `fade = chunk_size // 10`,
    `border = chunk_size - step`) at `runtime.py:60-63`. That is the same
    design decision encoded in two places, not one -- a known, deliberately
    deferred duplication rather than a fixed one. Migrating `demix()` onto
    this class is a recorded post-merge follow-up, not done here.
    `tests/test_chunking_plan.py` cross-checks both formulas against the
    registry's real chunk configs so the two cannot drift apart silently
    in the meantime.
    """

    chunk_size: int
    num_overlap: int
    batch_size: int
    step: int
    fade_size: int
    border: int

    @classmethod
    def from_spec(cls, spec) -> ChunkingPlan:
        chunk_size = spec.chunk_size
        num_overlap = spec.num_overlap
        step = chunk_size // num_overlap
        return cls(
            chunk_size=chunk_size,
            num_overlap=num_overlap,
            batch_size=spec.batch_size,
            step=step,
            fade_size=chunk_size // 10,
            border=chunk_size - step,
        )


@runtime_checkable
class SeparationBackend(Protocol):
    """Turns one mixture into stems, hiding how and where it computed them."""

    #: Stable identifier, matching the `backend=` argument that selects it.
    name: str

    @classmethod
    def is_available(cls) -> bool:
        """True when this backend can actually run on this machine right now."""

    @property
    def resolved_device(self) -> str:
        """The concrete target chosen, after any `auto`/`None` sentinel was resolved."""

    def separate(self, mix: np.ndarray, progress=None) -> dict[str, np.ndarray]:
        """Separate one `(channels, samples)` float32 mixture into named stems.

        Every returned array has the same shape as `mix`.
        """

    def release(self) -> None:
        """Drop resident model and device memory. Disk checkpoints stay."""


class BackendUnavailable(RuntimeError):
    """Raised when a backend is requested by name but cannot run here.

    Always raised, never swallowed into a fallback: silently substituting a
    different backend discards what the caller explicitly asked for, and it
    would only be discovered by noticing the wrong hardware was busy.
    """
