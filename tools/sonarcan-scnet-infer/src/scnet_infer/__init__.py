"""Public facade for inference-only SCNet separation.

The package root exposes only the version, one-shot separator, result type, and
package-qualified reusable session. Heavy runtime imports remain lazy.
Reads: scnet_infer.__about__, scnet_infer.api.
"""

from .__about__ import __version__
from .api import SCNetSession, SeparationResult, separate

__all__ = ["SCNetSession", "SeparationResult", "__version__", "separate"]
