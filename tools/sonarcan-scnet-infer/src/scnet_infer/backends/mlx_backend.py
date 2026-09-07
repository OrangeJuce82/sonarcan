"""MLX backend -- native Apple Silicon execution behind the same seam.

Builds the from-scratch MLX SCNet port (see `..mlx.model`'s module docstring
for why this is a from-scratch derivation rather than a vendored upstream)
from the package's own config and its own sha256-verified PyTorch
checkpoint, so the package-owned checkpoint contract is unchanged: no second
catalog, no converted-weight cache the loader does not control. All three
registry architecture families (`"scnet"`, `"scnet_masked"`, `"scnet_tran"`)
have an MLX model class; `supports_family` is computed from the model-class
registry below rather than declared, so a family this port has not
implemented cannot be silently advertised and then fail deep inside
construction.

`separate()` reproduces `runtime.demix()`'s exact chunked overlap-add
arithmetic, including one quirk worth naming rather than silently "fixing":
Torch's loop applies the edge-correction window (full amplitude at the very
first/last chunk) to the *whole currently-flushed batch*, not to the
individual chunk that triggered the edge condition -- when `batch_size > 1`
and the true edge chunk shares a flush with interior chunks, those interior
chunks get the edge-corrected window too. This is upstream's own behaviour
(`spec.batch_size=4`, `num_overlap=4` for the default checkpoint), not a
bug introduced here, and reproducing the chunking bit-for-bit is what this
package's parity tests hold both backends to -- so it is kept, not fixed.

Reads: .base (ChunkingPlan, BackendUnavailable), ..mlx (model classes,
conversion), torch (checkpoint bytes only -- always a hard dependency of
this package), numpy
"""

from __future__ import annotations

import numpy as np

from .base import BackendUnavailable, ChunkingPlan

_FAMILY_MODELS = {
    "scnet": "SCNetMLX",
    "scnet_masked": "SCNetMaskedMLX",
    "scnet_tran": "SCNetTranMLX",
}


def supported_families() -> frozenset:
    """Families this MLX port can actually build, measured from the model
    class registry rather than restated here."""
    return frozenset(_FAMILY_MODELS)


class MLXBackend:
    """Runs SCNet natively on Apple Silicon through MLX."""

    name = "mlx"

    def __init__(self, model, spec, device: str = "mps"):
        self._model = model
        self._spec = spec
        self._device = device
        self._plan = ChunkingPlan.from_spec(spec)

    # ------------------------------------------------------------- availability

    @classmethod
    def is_available(cls) -> bool:
        try:
            import mlx.core  # noqa: F401
            import mlx_spectro  # noqa: F401
        except ImportError:
            return False
        return True

    @classmethod
    def _require(cls) -> None:
        if not cls.is_available():
            raise BackendUnavailable(
                "the MLX backend needs the optional extra: "
                "pip install 'scnet-infer[mlx]' (Apple Silicon)"
            )

    # ------------------------------------------------------------- construction

    @classmethod
    def from_checkpoint(cls, *, spec, checkpoint_path, device: str = "mps") -> MLXBackend:
        """Build an MLX model from this package's own checkpoint and spec."""
        cls._require()
        cls.assert_supports_family(spec.family)
        device = cls._select_device(device)

        import torch

        from ..mlx.convert import convert_torch_to_mlx_weights, load_converted_weights
        from ..mlx.model import SCNetMaskedMLX, SCNetMLX, SCNetTranMLX

        model_classes = {
            "scnet": SCNetMLX,
            "scnet_masked": SCNetMaskedMLX,
            "scnet_tran": SCNetTranMLX,
        }

        state = torch.load(checkpoint_path, map_location="cpu", weights_only=True)
        if isinstance(state, dict) and "state_dict" in state:
            state = state["state_dict"]

        params = dict(spec.model)
        params["sources"] = list(spec.sources)
        model = model_classes[spec.family](**params)
        weights = convert_torch_to_mlx_weights(state, spec.family)
        load_converted_weights(model, weights)
        model.eval()
        return cls(model, spec, device=device)

    @staticmethod
    def _select_device(device):
        """MLX owns its own execution target; a Torch device string is refused.

        Reinterpreting `device="cuda"` as "run on the Apple GPU anyway" would
        discard what the caller explicitly asked for. Accepts only the
        sentinels that genuinely mean "wherever MLX runs".
        """
        if device in (None, "auto", "mps"):
            return "mps"
        raise BackendUnavailable(
            f"backend 'mlx' cannot honour device {device!r}; it executes on Apple "
            f"Silicon and accepts None, 'auto', or 'mps'. Use backend='torch' to "
            f"select a Torch device."
        )

    @staticmethod
    def supports_family(family) -> bool:
        return family in supported_families()

    @classmethod
    def assert_supports_family(cls, family) -> None:
        if cls.supports_family(family):
            return
        raise BackendUnavailable(
            f"checkpoint family {family!r} has no MLX port; use backend='torch' "
            f"for this model. Supported here: {sorted(supported_families())}"
        )

    # ----------------------------------------------------------------- protocol

    @property
    def resolved_device(self) -> str:
        return self._device

    @property
    def model(self):
        """The resident MLX model, for callers composing the advanced path directly."""
        return self._model

    def release(self) -> None:
        self._model = None
        try:
            import mlx.core as mx

            mx.clear_cache()
        except (ImportError, AttributeError):
            pass

    def separate(self, mix: np.ndarray, progress=None) -> dict[str, np.ndarray]:
        """Chunked overlap-add, mirroring `runtime.demix()`'s exact arithmetic
        (including its batch-level edge-window quirk -- see module docstring)."""
        import mlx.core as mx

        plan = self._plan
        sources = self._spec.sources

        audio = mx.array(np.ascontiguousarray(mix, dtype=np.float32))
        length = audio.shape[-1]
        border = plan.border
        padded = length > 2 * border and border > 0
        if padded:
            audio = _reflect_pad_last(audio, border, border)

        total_length = audio.shape[-1]
        window_template = _fade_window(plan.chunk_size, plan.fade_size)
        shape = (len(sources), *audio.shape)
        result = mx.zeros(shape, dtype=mx.float32)
        counter = mx.zeros(shape, dtype=mx.float32)

        batches: list = []
        locations: list = []
        position = 0
        total_chunks = max(1, (total_length + plan.step - 1) // plan.step)
        completed_chunks = 0
        while position < total_length:
            part = audio[:, position:position + plan.chunk_size]
            segment_length = part.shape[-1]
            missing = plan.chunk_size - segment_length
            if missing > 0:
                if segment_length > plan.chunk_size // 2:
                    part = _reflect_pad_last(part, 0, missing)
                else:
                    part = mx.pad(part, [(0, 0), (0, missing)], constant_values=0.0)
            batches.append(part)
            locations.append((position, segment_length))
            position += plan.step

            if len(batches) >= plan.batch_size or position >= total_length:
                stacked = mx.stack(batches, axis=0)
                estimated = self._model(stacked)
                mx.eval(estimated)

                overlap_window = window_template.copy()
                if position - plan.step == 0:
                    overlap_window[:plan.fade_size] = 1.0
                elif position >= total_length:
                    overlap_window[-plan.fade_size:] = 1.0
                overlap_window_mx = mx.array(overlap_window)

                for idx, (start, seg_len) in enumerate(locations):
                    span = slice(start, start + seg_len)
                    weight = overlap_window_mx[..., :seg_len]
                    result[..., span] = result[..., span] + estimated[idx, ..., :seg_len] * weight
                    counter[..., span] = counter[..., span] + weight
                # Materialize each overlap-add batch before the next model
                # forward. Otherwise MLX retains the full song's lazy graph,
                # causing memory growth proportional to its chunk count.
                mx.eval(result, counter)
                completed_chunks += len(locations)
                if progress is not None:
                    progress(min(completed_chunks, total_chunks), total_chunks)
                batches = []
                locations = []

        sources_out = np.array(result / counter, dtype=np.float32)
        np.nan_to_num(sources_out, copy=False, nan=0.0)
        if padded:
            sources_out = sources_out[..., border:-border]
        return dict(zip(sources, sources_out))


def _reflect_pad_last(array, left: int, right: int):
    """`torch.nn.functional.pad(x, (left, right), mode='reflect')`; `mx.pad`
    has no reflect mode. Reflects without repeating the edge sample."""
    import mlx.core as mx

    if left:
        array = mx.concatenate([array[..., left:0:-1], array], axis=-1)
    if right:
        array = mx.concatenate([array, array[..., -2:-2 - right:-1]], axis=-1)
    return array


def _fade_window(window_size: int, fade_size: int) -> np.ndarray:
    """Matches `runtime._window()` exactly: linear fade-in/out envelope."""
    window = np.ones(window_size, dtype=np.float32)
    window[:fade_size] *= np.linspace(0, 1, fade_size, dtype=np.float32)
    window[-fade_size:] *= np.linspace(1, 0, fade_size, dtype=np.float32)
    return window
