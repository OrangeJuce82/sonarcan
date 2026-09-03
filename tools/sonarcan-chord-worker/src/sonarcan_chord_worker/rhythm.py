"""Deterministic, tempo-aware repair of a Beat This! timeline."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import torch


def prepare_postprocessing_logits(logits: torch.Tensor) -> torch.Tensor:
    """Move bounded frame logits to CPU before Beat This! DBN postprocessing.

    Beat This! 1.1.0 converts its DBN inputs to float64 internally, which MPS
    does not support. Inference remains on the selected accelerator; only the
    two one-dimensional output timelines cross this boundary.
    """
    return logits.detach().float().cpu()
