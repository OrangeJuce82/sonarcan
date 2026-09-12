"""Export the pinned LV-Chordia/Librosa hybrid-CQT analysis kernels.

This is a development-only generator. The application reads the resulting
SACCQT01 file and executes the STFT projections with rustfft; Librosa is never
part of the shipped runtime.
"""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

import librosa
import numpy as np
from librosa import filters
from librosa.core import constantq


MAGIC = b"SACCQT01"
SAMPLE_RATE = 22_050
HOP_LENGTH = 512
BINS_PER_OCTAVE = 36
TOTAL_BINS = 288
FIRST_MODEL_BIN = 18
MODEL_BINS = 252


def append_values(payload: bytearray, values: np.ndarray) -> tuple[int, int]:
    flat = np.asarray(values, dtype="<f4").reshape(-1)
    offset = len(payload) // 4
    payload.extend(flat.tobytes())
    return offset, flat.size


def export(destination: Path, tuning: float) -> None:
    private_filter = constantq.__dict__["__vqt_filter_fft"]
    fmin = librosa.note_to_hz("F#0")
    fmin *= 2.0 ** (tuning / BINS_PER_OCTAVE)
    frequencies = librosa.cqt_frequencies(
        TOTAL_BINS, fmin=fmin, bins_per_octave=BINS_PER_OCTAVE
    )
    alpha = filters._relative_bandwidth(freqs=frequencies)
    lengths, _ = filters.wavelet_lengths(
        freqs=frequencies,
        sr=SAMPLE_RATE,
        filter_scale=1,
        window="hann",
        alpha=alpha,
    )
    pseudo_mask = 2.0 ** np.ceil(np.log2(lengths)) < 2 * HOP_LENGTH
    first_pseudo = int(np.nonzero(pseudo_mask)[0][0])

    payload = bytearray()
    metadata: dict[str, object] = {
        "sampleRate": SAMPLE_RATE,
        "hopLength": HOP_LENGTH,
        "firstModelBin": FIRST_MODEL_BIN,
        "modelBins": MODEL_BINS,
        "tuning": tuning,
        "stages": [],
    }

    # Hybrid CQT's high-frequency pseudo branch projects STFT magnitudes with
    # the magnitude of the complex basis.
    pseudo_frequencies = frequencies[first_pseudo:]
    pseudo_alpha = alpha[first_pseudo:]
    pseudo_basis, pseudo_fft, _ = private_filter(
        SAMPLE_RATE,
        pseudo_frequencies,
        1,
        1,
        0.01,
        hop_length=HOP_LENGTH,
        window="hann",
        dtype=np.complex64,
        alpha=pseudo_alpha,
    )
    pseudo_last = min(TOTAL_BINS, FIRST_MODEL_BIN + MODEL_BINS)
    pseudo_indices = list(range(first_pseudo, pseudo_last))
    pseudo_values = np.abs(pseudo_basis[: len(pseudo_indices)].toarray()) / np.sqrt(
        pseudo_fft
    )
    offset, count = append_values(payload, pseudo_values)
    metadata["stages"].append(
        {
            "kind": "magnitude",
            "rateDivisor": 1,
            "hopLength": HOP_LENGTH,
            "fftSize": pseudo_fft,
            "bins": pseudo_indices,
            "valueOffset": offset,
            "valueCount": count,
        }
    )

    # The full branch processes high-to-low octaves, halving audio and hop at
    # each stage. Store only bins consumed by ChordNet (18..269).
    full_bins = first_pseudo
    octave_count = int(np.ceil(full_bins / BINS_PER_OCTAVE))
    filters_per_octave = min(BINS_PER_OCTAVE, full_bins)
    rate_divisor = 1
    hop = HOP_LENGTH
    for octave in range(octave_count):
        if octave == 0:
            start = full_bins - filters_per_octave
            stop = full_bins
        else:
            start = max(0, full_bins - filters_per_octave * (octave + 1))
            stop = full_bins - filters_per_octave * octave
        octave_frequencies = frequencies[start:stop]
        octave_alpha = alpha[start:stop]
        basis, fft_size, _ = private_filter(
            SAMPLE_RATE / rate_divisor,
            octave_frequencies,
            1,
            1,
            0.01,
            window="hann",
            dtype=np.complex64,
            alpha=octave_alpha,
        )
        keep_start = max(start, FIRST_MODEL_BIN)
        if keep_start < stop:
            first_row = keep_start - start
            kept = basis[first_row:].toarray()
            scale = np.sqrt(rate_divisor) / np.sqrt(lengths[keep_start:stop])
            kept = kept * scale[:, np.newaxis]
            real_offset, real_count = append_values(payload, kept.real)
            imag_offset, imag_count = append_values(payload, kept.imag)
            metadata["stages"].append(
                {
                    "kind": "complex",
                    "rateDivisor": rate_divisor,
                    "hopLength": hop,
                    "fftSize": fft_size,
                    "bins": list(range(keep_start, stop)),
                    "realOffset": real_offset,
                    "realCount": real_count,
                    "imagOffset": imag_offset,
                    "imagCount": imag_count,
                }
            )
        if octave < octave_count - 1:
            next_maximum = frequencies[start - 1]
            current_rate = SAMPLE_RATE / rate_divisor
            if hop % 2 == 0 and next_maximum <= current_rate / 5:
                rate_divisor *= 2
                hop //= 2

    encoded = json.dumps(metadata, separators=(",", ":")).encode()
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    temporary.write_bytes(MAGIC + struct.pack("<I", len(encoded)) + encoded + payload)
    temporary.replace(destination)
    print(json.dumps({"path": str(destination), "bytes": destination.stat().st_size}))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    parser.add_argument("--tuning", type=float, default=0.0)
    parser.add_argument(
        "--bank",
        action="store_true",
        help="export tuning-00..99 for semitone offsets -0.50..+0.49",
    )
    arguments = parser.parse_args()
    if arguments.bank:
        destination = arguments.destination.resolve()
        for index in range(100):
            export(destination / f"tuning-{index:02}.saccqt", (index - 50) / 100)
        return 0
    if not -0.5 <= arguments.tuning < 0.5:
        parser.error("--tuning must be in [-0.5, 0.5)")
    export(arguments.destination.resolve(), arguments.tuning)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
