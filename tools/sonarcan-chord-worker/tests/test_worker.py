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
            return [0.5], [0.5], 120.0

        with tempfile.NamedTemporaryFile() as audio_file:
            with (
                patch("lv_chordia.device_utils.resolve_device", return_value="cpu"),
                patch("sonarcan_chord_worker.worker._analyze_chords", side_effect=analyze_chords),
                patch("sonarcan_chord_worker.worker.detect_rhythm", side_effect=analyze_rhythm),
            ):
                result = analyze(Path(audio_file.name), Path("beat-this.ckpt"), "cpu")

        self.assertEqual(calls, ["lv-chordia", "beat-this"])
        self.assertEqual(result["beats"], [0.5])
        self.assertEqual(result["modes"], {"submission": []})


if __name__ == "__main__":
    unittest.main()
