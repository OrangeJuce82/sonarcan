"""Resident LV-Chordia worker with bounded newline-delimited JSON output."""

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
    bpm_from_beats,
    dictionary_decode,
    verify_checkpoints,
    verify_downbeat_checkpoint,
)

MAX_REQUEST_BYTES = 16 * 1024


class ResidentAnalyzer:
    """Keep both model families loaded and one LV-Chordia result warm."""

    def __init__(self, downbeat_model: Path, requested_device: str = "auto") -> None:
        import torch
        from beat_this.inference import Audio2Frames
        from lv_chordia.chord_recognition import load_ensemble
        from lv_chordia.device_utils import resolve_device

        if requested_device == "auto":
            if not torch.backends.mps.is_available():
                raise RuntimeError("the required MPS accelerator is unavailable")
            requested_device = "mps"
        self.device = resolve_device(requested_device)
        verify_checkpoints()
        verify_downbeat_checkpoint(downbeat_model)
        self.ensemble = load_ensemble(False, device=self.device)
        self.rhythm_tracker = Audio2Frames(
            checkpoint_path=str(downbeat_model), device=self.device
        )
        self._chord_source: tuple[str, int, int] | None = None
        self._chord_entry = None
        self._chord_probabilities = None

    def self_test(self) -> dict:
        import torch

        with torch.inference_mode():
            chord_input = torch.zeros((1, 16, 252), dtype=torch.float32, device=self.device)
            for member in self.ensemble:
                outputs = member.net(chord_input)
                if not all(torch.isfinite(output).all().item() for output in outputs):
                    raise RuntimeError("LV-Chordia accelerator self-test produced invalid values")
            beat_outputs = self.rhythm_tracker.model(
                torch.zeros((1, 16, 128), dtype=torch.float32, device=self.device)
            )
            if not all(torch.isfinite(output).all().item() for output in beat_outputs.values()):
                raise RuntimeError("Beat This! accelerator self-test produced invalid values")
            if self.device.type == "mps":
                torch.mps.synchronize()
        return {"accelerated": True, "backend": self.device.type.upper()}

    def analyze(self, audio_path: Path, mode: str, include_rhythm: bool) -> dict:
        audio_path = _validated_audio_path(audio_path)
        if mode not in DICTIONARIES:
            raise ValueError("invalid LV-Chordia mode")
        warnings: list[str] = []
        try:
            modes = {mode: self._analyze_chord_mode(audio_path, mode)}
        except Exception as error:
            warnings.append(_analysis_warning("LV-Chordia", error))
            modes = {}

        rhythm = None
        if include_rhythm:
            try:
                rhythm = self._detect_rhythm(audio_path)
            except Exception as error:
                warnings.append(_analysis_warning("Beat This!", error))
        if include_rhythm and rhythm is None and not modes:
            raise RuntimeError("; ".join(warnings))

        result = {
            "modelVersion": f"lv-chordia@{SOURCE_REVISION}",
            "downbeatModelVersion": f"beat-this@{BEAT_THIS_VERSION}:final0",
            "modes": modes,
            "warnings": warnings,
        }
        if rhythm is not None:
            beats, downbeats, bpm, dbn_beats, dbn_downbeats, dbn_bpm = rhythm
            result.update(
                bpm=bpm,
                beats=beats,
                downbeats=downbeats,
                dbnBpm=dbn_bpm,
                dbnBeats=dbn_beats,
                dbnDownbeats=dbn_downbeats,
            )
        elif include_rhythm:
            result.update(
                bpm=None,
                beats=[],
                downbeats=[],
                dbnBpm=None,
                dbnBeats=[],
                dbnDownbeats=[],
            )
        return result

    def _analyze_chord_mode(self, audio_path: Path, mode: str) -> list[dict]:
        source = _source_identity(audio_path)
        if source != self._chord_source:
            self._chord_source = None
            self._chord_entry = None
            self._chord_probabilities = None
            self._load_chord_probabilities(audio_path)
            self._chord_source = source
        segments = dictionary_decode(
            self._chord_entry, self._chord_probabilities, DICTIONARIES[mode]
        )
        return [_timed(segment, sonarcan_label(segment["rawLabel"])) for segment in segments]

    def _load_chord_probabilities(self, audio_path: Path) -> None:
        from lv_chordia.extractors.cqt import CQTV2
        from lv_chordia.mir import DataEntry, io
        from lv_chordia.settings import DEFAULT_HOP_LENGTH, DEFAULT_SR

        entry = DataEntry()
        entry.prop.set("sr", DEFAULT_SR)
        entry.prop.set("hop_length", DEFAULT_HOP_LENGTH)
        entry.append_file(str(audio_path), io.MusicIO, "music")
        entry.append_extractor(CQTV2, "cqt")
        members = [network.inference(entry.cqt) for network in self.ensemble]
        probabilities = [
            np.mean([member[index] for member in members], axis=0)
            for index in range(len(FACTOR_NAMES))
        ]
        self._chord_entry = entry
        self._chord_probabilities = probabilities

    def _detect_rhythm(
        self, audio_path: Path
    ) -> tuple[
        list[float],
        list[float],
        float | None,
        list[float],
        list[float],
        float | None,
    ]:
        from beat_this.model.postprocessor import Postprocessor
        from beat_this.preprocessing import load_audio

        from .rhythm import prepare_postprocessing_logits

        signal, sample_rate = load_audio(str(audio_path))
        beat_logits, downbeat_logits = self.rhythm_tracker(signal, sample_rate)
        beat_logits = prepare_postprocessing_logits(beat_logits)
        downbeat_logits = prepare_postprocessing_logits(downbeat_logits)
        beats, downbeats = Postprocessor(type="minimal")(beat_logits, downbeat_logits)
        dbn_beats, dbn_downbeats = Postprocessor(type="dbn")(beat_logits, downbeat_logits)
        beat_times = [round(float(position), 6) for position in beats]
        downbeat_times = [round(float(position), 6) for position in downbeats]
        dbn_beat_times = [round(float(position), 6) for position in dbn_beats]
        dbn_downbeat_times = [round(float(position), 6) for position in dbn_downbeats]
        return (
            beat_times,
            downbeat_times,
            bpm_from_beats(beat_times),
            dbn_beat_times,
            dbn_downbeat_times,
            bpm_from_beats(dbn_beat_times),
        )


def analyze(
    audio_path: Path,
    downbeat_model: Path,
    requested_device: str = "auto",
    mode: str = "standard",
) -> dict:
    """One-shot compatibility entry point used by focused worker tests."""
    return ResidentAnalyzer(downbeat_model, requested_device).analyze(
        audio_path, mode, include_rhythm=True
    )


def _validated_audio_path(audio_path: Path) -> Path:
    resolved = audio_path.resolve(strict=True)
    if not resolved.is_file():
        raise ValueError("audio path must be an absolute regular file")
    return resolved


def _source_identity(audio_path: Path) -> tuple[str, int, int]:
    metadata = audio_path.stat()
    return str(audio_path), metadata.st_size, metadata.st_mtime_ns


def _analysis_warning(component: str, error: Exception) -> str:
    printable = "".join(character if character.isprintable() else " " for character in str(error))
    detail = (" ".join(printable.split()) or error.__class__.__name__)[:240]
    return f"{component} failed: {detail}"


def _timed(segment: dict, label: str) -> dict:
    return {
        "label": label,
        "sourceLabel": segment["rawLabel"],
        "startSeconds": segment["startSeconds"],
        "endSeconds": segment["endSeconds"],
        "strength": segment["strength"],
    }


def _serve(analyzer: ResidentAnalyzer) -> int:
    for raw_line in sys.stdin.buffer:
        if len(raw_line) > MAX_REQUEST_BYTES:
            _write_message({"error": "worker request exceeded 16 KiB"})
            continue
        try:
            request = json.loads(raw_line)
            command = request.get("command")
            if command == "selfTest":
                response = analyzer.self_test()
            elif command == "analyze":
                if not isinstance(request.get("includeRhythm"), bool):
                    raise ValueError("invalid rhythm request")
                response = analyzer.analyze(
                    Path(request["audio"]),
                    request["mode"],
                    request["includeRhythm"],
                )
            else:
                raise ValueError("invalid worker command")
            _write_message(response)
        except Exception as error:
            _write_message({"error": _bounded_error(error)})
    return 0


def _bounded_error(error: Exception) -> str:
    printable = "".join(character if character.isprintable() else " " for character in str(error))
    return (" ".join(printable.split()) or error.__class__.__name__)[:512]


def _write_message(message: dict) -> None:
    print(json.dumps(message, separators=(",", ":")), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="SonArcan LV-Chordia production worker")
    parser.add_argument("audio", nargs="?", type=Path)
    parser.add_argument("--mode", choices=tuple(DICTIONARIES), default="standard")
    parser.add_argument("--device", choices=("auto", "cpu", "cuda", "mps"), default="auto")
    parser.add_argument("--downbeat-model", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--accelerator-self-test", action="store_true")
    parser.add_argument("--serve", action="store_true")
    args = parser.parse_args()
    try:
        if args.downbeat_model is None:
            parser.error("--downbeat-model is required")
        if args.self_test:
            verify_checkpoints()
            verify_downbeat_checkpoint(args.downbeat_model)
            _write_message(
                {
                    "ok": True,
                    "modelVersion": f"lv-chordia@{SOURCE_REVISION}",
                    "downbeatModelVersion": f"beat-this@{BEAT_THIS_VERSION}:final0",
                    "modes": sorted(DICTIONARIES),
                }
            )
            return 0
        analyzer = ResidentAnalyzer(args.downbeat_model, args.device)
        if args.accelerator_self_test:
            _write_message(analyzer.self_test())
            return 0
        if args.serve:
            return _serve(analyzer)
        if args.audio is None:
            parser.error("audio is required unless --self-test or --serve is used")
        _write_message(analyzer.analyze(args.audio, args.mode, include_rhythm=True))
        return 0
    except Exception as error:
        print(f"Chord/downbeat analysis failed: {_bounded_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
