import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from sonarcan_chord_worker.worker import analyze


class WorkerTests(unittest.TestCase):
    def test_runs_lv_chordia_before_beat_this(self):
        calls = []

        def analyze_chords(_audio_path, _device):
            calls.append("lv-chordia")
            return {"submission": []}

        def analyze_rhythm(_audio_path, _model_path, _device):
            calls.append("beat-this")
            return (
                [0.5], [0.5], 120.0,
                [0.5], [0.5], 120.0,
            )

        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch("lv_chordia.device_utils.resolve_device", return_value="cpu"),
                patch("sonarcan_chord_worker.worker._analyze_chords", side_effect=analyze_chords),
                patch("sonarcan_chord_worker.worker.detect_rhythm", side_effect=analyze_rhythm),
            ):
                result = analyze(Path(audio_file.name), Path("beat-this.ckpt"), "cpu")

        self.assertEqual(calls, ["lv-chordia", "beat-this"])
        self.assertEqual(result["beats"], [0.5])
        self.assertEqual(result["dbnBeats"], [0.5])
        self.assertEqual(result["modes"], {"submission": []})
        self.assertEqual(result["warnings"], [])

    def test_beat_this_still_runs_when_lv_chordia_fails(self):
        rhythm = (
            [0.5], [0.5], 120.0,
            [0.5], [0.5], 120.0,
        )
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch("lv_chordia.device_utils.resolve_device", return_value="cpu"),
                patch(
                    "sonarcan_chord_worker.worker._analyze_chords",
                    side_effect=RuntimeError("broken chords"),
                ),
                patch("sonarcan_chord_worker.worker.detect_rhythm", return_value=rhythm),
            ):
                result = analyze(Path(audio_file.name), Path("beat-this.ckpt"), "cpu")

        self.assertEqual(result["beats"], [0.5])
        self.assertEqual(result["modes"], {
            "essential": [], "standard": [], "complete": []
        })
        self.assertEqual(result["warnings"], ["LV-Chordia failed: broken chords"])

    def test_lv_chordia_still_returns_when_beat_this_fails(self):
        modes = {"essential": [], "standard": [], "complete": []}
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch("lv_chordia.device_utils.resolve_device", return_value="cpu"),
                patch("sonarcan_chord_worker.worker._analyze_chords", return_value=modes),
                patch(
                    "sonarcan_chord_worker.worker.detect_rhythm",
                    side_effect=RuntimeError("broken rhythm"),
                ),
            ):
                result = analyze(Path(audio_file.name), Path("beat-this.ckpt"), "cpu")

        self.assertEqual(result["modes"], modes)
        self.assertEqual(result["beats"], [])
        self.assertEqual(result["warnings"], ["Beat This! failed: broken rhythm"])

    def test_both_analysis_failures_remain_fatal(self):
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch("lv_chordia.device_utils.resolve_device", return_value="cpu"),
                patch(
                    "sonarcan_chord_worker.worker._analyze_chords",
                    side_effect=RuntimeError("broken chords"),
                ),
                patch(
                    "sonarcan_chord_worker.worker.detect_rhythm",
                    side_effect=RuntimeError("broken rhythm"),
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "LV-Chordia.*Beat This"):
                    analyze(Path(audio_file.name), Path("beat-this.ckpt"), "cpu")


if __name__ == "__main__":
    unittest.main()
