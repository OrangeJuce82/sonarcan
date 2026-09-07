"""PyTorch -> MLX weight conversion for the three SCNet families, plus a
strict load gate.

`convert_torch_to_mlx_weights(state_dict, family)` renames a PyTorch
`state_dict`'s keys to match the MLX module tree built by `model.py`, and
reshapes the handful of tensor layouts that differ between the two
frameworks: `Conv2d` OIHW -> MLX OHWI, `ConvTranspose2d` IOHW -> MLX OHWI,
`Conv1d` (out, in/groups, k) -> MLX (out, k, in/groups). `Linear` and
`GroupNorm` weights need no reshape (both frameworks use the same `(out,
in)` / `(channels,)` layout). Torch's bidirectional `LSTM` packs two biases
per direction (`bias_ih_l0`, `bias_hh_l0`); MLX's `nn.LSTM` has one, so the
two are summed -- mathematically identical, since both act as a single
additive term before the gate nonlinearities (`i,f,g,o` gate order already
matches between the two implementations; see `model.py::BiLSTM`).

There is no upstream MLX SCNet to vendor a loader from (see `model.py`'s
module docstring), so unlike the sibling packages' `convert.py`, none of this
file is vendored -- it is derived directly from a real PyTorch
`state_dict()` dump of this package's own `SCNet`/`SCNet` (masked)/
`SCNet_Tran` classes, cross-checked key by key.

`load_converted_weights()` diffs the model's own parameter keys (via
`mlx.utils.tree_flatten`) against the converted weight keys and raises a
`ValueError` naming the mismatch before calling `load_weights` -- callers
must use this instead of calling `model.load_weights(..., strict=False)`
directly, which silently drops any unmatched key and leaves the
corresponding layer at random initialisation.

Reads: mlx.core, mlx.nn, mlx.utils (tree_flatten), numpy
"""

from __future__ import annotations

import re
from typing import Any

import mlx.core as mx
import numpy as np
from mlx import nn
from mlx.utils import tree_flatten


def _to_numpy(value: Any) -> np.ndarray:
    try:
        return value.detach().cpu().numpy()
    except AttributeError:
        return np.array(value)


# --------------------------------------------------------------------------- #
# Shared trunk: encoder / decoder / SDblock / ConvolutionModule -- identical
# module tree shape across all three families (each Torch source file
# duplicates this trunk verbatim; see CLAUDE.md's "faithful ... extracted
# from MSST" note on each file).
# --------------------------------------------------------------------------- #

_CONV_MODULE_INDEX = {"0": "norm1", "1": "conv1", "3": "conv2", "4": "norm2", "6": "conv3"}
_CONV1D_KEYS = {"conv1", "conv2", "conv3"}


def _convert_trunk_key(key: str):
    """Encoder/decoder/SDblock/SUlayer/ConvolutionModule key -> (mlx_key, kind)
    or None if `key` is not part of the shared trunk (caller tries family-
    specific rules next). `kind` selects the reshape rule applied by the
    caller: "conv2d", "convtranspose2d", "conv1d", or None (no reshape)."""

    m = re.match(r"^encoder\.(\d+)\.SDlayer\.convs\.(\d+)\.(weight|bias)$", key)
    if m:
        i, j, suffix = m.groups()
        return f"encoder_{i}.SDlayer.band_{j}.conv.{suffix}", ("conv2d" if suffix == "weight" else None)

    m = re.match(r"^encoder\.(\d+)\.conv_modules\.(\d+)\.layers\.(\d+)\.(\d+)\.(weight|bias)$", key)
    if m:
        i, j, k, idx, suffix = m.groups()
        sub = _CONV_MODULE_INDEX.get(idx)
        if sub is None:
            return None  # GLU / Swish: no parameters
        kind = ("conv1d" if suffix == "weight" and sub in _CONV1D_KEYS else None)
        return f"encoder_{i}.conv_module_{j}.layers_{k}.{sub}.{suffix}", kind

    m = re.match(r"^encoder\.(\d+)\.globalconv\.(weight|bias)$", key)
    if m:
        i, suffix = m.groups()
        return f"encoder_{i}.globalconv.conv.{suffix}", ("conv2d" if suffix == "weight" else None)

    m = re.match(r"^decoder\.(\d+)\.0\.conv\.(weight|bias)$", key)
    if m:
        d, suffix = m.groups()
        return f"decoder_fusion_{d}.conv.conv.{suffix}", ("conv2d" if suffix == "weight" else None)

    m = re.match(r"^decoder\.(\d+)\.1\.convtrs\.(\d+)\.(weight|bias)$", key)
    if m:
        d, j, suffix = m.groups()
        return f"decoder_su_{d}.band_{j}.conv.{suffix}", ("convtranspose2d" if suffix == "weight" else None)

    return None


def _reshape(kind, value: np.ndarray) -> np.ndarray:
    if kind == "conv2d":
        return np.transpose(value, (0, 2, 3, 1))  # OIHW -> OHWI
    if kind == "convtranspose2d":
        return np.transpose(value, (1, 2, 3, 0))  # IOHW -> OHWI
    if kind == "conv1d":
        return np.transpose(value, (0, 2, 1))  # (out, in/groups, k) -> (out, k, in/groups)
    return value


# --------------------------------------------------------------------------- #
# LSTM dual-path trunk (families "scnet" and "scnet_masked")
# --------------------------------------------------------------------------- #

_LSTM_SIDE = {"0": "norm_0", "1": "norm_1"}


def _convert_lstm_separation_key(key: str):
    m = re.match(r"^separation_net\.dp_modules\.(\d+)\.norm_layers\.([01])\.(weight|bias)$", key)
    if m:
        i, side, suffix = m.groups()
        return f"separation_net.dp_{i}.{_LSTM_SIDE[side]}.norm.{suffix}", None

    m = re.match(r"^separation_net\.dp_modules\.(\d+)\.linear_layers\.([01])\.(weight|bias)$", key)
    if m:
        i, side, suffix = m.groups()
        target = "linear_0" if side == "0" else "linear_1"
        return f"separation_net.dp_{i}.{target}.{suffix}", None

    return None


def _lstm_weight_target(key: str):
    """LSTM weight (not bias -- those are summed separately, see
    `_merge_lstm_biases`) -> (mlx_key, None). `Wx`/`Wh` need no reshape:
    MLX's `nn.LSTM` stores them in the same `(4*hidden, in_features)` /
    `(4*hidden, hidden)` layout torch's `weight_ih_l0`/`weight_hh_l0` do."""
    m = re.match(
        r"^separation_net\.dp_modules\.(\d+)\.lstm_layers\.([01])\.weight_(ih|hh)_l0(_reverse)?$", key
    )
    if not m:
        return None
    i, side, which, reverse = m.groups()
    target = "lstm_0" if side == "0" else "lstm_1"
    direction = "backward_cell" if reverse else "forward_cell"
    param = "Wx" if which == "ih" else "Wh"
    return f"separation_net.dp_{i}.{target}.{direction}.{param}", None


def _lstm_bias_target(key: str):
    """Whether `key` is one of the two torch biases MLX's single `bias`
    combines. Returns (mlx_key, torch_component) or None."""
    m = re.match(
        r"^separation_net\.dp_modules\.(\d+)\.lstm_layers\.([01])\.bias_(ih|hh)_l0(_reverse)?$", key
    )
    if not m:
        return None
    i, side, which, reverse = m.groups()
    target = "lstm_0" if side == "0" else "lstm_1"
    direction = "backward_cell" if reverse else "forward_cell"
    return f"separation_net.dp_{i}.{target}.{direction}.bias", which


# --------------------------------------------------------------------------- #
# Family entry points
# --------------------------------------------------------------------------- #


def convert_torch_to_mlx_weights(state_dict: dict[str, Any], family: str) -> dict[str, mx.array]:
    """Convert one family's PyTorch `state_dict` to MLX weights + layout."""
    if family in ("scnet", "scnet_masked"):
        return _convert_lstm_family(state_dict, family)
    if family == "scnet_tran":
        from .convert_tran import convert_scnet_tran_weights

        return convert_scnet_tran_weights(state_dict)
    raise ValueError(f"no MLX conversion registered for SCNet family {family!r}")


def _convert_lstm_family(state_dict: dict[str, Any], family: str) -> dict[str, mx.array]:
    mlx_weights: dict[str, mx.array] = {}
    bias_components: dict[str, dict[str, np.ndarray]] = {}

    for key, value in state_dict.items():
        trunk = _convert_trunk_key(key)
        if trunk is not None:
            mlx_key, kind = trunk
            mlx_weights[mlx_key] = mx.array(_reshape(kind, _to_numpy(value)))
            continue

        sep = _convert_lstm_separation_key(key)
        if sep is not None:
            mlx_key, kind = sep
            mlx_weights[mlx_key] = mx.array(_reshape(kind, _to_numpy(value)))
            continue

        weight = _lstm_weight_target(key)
        if weight is not None:
            mlx_key, _ = weight
            mlx_weights[mlx_key] = mx.array(_to_numpy(value))
            continue

        bias = _lstm_bias_target(key)
        if bias is not None:
            mlx_key, component = bias
            bias_components.setdefault(mlx_key, {})[component] = _to_numpy(value)
            continue

        extra = _convert_family_specific_key(key, value, family)
        if extra is not None:
            mlx_key, arr = extra
            mlx_weights[mlx_key] = mx.array(arr)
            continue

        # STFT/window buffers and anything else with no MLX-side parameter
        # are intentionally dropped here; `load_converted_weights` catches
        # any *model* parameter this silently failed to supply.

    for mlx_key, parts in bias_components.items():
        if "ih" in parts and "hh" in parts:
            mlx_weights[mlx_key] = mx.array(parts["ih"] + parts["hh"])

    return mlx_weights


def _convert_family_specific_key(key: str, value: Any, family: str):
    """`scnet_masked`-only parameters: `pos_embed_f` and `mask_layer`.
    Returns (mlx_key, numpy_array) or None."""
    if family != "scnet_masked":
        return None

    if key == "pos_embed_f":
        return "pos_embed_f", _to_numpy(value)

    # mask_layer = nn.Sequential(Conv2d, GELU, Conv2d, Tanh); indices 0 and 2
    # carry parameters, 1 and 3 do not.
    m = re.match(r"^mask_layer\.(0|2)\.(weight|bias)$", key)
    if m:
        idx, suffix = m.groups()
        target = "mask_conv1" if idx == "0" else "mask_conv2"
        arr = _to_numpy(value)
        if suffix == "weight":
            arr = np.transpose(arr, (0, 2, 3, 1))  # Conv2d OIHW -> OHWI
        return f"{target}.conv.{suffix}", arr

    return None


def load_converted_weights(model: nn.Module, mlx_weights: dict[str, mx.array]) -> None:
    """Load `mlx_weights` into `model`, refusing a silent partial load.

    `model.load_weights(..., strict=False)` on its own accepts any degree of
    mismatch between the checkpoint and the module tree, dropping whatever
    doesn't line up without a warning. This checks first: every one of the
    model's own parameter keys (from `mlx.utils.tree_flatten(model.
    parameters())`) must be present in `mlx_weights`, and every key in
    `mlx_weights` must be consumed by the model -- otherwise a `ValueError`
    is raised naming counts and up to 5 example keys on each side, so a
    conversion bug or a mismatched checkpoint fails loudly instead of
    loading a partially-random model.
    """
    model_keys = {key for key, _ in tree_flatten(model.parameters())}
    weight_keys = set(mlx_weights.keys())

    unmatched_model = sorted(model_keys - weight_keys)
    dropped_weights = sorted(weight_keys - model_keys)

    if unmatched_model or dropped_weights:
        parts = []
        if unmatched_model:
            example = ", ".join(unmatched_model[:5])
            parts.append(f"{len(unmatched_model)} model parameters unmatched (e.g. {example})")
        if dropped_weights:
            example = ", ".join(dropped_weights[:5])
            parts.append(f"{len(dropped_weights)} converted tensors dropped (e.g. {example})")
        raise ValueError("MLX weight conversion incomplete: " + ", ".join(parts))

    model.load_weights(list(mlx_weights.items()), strict=False)
