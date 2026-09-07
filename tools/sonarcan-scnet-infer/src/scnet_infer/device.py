"""Explicit compute-device validation.

Auto-detection chooses CUDA when available and CPU otherwise; explicit requests
are honored or rejected without silent fallback. `auto` deliberately never
resolves to MPS, on purpose, permanently -- see `mps_available()` below.
`device="mps"` on the Torch backend is refused categorically -- see
`resolve_device()`'s docstring for why this is a *stronger*-founded refusal
than the package's original one, not a reopened question.
Reads: torch backend availability.
"""

from __future__ import annotations

import re

import torch


def mps_available() -> bool:
    """True when this torch build exposes a usable Apple Silicon MPS backend.

    Used only for diagnostics and the real-checkpoint evidence-gathering test
    (`tests/test_device_parity.py`) -- `resolve_device()` itself refuses
    `"mps"` unconditionally, whether or not it is actually available, so this
    is not consulted on that path.
    """

    backend = getattr(torch.backends, "mps", None)
    return bool(backend is not None and backend.is_available())


def resolve_device(requested: str | None) -> torch.device:
    """Resolve one concrete supported device.

    `mps` is refused on the Torch backend, categorically, regardless of
    whether this machine has a working MPS build. This is not the package's
    original "unverified" refusal reinstated unchanged -- it replaces an
    *unverified* refusal with a *verified* one, which is a stronger reason to
    keep it, not a weaker one. Measurement (see CHANGELOG.md and
    `tests/test_device_parity.py`'s module docstring for the full record):
    torch==2.13.0's MPS backend intermittently mis-computes this model's
    chunked (`batch_size=4`) forward pass outright -- not a numeric nudge, a
    large, exactly-repeating wrong answer -- roughly one to two times in
    every three calls, proven by a fixed CPU reference disagreeing with
    looped MPS calls on the same model and input in the same process. A
    silent wrong stem a third to two-thirds of the time is not a tradeoff
    worth offering through this backend. Apple Silicon acceleration is
    available and, on the same evidence, reliable: use `backend="mlx"`,
    which matched the CPU reference exactly on every trial in that
    investigation.

    `auto` keeps its legacy meaning -- CUDA if available, else CPU -- and
    never promotes a Mac caller onto MPS on its own regardless.
    """

    value = "auto" if requested is None else requested.lower()
    if value == "auto":
        return torch.device("cuda:0" if torch.cuda.is_available() else "cpu")
    if value == "cpu":
        return torch.device("cpu")
    if value == "mps":
        raise ValueError(
            "device='mps' is refused on the torch backend: torch==2.13.0's MPS "
            "backend was measured to intermittently mis-compute this model's "
            "chunked forward pass (a large, repeating wrong answer, roughly "
            "1-2 times in 3 calls -- see CHANGELOG.md). Use backend='mlx' for "
            "native, verified-reliable Apple Silicon acceleration instead "
            "(pip install 'scnet-infer[mlx]'); use device='cpu' for a Torch "
            "device that has no such known issue."
        )
    if value == "cuda":
        value = "cuda:0"
    match = re.fullmatch(r"cuda:(\d+)", value)
    if match:
        index = int(match.group(1))
        if not torch.cuda.is_available():
            raise RuntimeError(f"explicit device {value!r} requested but CUDA is unavailable")
        if index >= torch.cuda.device_count():
            raise RuntimeError(f"explicit device {value!r} requested but only {torch.cuda.device_count()} CUDA device(s) exist")
        return torch.device(value)
    raise ValueError("device must be one of: auto, cpu, cuda, cuda:N")
