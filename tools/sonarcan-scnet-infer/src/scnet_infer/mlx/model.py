"""SCNet ported to Apple MLX -- a from-scratch port, not a vendored upstream.

Unlike the sibling `bs-roformer-infer` / `mdxnet-infer` packages, no upstream
MLX SCNet implementation exists anywhere to vendor (searched: `mlx-audio-
separator`, `mlx-audio`, no hits). This module is derived directly from this
package's own Torch source (`scnet_infer._model.scnet`, `.scnet_masked`,
`.scnet_tran`), preserving the same forward-pass math and module tree shape
(so `mlx/convert.py`'s key mapping stays a straightforward rename) while
adapting every tensor op to MLX's conventions.

**Layout strategy.** MLX's `Conv1d`/`Conv2d`/`ConvTranspose2d` are
channels-last (NLC / NHWC); this package's Torch source is channels-first
throughout (NCL / NCHW) and leans on that explicitly -- `DualPathRNN` and
`DualPathTran` do axis-specific `.transpose()`/`.view()` gymnastics keyed to
"channel is axis 1". Porting that gymnastics into a channels-last tensor
would require re-deriving every transpose by hand and re-verifying each one,
which is exactly the kind of "looks right, computes the wrong thing" risk
this package's CLAUDE.md warns about. Instead, `Conv1dCF`/`Conv2dCF`/
`ConvTranspose2dCF`/`GroupNormCF` below wrap the MLX-native (channels-last)
layers and transpose in and back out at each call, so the *rest* of the
model can be ported nearly line-for-line from Torch in channels-first shape.
This costs extra `moveaxis` calls at each conv/norm boundary; correctness
was prioritized over shaving them, and MLX's lazy graph can fuse a
transpose into the surrounding op on Metal regardless.

**The Metal rfft artifact.** MLX 0.31.2's `mx.fft.rfft` returns ~4.5e-07 for
an all-zero frame instead of exact 0 (see `exact_zero_safe_rfft` below).
`FeatureConversion` (inside every dual-path stack here) calls `rfft`/`irfft`
directly on intermediate feature maps, not on raw audio, so whether this
artifact is load-bearing for *this* architecture needed its own measurement
rather than assuming the sibling package's finding transfers -- see
`exact_zero_safe_rfft`'s docstring for what was measured and how.

**Rotary embeddings (SCNet-Tran only).** `scnet_tran.py`'s
`rotary_embedding_torch.RotaryEmbedding` stores inverse frequencies as a
buffer, not a derived-on-the-fly formula, and a sibling package's
`large_inst` checkpoint proved those can drift from the theta=10000 default
during training (up to 0.0088, with some entries negative). This port never
assumes: it always loads the checkpoint's actual `rotary_embed.freqs` buffer
and inverts it for `mx.fast.rope` (which wants the reciprocal of what
`rotary_embedding_torch` stores -- confirmed empirically, see
`mlx/convert.py`). For the record: the real `scnet-tran-v1.0.14` checkpoint's
freqs matched the theta=10000 default bit-for-bit (max abs diff 0.0), so this
particular checkpoint did not need the caution -- but the loader does not
special-case that, because the next checkpoint might.

Reads: mlx.core, mlx.nn, mlx_spectro (get_transform_mlx)
"""

from __future__ import annotations

import math
from contextlib import contextmanager

import mlx.core as mx
from mlx import nn
from mlx_spectro import get_transform_mlx

# --------------------------------------------------------------------------- #
# Metal rfft workaround
# --------------------------------------------------------------------------- #


@contextmanager
def exact_zero_safe_rfft():
    """Route `mx.fft.rfft` through the CPU stream for one STFT. NOT upstream code.

    MLX 0.31.2's Metal rfft kernel packs two real FFTs into one complex FFT; in
    float32 that cancellation is not bit-exact, so a frame whose true value is
    exactly zero comes back as roughly 4.5e-07 instead of 0 (same root cause as
    `bs_roformer.mlx.model.exact_zero_safe_rfft` and
    `mdxnet_infer.mlx.model.exact_zero_safe_rfft` -- same org, same MLX kernel).

    Whether that artifact is *load-bearing* depends on what consumes it, so it
    was measured for this architecture rather than assumed:

    - The trunk STFT (`SCNet*.forward`'s `torch.stft`/`istft` equivalent) reads
      raw audio, where a genuinely silent region legitimately produces a
      near-zero frame anyway -- this call site is not where the artifact would
      matter.
    - `FeatureConversion` (inside every dual-path stack in this file) applies
      `rfft`/`irfft` to intermediate *feature* activations, not raw audio, and
      those activations sit downstream of `GroupNorm` (eps=1e-5, three orders
      above the ~4.5e-07 artifact -- swallowed by the epsilon floor rather than
      amplified) and `RMSNorm`/LSTM gates, none of which discard a token's own
      magnitude and renormalize it the way the Roformer sibling's `L2Norm`
      (eps=1e-12) does. There is no single operation here that turns a
      4.5e-07 artifact into a full-scale feature the way it does there.

    Measured end to end through the public `.separate()` API on the real
    the then-current real SCNet checkpoint (2026-07-31, Apple M-series, torch
    2.13.0, mlx 0.31.2), zeros-tail worst-case max-abs Torch(MPS)-vs-MLX
    divergence with the guard in place and with it removed (then restored,
    to confirm the swap/restore itself is exact):

        with guard     7.4215e-05
        without guard  7.7039e-05  (~4% shift, not an order of magnitude)
        restored       7.4215e-05  (bit-identical to the original with-guard run)

    See `tests/test_mlx_parity.py`'s module docstring for the full
    trial-by-trial record, including a separate, more serious finding
    surfaced during this same investigation: Torch's own MPS backend was
    observed to intermittently mis-compute this model's chunked forward pass
    outright (a reproducible wrong answer, not a numeric nudge) -- unrelated
    to this guard, and traced there in detail before landing on the 4%
    figure above as this guard's own, much smaller contribution.

    Caveat, stated rather than hidden: this swaps a module-level attribute, so
    it is not thread-safe. Inference here is single-threaded per session.
    """
    original = mx.fft.rfft

    def cpu_stream_rfft(*args, **kwargs):
        with mx.stream(mx.cpu):
            result = original(*args, **kwargs)
            mx.eval(result)
        return result

    mx.fft.rfft = cpu_stream_rfft
    try:
        yield
    finally:
        mx.fft.rfft = original


# --------------------------------------------------------------------------- #
# Channels-first wrappers around MLX's channels-last conv/norm primitives
# --------------------------------------------------------------------------- #


def _cf_to_cl(x: mx.array) -> mx.array:
    """(B, C, ...) -> (B, ..., C): move the channel axis (1) to last."""
    return mx.moveaxis(x, 1, -1)


def _cl_to_cf(x: mx.array) -> mx.array:
    """(B, ..., C) -> (B, C, ...): move the channel axis (last) back to 1."""
    return mx.moveaxis(x, -1, 1)


class Conv1dCF(nn.Module):
    """`torch.nn.Conv1d`-shaped wrapper: accepts/returns (B, C, L)."""

    def __init__(self, in_channels, out_channels, kernel_size, stride=1, padding=0, groups=1, bias=True):
        super().__init__()
        self.conv = nn.Conv1d(
            in_channels, out_channels, kernel_size,
            stride=stride, padding=padding, groups=groups, bias=bias,
        )

    def __call__(self, x: mx.array) -> mx.array:
        return _cl_to_cf(self.conv(_cf_to_cl(x)))


class Conv2dCF(nn.Module):
    """`torch.nn.Conv2d`-shaped wrapper: accepts/returns (B, C, H, W)."""

    def __init__(self, in_channels, out_channels, kernel_size, stride=1, padding=0, bias=True):
        super().__init__()
        self.conv = nn.Conv2d(
            in_channels, out_channels, kernel_size, stride=stride, padding=padding, bias=bias,
        )

    def __call__(self, x: mx.array) -> mx.array:
        return _cl_to_cf(self.conv(_cf_to_cl(x)))


class ConvTranspose2dCF(nn.Module):
    """`torch.nn.ConvTranspose2d`-shaped wrapper: accepts/returns (B, C, H, W)."""

    def __init__(self, in_channels, out_channels, kernel_size, stride=1):
        super().__init__()
        self.conv = nn.ConvTranspose2d(in_channels, out_channels, kernel_size, stride=stride)

    def __call__(self, x: mx.array) -> mx.array:
        return _cl_to_cf(self.conv(_cf_to_cl(x)))


class GroupNormCF(nn.Module):
    """`torch.nn.GroupNorm`-shaped wrapper: accepts/returns channels-first."""

    def __init__(self, num_groups, num_channels, eps=1e-5):
        super().__init__()
        self.norm = nn.GroupNorm(num_groups, num_channels, eps=eps, pytorch_compatible=True)

    def __call__(self, x: mx.array) -> mx.array:
        return _cl_to_cf(self.norm(_cf_to_cl(x)))


def reflect_pad_last(x: mx.array, left: int, right: int) -> mx.array:
    """`torch.nn.functional.pad(x, (left, right), mode="reflect")` along the
    last axis; MLX's `mx.pad` has no reflect mode. Reflects without repeating
    the edge sample, matching torch's/numpy's convention."""
    if left:
        x = mx.concatenate([x[..., left:0:-1], x], axis=-1)
    if right:
        x = mx.concatenate([x, x[..., -2:-2 - right:-1]], axis=-1)
    return x


def reflect_pad_axis2(x: mx.array, left: int, right: int) -> mx.array:
    """`F.pad(x, (0, 0, left, right), mode="reflect")`: reflect along axis -2
    (SDlayer's frequency axis) while leaving the last axis untouched."""
    if left:
        x = mx.concatenate([x[:, :, left:0:-1, :], x], axis=2)
    if right:
        x = mx.concatenate([x, x[:, :, -2:-2 - right:-1, :]], axis=2)
    return x


# --------------------------------------------------------------------------- #
# Shared building blocks (byte-for-byte identical across scnet.py /
# scnet_masked.py / scnet_tran.py on the Torch side, so shared once here)
# --------------------------------------------------------------------------- #


class Swish(nn.Module):
    def __call__(self, x: mx.array) -> mx.array:
        return x * mx.sigmoid(x)


class ConvolutionModule(nn.Module):
    """Residual depthwise-separable conv stack, ported from Torch's
    `ConvolutionModule` (channels, depth, compress, kernel). Runs in
    channels-last internally (one transpose in, one out) since every op here
    -- GroupNorm, Conv1d, GLU -- is native in that layout; the residual add
    happens in channels-first to match `x = x + layer(x)` exactly."""

    def __init__(self, channels, depth=2, compress=4, kernel=3):
        super().__init__()
        assert kernel % 2 == 1
        self.depth = abs(depth)
        hidden_size = int(channels / compress)
        padding = kernel // 2

        for idx in range(self.depth):
            block = nn.Module()
            block.norm1 = nn.GroupNorm(1, channels, eps=1e-5, pytorch_compatible=True)
            block.conv1 = nn.Conv1d(channels, hidden_size * 2, kernel, padding=padding)
            block.conv2 = nn.Conv1d(hidden_size, hidden_size, kernel, padding=padding, groups=hidden_size)
            block.norm2 = nn.GroupNorm(1, hidden_size, eps=1e-5, pytorch_compatible=True)
            block.conv3 = nn.Conv1d(hidden_size, channels, 1)
            setattr(self, f"layers_{idx}", block)

    def __call__(self, x: mx.array) -> mx.array:
        # x: (B, C, L) channels-first, matching the Torch call site. Transpose
        # once at each end; every op in between (GroupNorm/Conv1d/GLU) is
        # native channels-last, and the residual add commutes with the
        # transpose (it is just an axis relabelling, not a computation).
        y = _cf_to_cl(x)
        for idx in range(self.depth):
            block = getattr(self, f"layers_{idx}")
            delta = block.norm1(y)
            delta = block.conv1(delta)
            delta = nn.glu(delta, axis=-1)
            delta = block.conv2(delta)
            delta = block.norm2(delta)
            delta = delta * mx.sigmoid(delta)  # Swish
            delta = block.conv3(delta)
            y = y + delta
        return _cl_to_cf(y)


class FusionLayer(nn.Module):
    def __init__(self, channels, kernel_size=3, stride=1, padding=1):
        super().__init__()
        self.conv = Conv2dCF(channels * 2, channels * 2, kernel_size, stride=stride, padding=padding)

    def __call__(self, x: mx.array, skip: mx.array | None = None) -> mx.array:
        if skip is not None:
            x = x + skip
        x = mx.concatenate([x, x], axis=1)  # torch's x.repeat(1, 2, 1, 1)
        x = self.conv(x)
        return nn.glu(x, axis=1)  # channels-first: GLU splits the channel axis directly


class SDlayer(nn.Module):
    """Sparse down-sample: per-band Conv2d along the frequency axis.

    Band convs are stored as individually named attributes (`band_0`,
    `band_1`, `band_2`), not a plain Python list: MLX's module-tree walker
    registers submodules held in *any* attribute, including list-valued ones,
    so keeping both a list and named attributes pointing at the same modules
    would register every weight under two different key paths and complicate
    the conversion audit for no reason.
    """

    def __init__(self, channels_in, channels_out, band_configs):
        super().__init__()
        self.num_bands = len(band_configs)
        self.strides = [config["stride"] for config in band_configs.values()]
        self.kernels = [config["kernel"] for config in band_configs.values()]
        for idx, config in enumerate(band_configs.values()):
            conv = Conv2dCF(channels_in, channels_out, (config["kernel"], 1), stride=(config["stride"], 1))
            setattr(self, f"band_{idx}", conv)
        self.SR_low = band_configs["low"]["SR"]
        self.SR_mid = band_configs["mid"]["SR"]

    def __call__(self, x: mx.array):
        _B, _C, Fr, _T = x.shape
        splits = [
            (0, math.ceil(Fr * self.SR_low)),
            (math.ceil(Fr * self.SR_low), math.ceil(Fr * (self.SR_low + self.SR_mid))),
            (math.ceil(Fr * (self.SR_low + self.SR_mid)), Fr),
        ]
        outputs = []
        original_lengths = []
        for idx, (start, end) in enumerate(splits):
            conv = getattr(self, f"band_{idx}")
            stride, kernel = self.strides[idx], self.kernels[idx]
            extracted = x[:, :, start:end, :]
            original_lengths.append(end - start)
            current_length = extracted.shape[2]
            if stride == 1:
                total_padding = kernel - stride
            else:
                total_padding = (stride - current_length % stride) % stride
            pad_left = total_padding // 2
            pad_right = total_padding - pad_left
            padded = mx.pad(extracted, [(0, 0), (0, 0), (pad_left, pad_right), (0, 0)])
            outputs.append(conv(padded))
        return outputs, original_lengths


class SUlayer(nn.Module):
    """Sparse up-sample: per-band ConvTranspose2d along the frequency axis."""

    def __init__(self, channels_in, channels_out, band_configs):
        super().__init__()
        self.num_bands = len(band_configs)
        for idx, config in enumerate(band_configs.values()):
            convtr = ConvTranspose2dCF(channels_in, channels_out, (config["kernel"], 1), stride=(config["stride"], 1))
            setattr(self, f"band_{idx}", convtr)

    def __call__(self, x: mx.array, lengths, origin_lengths):
        splits = [
            (0, lengths[0]),
            (lengths[0], lengths[0] + lengths[1]),
            (lengths[0] + lengths[1], None),
        ]
        outputs = []
        for idx, (start, end) in enumerate(splits):
            convtr = getattr(self, f"band_{idx}")
            segment = x[:, :, start:end, :] if end is not None else x[:, :, start:, :]
            out = convtr(segment)
            current_Fr_length = out.shape[2]
            dist = abs(origin_lengths[idx] - current_Fr_length) // 2
            outputs.append(out[:, :, dist:dist + origin_lengths[idx], :])
        return mx.concatenate(outputs, axis=2)


class SDblock(nn.Module):
    def __init__(self, channels_in, channels_out, band_configs, conv_config, depths=(3, 2, 1), kernel_size=3):
        super().__init__()
        self.SDlayer = SDlayer(channels_in, channels_out, band_configs)
        self.num_conv_modules = len(depths)
        for idx, depth in enumerate(depths):
            setattr(self, f"conv_module_{idx}", ConvolutionModule(channels_out, depth, **conv_config))
        self.globalconv = Conv2dCF(channels_out, channels_out, kernel_size, stride=1, padding=(kernel_size - 1) // 2)

    def __call__(self, x: mx.array):
        bands, original_lengths = self.SDlayer(x)
        processed = []
        for idx, band in enumerate(bands):
            conv = getattr(self, f"conv_module_{idx}")
            b, c, fr, t = band.shape
            merged = mx.reshape(mx.transpose(band, (0, 2, 1, 3)), (b * fr, c, t))
            merged = nn.gelu(conv(merged))  # conv (ConvolutionModule) returns channels-first; gelu is elementwise
            merged = mx.transpose(mx.reshape(merged, (b, fr, c, t)), (0, 2, 1, 3))
            processed.append(merged)
        lengths = [band.shape[2] for band in processed]
        full_band = mx.concatenate(processed, axis=2)
        skip = full_band
        output = self.globalconv(full_band)
        return output, skip, lengths, original_lengths


# --------------------------------------------------------------------------- #
# Trunk STFT / ISTFT (mirrors `SCNet.forward`'s pre/post-STFT reshape exactly)
# --------------------------------------------------------------------------- #


class TrunkSTFT:
    """Whole-mixture STFT/ISTFT via `mlx_spectro`, matching `torch.stft`/
    `torch.istft(**self.stft_config)` bit-for-bit-shape output for this
    package's exact call convention (verified against torch directly: spec
    max-abs diff ~2e-7 to 2e-5 depending on `normalized`, round-trip diff
    ~7e-7, both well inside this port's tolerance).

    Holds no weights (`window_fn` selects a computed window, "rect" or
    "hann"; nothing here is a checkpoint parameter), so this is a plain
    object, not an `nn.Module`.
    """

    def __init__(self, *, n_fft, hop_length, win_length, normalized, window_fn="rect"):
        self.n_fft = n_fft
        self.hop_length = hop_length
        self.win_length = win_length
        self._transform = get_transform_mlx(
            n_fft=n_fft, hop_length=hop_length, win_length=win_length,
            window_fn=window_fn, window=None, periodic=True, center=True,
            normalized=normalized,
        )

    def stft(self, x: mx.array) -> mx.array:
        """x: (N, L) real -> (N, F, T) complex64, matching
        `torch.stft(..., return_complex=True)`'s (F, T) axis order."""
        with exact_zero_safe_rfft():
            spec = self._transform.stft(x)
            mx.eval(spec)
        return spec

    def istft(self, spec: mx.array) -> mx.array:
        """spec: (N, F, T) complex64 -> (N, L) real, matching
        `torch.istft(**stft_config)` called with no explicit `length` (its
        implicit reconstruction length is `hop_length * (T - 1)`; requested
        explicitly here rather than trusted to match by omission on both
        sides)."""
        n_frames = spec.shape[-1]
        length = self.hop_length * (n_frames - 1)
        return self._transform.istft(spec, length=length)


def stft_encode(stft: TrunkSTFT, audio_channels: int, x: mx.array):
    """Port of `SCNet.forward`'s STFT prelude. x: (B, C, L) -> (spec_cf, padding)
    where spec_cf is (B, 2*C, F, T) real, matching the Torch layout exactly
    (channel axis interleaves [ch0_real, ch0_imag, ch1_real, ch1_imag, ...])."""
    length = x.shape[-1]
    padding = stft.hop_length - length % stft.hop_length
    if (length + padding) // stft.hop_length % 2 == 0:
        padding += stft.hop_length
    x = mx.pad(x, [(0, 0), (0, 0), (0, padding)])

    flat = mx.reshape(x, (-1, x.shape[-1]))
    original_shape0 = flat.shape[0]  # B*C going into stft, matching torch's x.shape[0] at that point
    spec = stft.stft(flat)  # (B*C, F, T) complex
    real_imag = mx.stack([spec.real, spec.imag], axis=-1)  # (B*C, F, T, 2), matches view_as_real
    original_shape1, original_shape2 = real_imag.shape[1], real_imag.shape[2]  # F, T (torch's x.shape[1], x.shape[2])
    real_imag = mx.transpose(real_imag, (0, 3, 1, 2))  # (B*C, 2, F, T)
    spec_cf = mx.reshape(
        real_imag,
        (original_shape0 // audio_channels, real_imag.shape[1] * audio_channels, original_shape1, original_shape2),
    )
    return spec_cf, padding


def reshape_to_realimag(y: mx.array, B: int, n: int, Fr: int, T: int) -> mx.array:
    """Port of the reshape shared by every family's output tail:
    `x.view(B, n, -1, Fr, T).reshape(-1, 2, Fr, T).permute(0, 2, 3, 1)`.
    Used on the decoder output directly (family `"scnet"`/`"scnet_tran"`) or
    on both the mixture and the mask separately (family `"scnet_masked"`,
    which multiplies the two as complex spectra before a single ISTFT)."""
    y = mx.reshape(y, (B, n, -1, Fr, T))
    y = mx.reshape(y, (-1, 2, Fr, T))
    return mx.transpose(y, (0, 2, 3, 1))  # (-1, Fr, T, 2)


def realimag_to_complex(x: mx.array) -> mx.array:
    """(..., 2) real/imag pair -> complex64, matching `torch.view_as_complex`."""
    return x[..., 0].astype(mx.complex64) + 1j * x[..., 1].astype(mx.complex64)


def stft_decode(stft: TrunkSTFT, x: mx.array) -> mx.array:
    """x: (..., F, T, 2) real/imag pair (see `reshape_to_realimag`) -> (..., L)
    real, the mirror of `stft_encode`'s tail (`torch.view_as_complex` +
    `torch.istft`)."""
    return stft.istft(realimag_to_complex(x))


# --------------------------------------------------------------------------- #
# Dual-path RNN separation net (family="scnet" and "scnet_masked")
# --------------------------------------------------------------------------- #


class BiLSTM(nn.Module):
    """`torch.nn.LSTM(..., bidirectional=True, batch_first=True)`, ported as
    two unidirectional `mlx.nn.LSTM` cells run forward and reversed-then-
    reversed-back, concatenated `[forward, backward]` on the last axis --
    matching torch's bidirectional output ordering. `mlx.nn.LSTM`'s internal
    gate order (i, f, g, o) already matches torch's packed `weight_ih`/
    `weight_hh` gate order, so no gate reordering is needed in `convert.py`,
    only a weight-layout transpose and folding torch's two biases into one
    (`bias = bias_ih + bias_hh`, mathematically identical since both act as
    a single additive term before the gate nonlinearities).
    """

    def __init__(self, input_size: int, hidden_size: int):
        super().__init__()
        self.forward_cell = nn.LSTM(input_size, hidden_size)
        self.backward_cell = nn.LSTM(input_size, hidden_size)

    def __call__(self, x: mx.array) -> mx.array:
        # x: (B, L, D)
        fwd_h, _ = self.forward_cell(x)
        bwd_h, _ = self.backward_cell(x[:, ::-1, :])
        bwd_h = bwd_h[:, ::-1, :]
        return mx.concatenate([fwd_h, bwd_h], axis=-1)


class DualPathRNN(nn.Module):
    """Ported line-for-line from Torch's `DualPathRNN`. The transpose/reshape
    sequence is preserved exactly (see class docstring in `_model/separation.py`
    for what each step means); only the op names change (`mx.transpose`/
    `mx.reshape` instead of `.transpose()`/`.contiguous().view()` -- MLX has no
    contiguity precondition for reshape, so `.contiguous()` calls are simply
    dropped)."""

    def __init__(self, d_model: int, expand: int, bidirectional: bool = True):
        super().__init__()
        self.d_model = d_model
        self.hidden_size = d_model * expand
        self.norm_0 = GroupNormCF(1, d_model)
        self.norm_1 = GroupNormCF(1, d_model)
        self.lstm_0 = BiLSTM(d_model, self.hidden_size)
        self.lstm_1 = BiLSTM(d_model, self.hidden_size)
        self.linear_0 = nn.Linear(self.hidden_size * 2, d_model)
        self.linear_1 = nn.Linear(self.hidden_size * 2, d_model)

    def __call__(self, x: mx.array) -> mx.array:
        B, C, F, T = x.shape

        original = x
        y = self.norm_0(x)
        y = mx.transpose(y, (0, 3, 2, 1))  # (B,C,F,T) -> (B,T,F,C), torch .transpose(1,3)
        y = mx.reshape(y, (B * T, F, C))
        y = self.lstm_0(y)
        y = self.linear_0(y)
        y = mx.reshape(y, (B, T, F, C))
        y = mx.transpose(y, (0, 3, 2, 1))  # (B,T,F,C) -> (B,C,F,T)
        x = y + original

        original = x
        y = self.norm_1(x)
        y = mx.transpose(y, (0, 2, 1, 3))  # (B,C,F,T) -> (B,F,C,T), torch .transpose(1,2)
        y = mx.reshape(y, (B * F, C, T))
        y = mx.transpose(y, (0, 2, 1))  # -> (B*F, T, C)
        y = self.lstm_1(y)
        y = self.linear_1(y)
        y = mx.transpose(y, (0, 2, 1))  # (B*F,T,C) -> (B*F,C,T)
        y = mx.reshape(y, (B, F, C, T))
        y = mx.transpose(y, (0, 2, 1, 3))  # -> (B,C,F,T)
        x = y + original
        return x


class FeatureConversion(nn.Module):
    """rfft/irfft along the time axis, alternating between dual-path layers.

    Only the forward direction is guarded by `exact_zero_safe_rfft` -- the
    documented Metal artifact is specific to `mx.fft.rfft`'s real-to-complex
    kernel; `irfft` (complex-to-real) was not measured to have the same
    defect and guarding it without evidence would be exactly the kind of
    unmeasured "declared" mitigation this port avoids elsewhere.
    """

    def __init__(self, channels: int, inverse: bool):
        super().__init__()
        self.inverse = inverse
        self.channels = channels

    def __call__(self, x: mx.array) -> mx.array:
        if self.inverse:
            half = self.channels // 2
            real = x[:, :half, :, :]
            imag = x[:, half:, :, :]
            spec = real.astype(mx.complex64) + 1j * imag.astype(mx.complex64)
            return mx.fft.irfft(spec, axis=3, norm="ortho")
        with exact_zero_safe_rfft():
            spec = mx.fft.rfft(x, axis=3, norm="ortho")
            mx.eval(spec)
        return mx.concatenate([spec.real, spec.imag], axis=1)


class SeparationNet(nn.Module):
    def __init__(self, channels: int, expand: int = 1, num_layers: int = 6):
        super().__init__()
        self.num_layers = num_layers
        for idx in range(num_layers):
            d_model = channels * (2 if idx % 2 == 1 else 1)
            setattr(self, f"dp_{idx}", DualPathRNN(d_model, expand))
            setattr(self, f"feature_conversion_{idx}", FeatureConversion(channels * 2, inverse=(idx % 2 != 0)))

    def __call__(self, x: mx.array) -> mx.array:
        for idx in range(self.num_layers):
            x = getattr(self, f"dp_{idx}")(x)
            x = getattr(self, f"feature_conversion_{idx}")(x)
        return x


def _band_configs(band_SR, band_stride, band_kernel) -> dict:
    keys = ("low", "mid", "high")
    return {
        keys[i]: {"SR": band_SR[i], "stride": band_stride[i], "kernel": band_kernel[i]}
        for i in range(len(keys))
    }


class SCNetMLX(nn.Module):
    """Port of `scnet_infer._model.scnet.SCNet` (family `"scnet"`, the default
    registry model). LSTM dual-path separation net, no rotary embeddings, no
    mask head -- the mixture's magnitude/phase are reconstructed directly by
    the decoder + ISTFT.

    `decoder_fusion_i`/`decoder_su_i` are stored in the *same* order Torch's
    `nn.ModuleList.insert(0, ...)` produces (index `i` pairs with encoder
    stage `num_stages - 1 - i`), so a plain end-to-end index loop plus
    `list.pop()` on the skip-connection stacks reproduces Torch's `deque`
    push/pop order exactly.
    """

    def __init__(
        self,
        sources=("drums", "bass", "other", "vocals"),
        audio_channels=2,
        dims=(4, 32, 64, 128),
        nfft=4096,
        hop_size=1024,
        win_size=4096,
        normalized=True,
        band_SR=(0.175, 0.392, 0.433),
        band_stride=(1, 4, 16),
        band_kernel=(3, 4, 16),
        conv_depths=(3, 2, 1),
        compress=4,
        conv_kernel=3,
        num_dplayer=6,
        expand=1,
    ):
        super().__init__()
        self.sources = list(sources)
        self.audio_channels = audio_channels
        self.dims = list(dims)
        band_configs = _band_configs(band_SR, band_stride, band_kernel)
        conv_config = {"compress": compress, "kernel": conv_kernel}
        self.stft = TrunkSTFT(
            n_fft=nfft, hop_length=hop_size, win_length=win_size,
            normalized=normalized, window_fn="rect",
        )

        self.num_stages = len(dims) - 1
        decoders = []
        for index in range(self.num_stages):
            setattr(
                self, f"encoder_{index}",
                SDblock(dims[index], dims[index + 1], band_configs, conv_config, depths=conv_depths),
            )
            fusion = FusionLayer(dims[index + 1])
            channels_out = dims[index] if index != 0 else dims[index] * len(self.sources)
            su = SUlayer(dims[index + 1], channels_out, band_configs)
            decoders.insert(0, (fusion, su))
        for idx, (fusion, su) in enumerate(decoders):
            setattr(self, f"decoder_fusion_{idx}", fusion)
            setattr(self, f"decoder_su_{idx}", su)

        self.separation_net = SeparationNet(channels=dims[-1], expand=expand, num_layers=num_dplayer)

    def __call__(self, x: mx.array) -> mx.array:
        B = x.shape[0]
        y, padding = stft_encode(self.stft, self.audio_channels, x)
        Fr, T = y.shape[2], y.shape[3]

        save_skip, save_lengths, save_original_lengths = [], [], []
        for idx in range(self.num_stages):
            y, skip, lengths, original_lengths = getattr(self, f"encoder_{idx}")(y)
            save_skip.append(skip)
            save_lengths.append(lengths)
            save_original_lengths.append(original_lengths)

        y = self.separation_net(y)

        for idx in range(self.num_stages):
            fusion = getattr(self, f"decoder_fusion_{idx}")
            su = getattr(self, f"decoder_su_{idx}")
            y = fusion(y, save_skip.pop())
            y = su(y, save_lengths.pop(), save_original_lengths.pop())

        n = self.dims[0]
        pairs = reshape_to_realimag(y, B, n, Fr, T)
        waveform = stft_decode(self.stft, pairs)
        waveform = mx.reshape(waveform, (B, len(self.sources), self.audio_channels, -1))
        if padding:
            waveform = waveform[:, :, :, :-padding]
        return waveform


class SCNetMaskedMLX(nn.Module):
    """Port of `scnet_infer._model.scnet_masked.SCNet` (family
    `"scnet_masked"`). Same LSTM dual-path trunk as `SCNetMLX`, plus a
    learned frequency positional embedding added before the encoder and a
    small mask head applied after the decoder; the *mixture* spectrogram
    (not the model's raw decoder output) is complex-multiplied by that mask
    before a single ISTFT, and the STFT/ISTFT window is Hann rather than
    rectangular (`scnet_masked.py` builds `torch.hann_window(nfft,
    periodic=True)` explicitly, unlike `scnet.py`/`scnet_tran.py`, which pass
    no window to `torch.stft` at all)."""

    def __init__(
        self,
        sources=("drums", "bass", "other", "vocals"),
        audio_channels=2,
        dims=(4, 32, 64, 128),
        nfft=4096,
        hop_size=1024,
        win_size=4096,
        normalized=True,
        band_SR=(0.175, 0.392, 0.433),
        band_stride=(1, 4, 16),
        band_kernel=(3, 4, 16),
        conv_depths=(3, 2, 1),
        compress=4,
        conv_kernel=3,
        num_dplayer=6,
        expand=1,
    ):
        super().__init__()
        self.sources = list(sources)
        self.audio_channels = audio_channels
        self.dims = list(dims)
        band_configs = _band_configs(band_SR, band_stride, band_kernel)
        conv_config = {"compress": compress, "kernel": conv_kernel}
        self.stft = TrunkSTFT(
            n_fft=nfft, hop_length=hop_size, win_length=win_size,
            normalized=normalized, window_fn="hann",
        )

        self.embed_dim = dims[0]
        self.max_f = nfft // 2 + 1
        self.pos_embed_f = mx.zeros((1, self.embed_dim, self.max_f, 1))

        self.num_stages = len(dims) - 1
        decoders = []
        for index in range(self.num_stages):
            setattr(
                self, f"encoder_{index}",
                SDblock(dims[index], dims[index + 1], band_configs, conv_config, depths=conv_depths),
            )
            fusion = FusionLayer(dims[index + 1])
            channels_out = dims[index] if index != 0 else dims[index] * len(self.sources)
            su = SUlayer(dims[index + 1], channels_out, band_configs)
            decoders.insert(0, (fusion, su))
        for idx, (fusion, su) in enumerate(decoders):
            setattr(self, f"decoder_fusion_{idx}", fusion)
            setattr(self, f"decoder_su_{idx}", su)

        self.separation_net = SeparationNet(channels=dims[-1], expand=expand, num_layers=num_dplayer)

        mask_channels = 4 * len(self.sources)
        self.mask_conv1 = Conv2dCF(mask_channels, 64, 3, stride=1, padding=1)
        self.mask_conv2 = Conv2dCF(64, mask_channels, 1, stride=1, padding=0)

    def __call__(self, x: mx.array) -> mx.array:
        B = x.shape[0]
        y, padding = stft_encode(self.stft, self.audio_channels, x)
        _, C, Fr, T = y.shape
        assert C == self.embed_dim, f"STFT channel dim {C} != embed_dim {self.embed_dim}"

        mixture = mx.concatenate([y] * len(self.sources), axis=1)  # torch's x.repeat(1, len(sources), 1, 1)

        if Fr > self.max_f:
            repeats = -(-Fr // self.max_f)  # ceil division
            pos_f = mx.concatenate([self.pos_embed_f] * repeats, axis=2)[:, :, :Fr, :]
        else:
            pos_f = self.pos_embed_f[:, :, :Fr, :]
        y = y + pos_f

        save_skip, save_lengths, save_original_lengths = [], [], []
        for idx in range(self.num_stages):
            y, skip, lengths, original_lengths = getattr(self, f"encoder_{idx}")(y)
            save_skip.append(skip)
            save_lengths.append(lengths)
            save_original_lengths.append(original_lengths)

        y = self.separation_net(y)

        for idx in range(self.num_stages):
            fusion = getattr(self, f"decoder_fusion_{idx}")
            su = getattr(self, f"decoder_su_{idx}")
            y = fusion(y, save_skip.pop())
            y = su(y, save_lengths.pop(), save_original_lengths.pop())

        mask = self.mask_conv1(y)
        mask = nn.gelu(mask)
        mask = self.mask_conv2(mask)
        mask = mx.tanh(mask)

        n = self.dims[0]
        mixture_complex = realimag_to_complex(reshape_to_realimag(mixture, B, n, Fr, T))
        mask_complex = realimag_to_complex(reshape_to_realimag(mask, B, n, Fr, T))
        spec = mixture_complex * mask_complex

        waveform = self.stft.istft(spec)
        waveform = mx.reshape(waveform, (B, len(self.sources), self.audio_channels, -1))
        if padding:
            waveform = waveform[:, :, :, :-padding]
        return waveform


# --------------------------------------------------------------------------- #
# Transformer dual-path separation net (family="scnet_tran")
# --------------------------------------------------------------------------- #


class RMSNorm(nn.Module):
    """Ported from Torch's `RMSNorm`, which despite the name implements
    `F.normalize(x, dim=-1) * sqrt(dim) * gamma` (L2-normalize, not the
    mean-square-based formula `mx.fast.rms_norm` implements) -- written out
    explicitly rather than reused, since the two differ in general and this
    port needs `F.normalize`'s exact eps convention (`x / max(||x||_2,
    eps)`, eps=1e-12) rather than trusting a same-named MLX primitive to
    match. The checkpoint's own parameter is named `gamma`; kept as
    `self.weight` here for consistency with the rest of this file's naming
    (`convert.py` renames it on load, same as the sibling Roformer packages'
    `gamma` -> `weight` convention)."""

    def __init__(self, dim: int):
        super().__init__()
        self.scale = dim ** 0.5
        self.weight = mx.ones((dim,))

    def __call__(self, x: mx.array) -> mx.array:
        norm = mx.sqrt(mx.sum(x * x, axis=-1, keepdims=True))
        norm = mx.maximum(norm, 1e-12)
        return (x / norm) * self.scale * self.weight


class FeedForward(nn.Module):
    def __init__(self, dim: int, mult: int = 4):
        super().__init__()
        dim_inner = int(dim * mult)
        self.norm = RMSNorm(dim)
        self.linear1 = nn.Linear(dim, dim_inner)
        self.linear2 = nn.Linear(dim_inner, dim)

    def __call__(self, x: mx.array) -> mx.array:
        x = self.norm(x)
        x = self.linear1(x)
        x = nn.gelu(x)
        return self.linear2(x)


class Attention(nn.Module):
    """Ported from Torch's `Attention` (gated multi-head attention with an
    optional rotary embedding). `tran_flash_attn=false` for every registry
    checkpoint using this family (see `config/checkpoints.toml`), so this
    always takes Torch's non-flash path: `softmax(q @ k^T * scale) @ v` with
    `scale = dim_head ** -0.5` -- Torch's `Attention.scale` attribute is
    itself dead code (never read in its own `forward`; the real scale comes
    from `Attend`'s default, which evaluates to the same number), so nothing
    is lost by computing it directly here.

    Rotary frequencies are always loaded from the checkpoint, never derived
    from a theta formula -- see this module's file-top docstring for why."""

    def __init__(self, dim: int, heads: int = 8, dim_head: int = 64, rotary: bool = True):
        super().__init__()
        self.heads = heads
        self.dim_head = dim_head
        self.scale = dim_head ** -0.5
        dim_inner = heads * dim_head
        self.rotary = rotary
        self.norm = RMSNorm(dim)
        self.to_qkv = nn.Linear(dim, dim_inner * 3, bias=False)
        self.to_gates = nn.Linear(dim, heads)
        self.to_out = nn.Linear(dim_inner, dim, bias=False)
        if rotary:
            self.rope_freqs = mx.zeros((dim_head // 2,))

    def __call__(self, x: mx.array) -> mx.array:
        B, N, _ = x.shape
        y = self.norm(x)
        qkv = self.to_qkv(y)
        qkv = mx.reshape(qkv, (B, N, 3, self.heads, self.dim_head))
        qkv = mx.transpose(qkv, (2, 0, 3, 1, 4))  # (3, B, heads, N, dim_head)
        q, k, v = qkv[0], qkv[1], qkv[2]

        if self.rotary:
            # mx.fast.rope wants the reciprocal of what rotary_embedding_torch
            # stores (confirmed empirically -- see module docstring).
            freqs = mx.reciprocal(self.rope_freqs)
            q = mx.fast.rope(q, dims=self.dim_head, traditional=True, base=None, scale=1.0, offset=0, freqs=freqs)
            k = mx.fast.rope(k, dims=self.dim_head, traditional=True, base=None, scale=1.0, offset=0, freqs=freqs)

        scores = (q * self.scale) @ mx.transpose(k, (0, 1, 3, 2))
        attn = mx.softmax(scores, axis=-1)
        out = attn @ v  # (B, heads, N, dim_head)

        gates = self.to_gates(y)  # (B, N, heads)
        gates = mx.transpose(gates, (0, 2, 1))[..., None]  # (B, heads, N, 1)
        out = out * mx.sigmoid(gates)

        out = mx.transpose(out, (0, 2, 1, 3))  # (B, N, heads, dim_head)
        out = mx.reshape(out, (B, N, self.heads * self.dim_head))
        return self.to_out(out)


class Transformer(nn.Module):
    def __init__(self, dim: int, depth: int, heads: int, dim_head: int, rotary: bool = True):
        super().__init__()
        self.depth = depth
        for idx in range(depth):
            setattr(self, f"attn_{idx}", Attention(dim, heads=heads, dim_head=dim_head, rotary=rotary))
            setattr(self, f"ff_{idx}", FeedForward(dim, mult=4))
        self.norm = RMSNorm(dim)

    def __call__(self, x: mx.array) -> mx.array:
        for idx in range(self.depth):
            x = getattr(self, f"attn_{idx}")(x) + x
            x = getattr(self, f"ff_{idx}")(x) + x
        return self.norm(x)


class DualPathTran(nn.Module):
    """Ported line-for-line from Torch's `DualPathTran` -- identical
    transpose/reshape choreography to `DualPathRNN` (see that class's
    docstring); only the per-axis processing block changes (`Transformer`
    instead of `BiLSTM` + `Linear`)."""

    def __init__(self, d_model: int, tran_params: dict):
        super().__init__()
        self.norm_0 = GroupNormCF(1, d_model)
        self.norm_1 = GroupNormCF(1, d_model)
        transformer_kwargs = {
            "dim": d_model, "depth": tran_params["depth"],
            "heads": tran_params["heads"], "dim_head": tran_params["dim_head"],
        }
        self.freq_layer = Transformer(**transformer_kwargs)
        self.time_layer = Transformer(**transformer_kwargs)

    def __call__(self, x: mx.array) -> mx.array:
        B, C, F, T = x.shape

        original = x
        y = self.norm_0(x)
        y = mx.transpose(y, (0, 3, 2, 1))  # (B,C,F,T) -> (B,T,F,C)
        y = mx.reshape(y, (B * T, F, C))
        y = self.freq_layer(y)
        y = mx.reshape(y, (B, T, F, C))
        y = mx.transpose(y, (0, 3, 2, 1))
        x = y + original

        original = x
        y = self.norm_1(x)
        y = mx.transpose(y, (0, 2, 1, 3))  # (B,C,F,T) -> (B,F,C,T)
        y = mx.reshape(y, (B * F, C, T))
        y = mx.transpose(y, (0, 2, 1))  # -> (B*F, T, C)
        y = self.time_layer(y)
        y = mx.transpose(y, (0, 2, 1))  # -> (B*F, C, T)
        y = mx.reshape(y, (B, F, C, T))
        y = mx.transpose(y, (0, 2, 1, 3))
        x = y + original
        return x


class SeparationNetTran(nn.Module):
    def __init__(self, channels: int, num_layers: int, tran_params: dict):
        super().__init__()
        self.num_layers = num_layers
        for idx in range(num_layers):
            d_model = channels * (2 if idx % 2 == 1 else 1)
            setattr(self, f"dp_{idx}", DualPathTran(d_model, tran_params))
            setattr(self, f"feature_conversion_{idx}", FeatureConversion(channels * 2, inverse=(idx % 2 != 0)))

    def __call__(self, x: mx.array) -> mx.array:
        for idx in range(self.num_layers):
            x = getattr(self, f"dp_{idx}")(x)
            x = getattr(self, f"feature_conversion_{idx}")(x)
        return x


class SCNetTranMLX(nn.Module):
    """Port of `scnet_infer._model.scnet_tran.SCNet_Tran` (family
    `"scnet_tran"`). Same encoder/decoder trunk as `SCNetMLX`, rectangular
    STFT window (no window key in `stft_config`, same as `"scnet"`), and a
    Transformer dual-path separation net instead of LSTM.

    `first_conv` is constructed but never called -- upstream's own
    `SCNet_Tran.forward` never invokes `self.first_conv` either (verified
    against the real Torch source: it is assigned in `__init__` and never
    referenced again). It is kept here, unused, purely so the checkpoint's
    `first_conv.weight` tensor has a home; dropping it would make the
    auditing weight loader correctly refuse to load this checkpoint at all
    for a parameter upstream itself never uses.
    """

    def __init__(
        self,
        sources=("drums", "bass", "other", "vocals"),
        audio_channels=2,
        dims=(4, 32, 64, 128),
        nfft=4096,
        hop_size=1024,
        win_size=4096,
        normalized=True,
        band_SR=(0.175, 0.392, 0.433),
        band_stride=(1, 4, 16),
        band_kernel=(3, 4, 16),
        conv_depths=(3, 2, 1),
        compress=4,
        conv_kernel=3,
        num_dplayer=6,
        expand=1,
        tran_rotary_embedding_dim=64,
        tran_depth=1,
        tran_heads=8,
        tran_dim_head=64,
        tran_attn_dropout=0.0,
        tran_ff_dropout=0.0,
        tran_flash_attn=False,
    ):
        super().__init__()
        self.sources = list(sources)
        self.audio_channels = audio_channels
        self.dims = list(dims)
        band_configs = _band_configs(band_SR, band_stride, band_kernel)
        conv_config = {"compress": compress, "kernel": conv_kernel}
        self.stft = TrunkSTFT(
            n_fft=nfft, hop_length=hop_size, win_length=win_size,
            normalized=normalized, window_fn="rect",
        )

        self.first_conv = Conv2dCF(dims[0], dims[0], 1, stride=1, padding=0, bias=False)  # unused; see docstring

        self.num_stages = len(dims) - 1
        decoders = []
        for index in range(self.num_stages):
            setattr(
                self, f"encoder_{index}",
                SDblock(dims[index], dims[index + 1], band_configs, conv_config, depths=conv_depths),
            )
            fusion = FusionLayer(dims[index + 1])
            channels_out = dims[index] if index != 0 else dims[index] * len(self.sources)
            su = SUlayer(dims[index + 1], channels_out, band_configs)
            decoders.insert(0, (fusion, su))
        for idx, (fusion, su) in enumerate(decoders):
            setattr(self, f"decoder_fusion_{idx}", fusion)
            setattr(self, f"decoder_su_{idx}", su)

        tran_params = {
            "rotary_embedding_dim": tran_rotary_embedding_dim,
            "depth": tran_depth, "heads": tran_heads, "dim_head": tran_dim_head,
        }
        self.separation_net = SeparationNetTran(channels=dims[-1], num_layers=num_dplayer, tran_params=tran_params)

    def __call__(self, x: mx.array) -> mx.array:
        B = x.shape[0]
        y, padding = stft_encode(self.stft, self.audio_channels, x)
        Fr, T = y.shape[2], y.shape[3]

        save_skip, save_lengths, save_original_lengths = [], [], []
        for idx in range(self.num_stages):
            y, skip, lengths, original_lengths = getattr(self, f"encoder_{idx}")(y)
            save_skip.append(skip)
            save_lengths.append(lengths)
            save_original_lengths.append(original_lengths)

        y = self.separation_net(y)

        for idx in range(self.num_stages):
            fusion = getattr(self, f"decoder_fusion_{idx}")
            su = getattr(self, f"decoder_su_{idx}")
            y = fusion(y, save_skip.pop())
            y = su(y, save_lengths.pop(), save_original_lengths.pop())

        n = self.dims[0]
        pairs = reshape_to_realimag(y, B, n, Fr, T)
        waveform = stft_decode(self.stft, pairs)
        waveform = mx.reshape(waveform, (B, len(self.sources), self.audio_channels, -1))
        if padding:
            waveform = waveform[:, :, :, :-padding]
        return waveform
