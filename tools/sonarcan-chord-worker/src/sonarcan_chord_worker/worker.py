"""Production LV-Chordia worker: bounded JSON out, diagnostics on stderr."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np

from .core import sonarcan_label
from .engine import (
    BEAT_THIS_VERSION,
    DICTIONARIES,
    FACTOR_NAMES,
    SOURCE_REVISION,
    detect_rhythm,
    dictionary_decode,
    verify_checkpoints,
    verify_downbeat_checkpoint,
)


def analyze(audio_path: Path, downbeat_model: Path, requested_device: str = "auto") -> dict:
    import torch
    from lv_chordia.device_utils import resolve_device

    audio_path = audio_path.resolve(strict=True)
    if not audio_path.is_file():
        raise ValueError("audio path must be an absolute regular file")
    if requested_device == "auto":
        requested_device = "mps" if torch.backends.mps.is_available() else "cpu"
    device = resolve_device(requested_device)
    warnings: list[str] = []
    try:
        modes = _analyze_chords(audio_path, device)
    except Exception as error:
        warnings.append(_analysis_warning("LV-Chordia", error))
        modes = {mode: [] for mode in DICTIONARIES}
    try:
        rhythm = detect_rhythm(audio_path, downbeat_model, device)
    except Exception as error:
        warnings.append(_analysis_warning("Beat This!", error))
        rhythm = ([], [], None, [], [], None)
    if len(warnings) == 2:
        raise RuntimeError("; ".join(warnings))
    (
        beats, downbeats, bpm,
        dbn_beats, dbn_downbeats, dbn_bpm,
    ) = rhythm
    return {
        "modelVersion": f"lv-chordia@{SOURCE_REVISION}",
        "downbeatModelVersion": f"beat-this@{BEAT_THIS_VERSION}:final0",
        "bpm": bpm,
        "beats": beats,
        "downbeats": downbeats,
        "dbnBpm": dbn_bpm,
        "dbnBeats": dbn_beats,
        "dbnDownbeats": dbn_downbeats,
        "modes": modes,
        "warnings": warnings,
    }


def _analysis_warning(component: str, error: Exception) -> str:
    printable = "".join(character if character.isprintable() else " " for character in str(error))
    detail = (" ".join(printable.split()) or error.__class__.__name__)[:240]
    return f"{component} failed: {detail}"


def _analyze_chords(audio_path: Path, device) -> dict[str, list[dict]]:
    from lv_chordia.chord_recognition import load_ensemble
    from lv_chordia.extractors.cqt import CQTV2
    from lv_chordia.mir import DataEntry, io
    from lv_chordia.settings import DEFAULT_HOP_LENGTH, DEFAULT_SR

    verify_checkpoints()
    ensemble = load_ensemble(False, device=device)
    entry = DataEntry()
    entry.prop.set("sr", DEFAULT_SR)
    entry.prop.set("hop_length", DEFAULT_HOP_LENGTH)
    entry.append_file(str(audio_path), io.MusicIO, "music")
    entry.append_extractor(CQTV2, "cqt")
    members = [network.inference(entry.cqt) for network in ensemble]
    probabilities = [np.mean([member[index] for member in members], axis=0) for index in range(len(FACTOR_NAMES))]
    modes: dict[str, list[dict]] = {}
    for mode, dictionary in DICTIONARIES.items():
        segments = dictionary_decode(entry, probabilities, dictionary)
        modes[mode] = [_timed(segment, sonarcan_label(segment["rawLabel"])) for segment in segments]
    return modes


def _timed(segment: dict, label: str) -> dict:
    return {
        "label": label,
        "sourceLabel": segment["rawLabel"],
        "startSeconds": segment["startSeconds"],
        "endSeconds": segment["endSeconds"],
        "strength": segment["strength"],
    }


def accelerator_self_test(downbeat_model: Path) -> dict:
    """Exercise the qualified production accelerators before enabling analysis."""
    import torch
    from beat_this.inference import load_model
    from lv_chordia.chord_recognition import load_ensemble

    if not torch.backends.mps.is_available():
        raise RuntimeError("the qualified MPS accelerator is unavailable")
    device = torch.device("mps")
    with torch.inference_mode():
        chord_input = torch.zeros((1, 16, 252), dtype=torch.float32, device=device)
        for member in load_ensemble(False, device=device):
            outputs = member.net(chord_input)
            if not all(torch.isfinite(output).all().item() for output in outputs):
                raise RuntimeError("LV-Chordia accelerator self-test produced invalid values")
        beat_model = load_model(str(downbeat_model), device)
        beat_outputs = beat_model(torch.zeros((1, 16, 128), dtype=torch.float32, device=device))
        if not all(torch.isfinite(output).all().item() for output in beat_outputs.values()):
            raise RuntimeError("Beat This! accelerator self-test produced invalid values")
        torch.mps.synchronize()

    return {"accelerated": True, "backend": "MPS"}


def main() -> int:
    parser = argparse.ArgumentParser(description="SonArcan LV-Chordia production worker")
    parser.add_argument("audio", nargs="?", type=Path)
    parser.add_argument("--device", choices=("auto", "cpu", "mps"), default="auto")
    parser.add_argument("--downbeat-model", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--accelerator-self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.accelerator_self_test:
            if args.downbeat_model is None:
                parser.error("--downbeat-model is required")
            verify_checkpoints()
            verify_downbeat_checkpoint(args.downbeat_model)
            print(json.dumps(accelerator_self_test(args.downbeat_model), separators=(",", ":")))
            return 0
        if args.self_test:
            verify_checkpoints()
            if args.downbeat_model is None:
                parser.error("--downbeat-model is required")
            verify_downbeat_checkpoint(args.downbeat_model)
            print(json.dumps({
                "ok": True,
                "modelVersion": f"lv-chordia@{SOURCE_REVISION}",
                "downbeatModelVersion": f"beat-this@{BEAT_THIS_VERSION}:final0",
                "modes": sorted(DICTIONARIES),
            }))
            return 0
        if args.audio is None:
            parser.error("audio is required unless --self-test is used")
        if args.downbeat_model is None:
            parser.error("--downbeat-model is required")
        print(json.dumps(analyze(args.audio, args.downbeat_model, args.device), separators=(",", ":")))
        return 0
    except Exception as error:
        print(f"Chord/downbeat analysis failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
