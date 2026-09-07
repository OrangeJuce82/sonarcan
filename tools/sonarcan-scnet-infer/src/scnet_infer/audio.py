"""Single audio-normalization boundary for every facade entry point.

Paths infer their sample rate through soundfile; arrays require an explicit rate.
SCNet checkpoints currently require stereo 44.1 kHz input.
Reads: soundfile and NumPy.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
import soundfile as sf


def load_audio(audio: str | Path | np.ndarray, sample_rate: int | None, expected_rate: int) -> np.ndarray:
    """Return contiguous float32 channel-first stereo audio."""

    if isinstance(audio, (str, Path)):
        data, actual_rate = sf.read(audio, dtype="float32", always_2d=True)
        array = data.T
        if sample_rate is not None and sample_rate != actual_rate:
            raise ValueError(f"sample_rate={sample_rate} disagrees with file rate {actual_rate}")
    else:
        if sample_rate is None:
            raise ValueError("sample_rate is required for array input")
        actual_rate = sample_rate
        array = np.asarray(audio, dtype=np.float32)
        if array.ndim == 1:
            array = np.stack([array, array])
        elif array.ndim == 2 and array.shape[0] in (1, 2):
            pass
        elif array.ndim == 2 and array.shape[1] in (1, 2):
            array = array.T
        else:
            raise ValueError("audio must be mono/stereo with shape (samples,), (channels, samples), or (samples, channels)")
    if actual_rate != expected_rate:
        raise ValueError(f"checkpoint requires {expected_rate} Hz audio; got {actual_rate} Hz")
    if array.shape[0] == 1:
        array = np.repeat(array, 2, axis=0)
    if array.shape[0] != 2 or array.shape[1] == 0:
        raise ValueError("SCNet requires non-empty mono or stereo audio")
    return np.ascontiguousarray(array, dtype=np.float32)
