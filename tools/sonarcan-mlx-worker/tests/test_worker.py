from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

import numpy as np

from scnet_infer.checkpoints import get_spec
from scnet_infer.fast_demucs import MODEL_BYTES as FAST_MODEL_BYTES, validate_contract as validate_fast_contract
from scnet_infer.sonarcan_worker import MODEL_ID, STEM_NAMES, build_parser, validate_contract, write_float_wave
from sonarcan_mlx_worker.worker import main


class WorkerContractTests(unittest.TestCase):
    def test_uses_the_pinned_four_stem_scnet_contract(self) -> None:
        validate_contract()
        spec = get_spec(MODEL_ID)
        self.assertEqual(spec.family, "scnet")
        self.assertEqual(spec.size, 168_852_258)
        self.assertEqual(set(spec.sources), set(STEM_NAMES))
        self.assertEqual(spec.license, "MIT")

    def test_uses_the_pinned_four_stem_fast_contract(self) -> None:
        validate_fast_contract()
        self.assertEqual(FAST_MODEL_BYTES, 84_141_911)

    def test_health_checks_do_not_require_or_download_the_model(self) -> None:
        self.assertEqual(build_parser("test").parse_args(["self-test"]).command, "self-test")
        output = StringIO()
        with redirect_stdout(output):
            self.assertEqual(main(["self-test"]), 0)
        event = json.loads(output.getvalue())
        self.assertEqual(event["model"], MODEL_ID)
        self.assertEqual(event["stems"], list(STEM_NAMES))

    def test_separation_requires_a_verified_cache_directory(self) -> None:
        parsed = build_parser("test").parse_args([
            "separate", "--input", "/tmp/input.wav", "--output", "/tmp/output",
            "--model-dir", "/tmp/models", "--ffmpeg", "/tmp/ffmpeg", "--profile", "fast",
        ])
        self.assertEqual(parsed.model_dir, Path("/tmp/models"))
        self.assertEqual(parsed.profile, "fast")

    def test_writes_ieee_float_wave(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "stem.wav"
            write_float_wave(destination, np.array([[0.0, 0.5], [0.25, -0.5]], dtype=np.float32))
            wave = destination.read_bytes()
            self.assertEqual(wave[:4], b"RIFF")
            self.assertEqual(wave[8:12], b"WAVE")
            self.assertEqual(int.from_bytes(wave[20:22], "little"), 3)


if __name__ == "__main__":
    unittest.main()
