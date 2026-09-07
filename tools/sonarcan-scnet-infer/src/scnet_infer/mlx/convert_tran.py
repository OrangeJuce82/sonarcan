"""PyTorch -> MLX weight conversion for family `"scnet_tran"`.

Split out of `convert.py` because the transformer trunk's key shapes
(Attention/FeedForward/RMSNorm/rotary buffers) are entirely different from
the LSTM families' (`"scnet"`/`"scnet_masked"`) and mixing both regex sets
in one function was harder to audit than two focused ones. The shared
encoder/decoder/SDblock rules live in `convert._convert_trunk_key` and are
reused here rather than duplicated.

Reads: .convert (_convert_trunk_key, _reshape, _to_numpy), mlx.core, numpy
"""

from __future__ import annotations

import re
from typing import Any

import mlx.core as mx
import numpy as np

from .convert import _convert_trunk_key, _reshape, _to_numpy


def _convert_tran_separation_key(key: str):
    """`separation_net.*` keys for the Transformer dual-path net.

    Returns (mlx_key, kind) or None. `kind` is only ever `None` here (no
    conv-layout tensors live in this half of the model)."""

    m = re.match(r"^separation_net\.dp_modules\.(\d+)\.norm_layers\.([01])\.(weight|bias)$", key)
    if m:
        i, side, suffix = m.groups()
        target = "norm_0" if side == "0" else "norm_1"
        return f"separation_net.dp_{i}.{target}.norm.{suffix}", None

    m = re.match(
        r"^separation_net\.dp_modules\.(\d+)\.(time_layer|freq_layer)\.layers\.(\d+)\.0\.(.+)$", key
    )
    if m:
        i, branch, k, rest = m.groups()
        prefix = f"separation_net.dp_{i}.{branch}.attn_{k}"
        if rest == "rotary_embed.freqs":
            return f"{prefix}.rope_freqs", None
        if rest == "norm.gamma":
            return f"{prefix}.norm.weight", None
        if rest in ("to_qkv.weight", "to_gates.weight", "to_gates.bias"):
            return f"{prefix}.{rest}", None
        if rest == "to_out.0.weight":
            return f"{prefix}.to_out.weight", None
        return None

    m = re.match(
        r"^separation_net\.dp_modules\.(\d+)\.(time_layer|freq_layer)\.layers\.(\d+)\.1\.net\.(\d+)\.(.+)$", key
    )
    if m:
        i, branch, k, net_idx, suffix = m.groups()
        prefix = f"separation_net.dp_{i}.{branch}.ff_{k}"
        if net_idx == "0" and suffix == "gamma":
            return f"{prefix}.norm.weight", None
        if net_idx == "1":
            return f"{prefix}.linear1.{suffix}", None
        if net_idx == "4":
            return f"{prefix}.linear2.{suffix}", None
        return None

    m = re.match(r"^separation_net\.dp_modules\.(\d+)\.(time_layer|freq_layer)\.norm\.gamma$", key)
    if m:
        i, branch = m.groups()
        return f"separation_net.dp_{i}.{branch}.norm.weight", None

    return None


def convert_scnet_tran_weights(state_dict: dict[str, Any]) -> dict[str, mx.array]:
    mlx_weights: dict[str, mx.array] = {}

    for key, value in state_dict.items():
        trunk = _convert_trunk_key(key)
        if trunk is not None:
            mlx_key, kind = trunk
            mlx_weights[mlx_key] = mx.array(_reshape(kind, _to_numpy(value)))
            continue

        if key == "first_conv.weight":
            mlx_weights["first_conv.conv.weight"] = mx.array(
                np.transpose(_to_numpy(value), (0, 2, 3, 1))
            )
            continue

        sep = _convert_tran_separation_key(key)
        if sep is not None:
            mlx_key, kind = sep
            mlx_weights[mlx_key] = mx.array(_reshape(kind, _to_numpy(value)))
            continue

        # No MLX-side parameter for this key (e.g. no such key exists for
        # this family); `load_converted_weights` catches any *model*
        # parameter this silently failed to supply.

    return mlx_weights
