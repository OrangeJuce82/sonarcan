"""Public facade for inference-only SCNet separation.

The package root exposes only the version, one-shot separator, result type, and
package-qualified reusable session. Heavy runtime imports remain lazy.
Reads: scnet_infer.__about__, scnet_infer.api.
"""

from typing import TYPE_CHECKING, Any

from .__about__ import __version__

if TYPE_CHECKING:
    from .api import SCNetSession, SeparationResult, separate

__all__ = ["SCNetSession", "SeparationResult", "__version__", "separate"]


def __getattr__(name: str) -> Any:
    if name not in {"SCNetSession", "SeparationResult", "separate"}:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    from . import api

    return getattr(api, name)
