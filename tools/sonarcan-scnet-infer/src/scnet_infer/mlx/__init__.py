"""Thin barrel for the from-scratch MLX SCNet port -- lazy on purpose.

`scnet_infer.mlx` is only ever imported on demand by the (caller-owned) MLX
compute backend, never by the package's default import path, so `import
scnet_infer` stays MLX-free even with this subpackage present. Attribute
access is deferred via `__getattr__` so merely importing this package (e.g.
for introspection) doesn't eagerly import `mlx.core`/`mlx.nn` -- the cost of
that import is paid only when a name is actually used.

Reads: .model (SCNetMLX, SCNetMaskedMLX, SCNetTranMLX, lazily), .convert
(convert_torch_to_mlx_weights, load_converted_weights, lazily)
"""

from __future__ import annotations

__all__ = [
    "SCNetMLX",
    "SCNetMaskedMLX",
    "SCNetTranMLX",
    "convert_torch_to_mlx_weights",
    "load_converted_weights",
]

_MODEL_NAMES = ("SCNetMLX", "SCNetMaskedMLX", "SCNetTranMLX")


def __getattr__(name: str):
    if name in _MODEL_NAMES:
        from . import model

        return getattr(model, name)
    if name in ("convert_torch_to_mlx_weights", "load_converted_weights"):
        from . import convert

        return getattr(convert, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
