"""Bounded librosa feature extraction for the SonArcan chord engine.

This module intentionally emits observations, not chord labels. Musical
interpretation and temporal decoding remain owned by the testable Rust engine.
"""

from __future__ import annotations

import json
import math
import struct
import sys
from pathlib import Path

FEATURE_VERSION = 3
SAMPLE_RATE = 22_050
HOP_LENGTH = 512
MAX_SEGMENTS = 4_096
MIN_SEGMENT_SECONDS = 0.55
MAX_SEGMENT_SECONDS = 4.0
STEM_MAGIC = b"SACSTM02"


def _normalized(values):
    import numpy as np

    values = np.maximum(np.asarray(values, dtype=np.float64), 0.0)
    total = float(values.sum())
    return values / total if total > np.finfo(np.float64).eps else np.zeros_like(values)


def _key(chroma):
    import numpy as np

    major = _normalized([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
    minor = _normalized([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])
    observed = _normalized(chroma)
    scores = []
    for root in range(12):
        scores.append((float(np.dot(observed, np.roll(major, root))), root, False))
        scores.append((float(np.dot(observed, np.roll(minor, root))), root, True))
    _, root, is_minor = max(scores)
    return root, is_minor


def _boundaries(chroma, duration_seconds):
    import numpy as np

    if chroma.shape[1] <= 1:
        return [0, chroma.shape[1]]
    frame_count = chroma.shape[1]
    smoothing_frames = max(3, round(0.35 * SAMPLE_RATE / HOP_LENGTH))
    smoothing_kernel = np.ones(smoothing_frames) / smoothing_frames
    left_padding = smoothing_frames // 2
    right_padding = smoothing_frames - 1 - left_padding
    smoothed = np.stack([
        np.convolve(
            np.pad(row, (left_padding, right_padding), mode="edge"),
            smoothing_kernel,
            mode="valid",
        )
        for row in chroma
    ])
    lag = max(1, round(0.45 * SAMPLE_RATE / HOP_LENGTH))
    left = smoothed[:, :-lag]
    right = smoothed[:, lag:]
    denominator = np.linalg.norm(left, axis=0) * np.linalg.norm(right, axis=0)
    similarity = np.divide((left * right).sum(axis=0), denominator, out=np.ones_like(denominator), where=denominator > 1e-12)
    novelty = np.zeros(frame_count)
    novelty_start = lag // 2
    novelty[novelty_start:novelty_start + similarity.shape[0]] = np.maximum(0.0, 1.0 - similarity)
    novelty = np.convolve(novelty, np.ones(5) / 5, mode="same")
    minimum_frames = max(1, round(MIN_SEGMENT_SECONDS * SAMPLE_RATE / HOP_LENGTH))
    maximum_frames = max(minimum_frames + 1, round(MAX_SEGMENT_SECONDS * SAMPLE_RATE / HOP_LENGTH))
    positive = novelty[novelty > 0]
    threshold = float(np.quantile(positive, 0.58)) if positive.size else 1.0
    local_radius = max(1, minimum_frames // 2)
    peaks = [
        frame for frame in range(lag, frame_count)
        if novelty[frame] >= threshold
        and novelty[frame] >= novelty[max(0, frame - local_radius):min(frame_count, frame + local_radius + 1)].max()
    ]

    result = [0]
    for boundary in [*peaks, frame_count]:
        while boundary - result[-1] > maximum_frames:
            target = result[-1] + maximum_frames
            radius = max(1, round(0.8 * SAMPLE_RATE / HOP_LENGTH))
            search_start = max(result[-1] + minimum_frames, target - radius)
            search_end = min(boundary - minimum_frames, target + radius)
            split = target if search_end <= search_start else search_start + int(np.argmax(novelty[search_start:search_end + 1]))
            result.append(split)
            if len(result) >= MAX_SEGMENTS:
                return [*result, frame_count]
        if boundary == frame_count or boundary - result[-1] >= minimum_frames:
            result.append(boundary)
        elif boundary == frame_count:
            result[-1] = boundary
    if len(result) > 2 and result[-1] - result[-2] < minimum_frames:
        result.pop(-2)
    return result


def _ambiguity(chroma):
    """Estimate diffuseness without treating normal mix overtones as uncertainty."""
    import numpy as np

    normalized = _normalized(chroma)
    dominant_energy = float(np.partition(normalized, -3)[-3:].sum())
    return float(np.clip((0.62 - dominant_energy) / 0.32, 0.0, 1.0))


def _bass_strength(chroma):
    import numpy as np

    normalized = _normalized(chroma)
    strongest = np.partition(normalized, -2)[-2:]
    return float((strongest[1] - strongest[0]) / max(strongest[1], 1e-12))


def _stem_signal(path: Path):
    """Read SonArcan's bounded stereo float PCM cache without WAV conversion."""
    import numpy as np

    with path.open("rb") as source:
        header = source.read(20)
        if len(header) != 20 or header[:8] != STEM_MAGIC:
            raise ValueError("invalid SonArcan stem header")
        sample_rate, frames = struct.unpack("<IQ", header[8:])
        expected_bytes = frames * 2 * 4
        if sample_rate < 8_000 or sample_rate > 384_000 or frames == 0:
            raise ValueError("SonArcan stem dimensions are invalid")
        if path.stat().st_size != 20 + expected_bytes:
            raise ValueError("SonArcan stem length does not match its header")
        samples = np.fromfile(source, dtype="<f4", count=frames * 2)
    if samples.size != frames * 2 or not np.isfinite(samples).all():
        raise ValueError("SonArcan stem samples are invalid")
    return samples.reshape(frames, 2).mean(axis=1), sample_rate


def _load_stem(path: Path, target_rate: int):
    import librosa
    import numpy as np

    signal, sample_rate = _stem_signal(path)
    if sample_rate != target_rate:
        signal = librosa.resample(signal, orig_sr=sample_rate, target_sr=target_rate)
    return np.asarray(signal, dtype=np.float32)


def _fused_chroma(harmonic, sample_rate):
    """Fuse detailed CQT chroma with a dynamics/timbre-stable CENS view."""
    import librosa
    import numpy as np

    high_resolution = librosa.feature.chroma_cqt(
        y=harmonic,
        sr=sample_rate,
        hop_length=HOP_LENGTH,
        n_chroma=36,
        bins_per_octave=36,
    )
    cqt_chroma = high_resolution.reshape(12, 3, -1).sum(axis=1)
    cens = librosa.feature.chroma_cens(
        y=harmonic,
        sr=sample_rate,
        hop_length=HOP_LENGTH,
        n_chroma=12,
        bins_per_octave=36,
        win_len_smooth=21,
    )
    frames = min(cqt_chroma.shape[1], cens.shape[1])
    cqt_chroma = librosa.util.normalize(cqt_chroma[:, :frames], norm=1, axis=0)
    cens = librosa.util.normalize(cens[:, :frames], norm=1, axis=0)
    return np.asarray(0.7 * cqt_chroma + 0.3 * cens, dtype=np.float64)


def extract(path: Path, stem_paths: tuple[Path, Path, Path, Path] | None = None) -> dict[str, object]:
    import librosa
    import numpy as np

    signal, sample_rate = librosa.load(path, sr=SAMPLE_RATE, mono=True)
    duration = float(len(signal) / sample_rate)
    if not np.isfinite(signal).all() or duration <= 0.0:
        raise ValueError("audio contains no finite samples")

    if stem_paths is None:
        harmonic_input = signal
        bass_input = signal
        analysis_source = "mix"
    else:
        bass_input, other, guitar, piano = (
            _load_stem(stem_path, sample_rate) for stem_path in stem_paths
        )
        shared_frames = min(len(bass_input), len(other), len(guitar), len(piano), len(signal))
        harmonic_input = other[:shared_frames] + guitar[:shared_frames] + piano[:shared_frames]
        bass_input = bass_input[:shared_frames]
        analysis_source = "stems"

    # A wide HPSS margin follows librosa's enhanced-chroma recipe and rejects
    # drum transients. With stems, the input also excludes lead vocals.
    harmonic = librosa.effects.harmonic(harmonic_input, margin=8.0)
    bass_harmonic = librosa.effects.harmonic(bass_input, margin=4.0)
    chroma = _fused_chroma(harmonic, sample_rate)
    bass_cqt = np.abs(librosa.cqt(
        bass_harmonic,
        sr=sample_rate,
        hop_length=HOP_LENGTH,
        fmin=librosa.note_to_hz("C1"),
        n_bins=36,
        bins_per_octave=12,
    ))
    bass = np.stack([bass_cqt[pitch::12].sum(axis=0) for pitch in range(12)])
    rms = librosa.feature.rms(y=signal, hop_length=HOP_LENGTH)[0]
    frames = min(chroma.shape[1], bass.shape[1], rms.shape[0])
    chroma, bass, rms = chroma[:, :frames], bass[:, :frames], rms[:frames]
    boundaries = _boundaries(chroma, duration)
    key_root, key_minor = _key(chroma.mean(axis=1))
    peak_rms = max(float(rms.max()), np.finfo(np.float64).eps)

    segments = []
    for start, end in zip(boundaries, boundaries[1:]):
        safe_start = min(start, frames - 1)
        safe_end = min(max(end, safe_start + 1), frames)
        segment_chroma = _normalized(chroma[:, safe_start:safe_end].mean(axis=1))
        segment_bass = _normalized(bass[:, safe_start:safe_end].mean(axis=1))
        relative_db = 20.0 * math.log10(max(float(rms[safe_start:safe_end].mean()), 1e-12) / peak_rms)
        silence = float(np.clip((-35.0 - relative_db) / 25.0, 0.0, 1.0))
        segment_key_root, segment_key_minor = _key(segment_chroma)
        segments.append({
            "startSeconds": min(duration, start * HOP_LENGTH / sample_rate),
            "endSeconds": min(duration, end * HOP_LENGTH / sample_rate),
            "chroma": segment_chroma.tolist(),
            "bassChroma": segment_bass.tolist(),
            "silence": silence,
            "ambiguity": _ambiguity(segment_chroma),
            "bassStrength": _bass_strength(segment_bass),
            "keyRoot": segment_key_root,
            "keyMinor": segment_key_minor,
        })
    return {
        "featureVersion": FEATURE_VERSION,
        "analysisSource": analysis_source,
        "durationSeconds": duration,
        "keyRoot": key_root,
        "keyMinor": key_minor,
        "segments": segments,
    }


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) not in (1, 6) or (len(arguments) == 6 and arguments[1] != "--stems"):
        print("usage: python -m sonarcan_mlx_worker.chords AUDIO [--stems BASS OTHER GUITAR PIANO]", file=sys.stderr)
        return 2
    try:
        path = Path(arguments[0])
        if not path.is_absolute() or not path.is_file():
            raise ValueError("audio path must be an absolute regular file")
        stem_paths = tuple(Path(value) for value in arguments[2:]) if len(arguments) == 6 else None
        if stem_paths is not None and any(not value.is_absolute() or not value.is_file() for value in stem_paths):
            raise ValueError("stem paths must be absolute regular files")
        print(json.dumps(extract(path, stem_paths), separators=(",", ":"), allow_nan=False))
        return 0
    except Exception as error:  # structured one-line diagnostic for the Rust supervisor
        print(f"chord feature extraction failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
