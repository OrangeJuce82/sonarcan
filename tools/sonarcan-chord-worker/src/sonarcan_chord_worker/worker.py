"""Production LV-Chordia worker: bounded JSON out, diagnostics on stderr."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np

from .core import sonarcan_label
from .engine import DICTIONARIES, FACTOR_NAMES, SOURCE_REVISION, dictionary_decode, verify_checkpoints


def analyze(audio_path: Path, requested_device: str = "auto") -> dict:
    import torch
    from lv_chordia.chord_recognition import load_ensemble
    from lv_chordia.device_utils import resolve_device
    from lv_chordia.extractors.cqt import CQTV2
    from lv_chordia.mir import DataEntry, io
    from lv_chordia.settings import DEFAULT_HOP_LENGTH, DEFAULT_SR

    audio_path = audio_path.resolve(strict=True)
    if not audio_path.is_file():
        raise ValueError("audio path must be an absolute regular file")
    if requested_device == "auto":
        requested_device = "mps" if torch.backends.mps.is_available() else "cpu"
    device = resolve_device(requested_device)
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
    return {
        "modelVersion": f"lv-chordia@{SOURCE_REVISION}",
        "modes": modes,
    }


def _timed(segment: dict, label: str) -> dict:
    return {
        "label": label,
        "startSeconds": segment["startSeconds"],
        "endSeconds": segment["endSeconds"],
        "strength": segment["strength"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="SonArcan LV-Chordia production worker")
    parser.add_argument("audio", nargs="?", type=Path)
    parser.add_argument("--device", choices=("auto", "cpu", "mps"), default="auto")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.self_test:
            verify_checkpoints()
            print(json.dumps({"ok": True, "modelVersion": f"lv-chordia@{SOURCE_REVISION}"}))
            return 0
        if args.audio is None:
            parser.error("audio is required unless --self-test is used")
        print(json.dumps(analyze(args.audio, args.device), separators=(",", ":")))
        return 0
    except Exception as error:
        print(f"LV-Chordia analysis failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
