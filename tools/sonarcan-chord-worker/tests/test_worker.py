import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from sonarcan_chord_worker.worker import ResidentAnalyzer, analyze


class WorkerTests(unittest.TestCase):
    def test_auto_device_never_falls_back_to_cpu(self):
        with tempfile.NamedTemporaryFile() as audio_file:
            with patch("torch.backends.mps.is_available", return_value=False):
                with self.assertRaisesRegex(RuntimeError, "required MPS accelerator"):
                    analyze(Path(audio_file.name), Path("beat-this.ckpt"))

    def test_runs_requested_lv_chordia_mode_before_beat_this(self):
        calls = []
        analyzer = object.__new__(ResidentAnalyzer)

        def analyze_chords(_audio_path, mode):
            calls.append(f"lv-chordia:{mode}")
            return []

        def analyze_rhythm(_audio_path):
            calls.append("beat-this")
            return [0.5], [0.5], 120.0, [0.5], [0.5], 120.0

        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch.object(analyzer, "_analyze_chord_mode", side_effect=analyze_chords),
                patch.object(analyzer, "_detect_rhythm", side_effect=analyze_rhythm),
            ):
                result = analyzer.analyze(Path(audio_file.name), "essential", True)

        self.assertEqual(calls, ["lv-chordia:essential", "beat-this"])
        self.assertEqual(result["beats"], [0.5])
        self.assertEqual(result["dbnBeats"], [0.5])
        self.assertEqual(result["modes"], {"essential": []})
        self.assertEqual(result["warnings"], [])

    def test_on_demand_mode_omits_rhythm_work(self):
        analyzer = object.__new__(ResidentAnalyzer)
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch.object(analyzer, "_analyze_chord_mode", return_value=[]),
                patch.object(analyzer, "_detect_rhythm") as rhythm,
            ):
                result = analyzer.analyze(Path(audio_file.name), "complete", False)

        rhythm.assert_not_called()
        self.assertEqual(result["modes"], {"complete": []})
        self.assertNotIn("beats", result)

    def test_on_demand_mode_reuses_current_source_probabilities(self):
        analyzer = object.__new__(ResidentAnalyzer)
        analyzer._chord_source = None
        analyzer._chord_entry = None
        analyzer._chord_probabilities = None

        def load_probabilities(_audio_path):
            analyzer._chord_entry = "entry"
            analyzer._chord_probabilities = "probabilities"

        with tempfile.NamedTemporaryFile() as audio_file:
            audio_path = Path(audio_file.name).resolve()
            with (
                patch.object(
                    analyzer,
                    "_load_chord_probabilities",
                    side_effect=load_probabilities,
                ) as load,
                patch("sonarcan_chord_worker.worker.dictionary_decode", return_value=[]),
            ):
                analyzer._analyze_chord_mode(audio_path, "essential")
                analyzer._analyze_chord_mode(audio_path, "complete")

        load.assert_called_once_with(audio_path)

    def test_beat_this_still_runs_when_lv_chordia_fails(self):
        analyzer = object.__new__(ResidentAnalyzer)
        rhythm = [0.5], [0.5], 120.0, [0.5], [0.5], 120.0
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch.object(analyzer, "_analyze_chord_mode", side_effect=RuntimeError("broken chords")),
                patch.object(analyzer, "_detect_rhythm", return_value=rhythm),
            ):
                result = analyzer.analyze(Path(audio_file.name), "standard", True)

        self.assertEqual(result["modes"], {})
        self.assertEqual(result["warnings"], ["LV-Chordia failed: broken chords"])

    def test_lv_chordia_still_returns_when_beat_this_fails(self):
        analyzer = object.__new__(ResidentAnalyzer)
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch.object(analyzer, "_analyze_chord_mode", return_value=[]),
                patch.object(analyzer, "_detect_rhythm", side_effect=RuntimeError("broken rhythm")),
            ):
                result = analyzer.analyze(Path(audio_file.name), "standard", True)

        self.assertEqual(result["modes"], {"standard": []})
        self.assertEqual(result["beats"], [])
        self.assertEqual(result["warnings"], ["Beat This! failed: broken rhythm"])

    def test_both_fail_the_request(self):
        analyzer = object.__new__(ResidentAnalyzer)
        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch.object(analyzer, "_analyze_chord_mode", side_effect=RuntimeError("broken chords")),
                patch.object(analyzer, "_detect_rhythm", side_effect=RuntimeError("broken rhythm")),
            ):
                with self.assertRaisesRegex(RuntimeError, "LV-Chordia.*Beat This"):
                    analyzer.analyze(Path(audio_file.name), "standard", True)


if __name__ == "__main__":
    unittest.main()
