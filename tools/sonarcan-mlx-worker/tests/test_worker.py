from __future__ import annotations

import json
import struct
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from scnet_infer.backends.mlx_backend import MLXBackend
from scnet_infer.checkpoints import get_spec
from scnet_infer.fast_demucs import MLX_BATCH_SIZE, MODEL_BYTES as FAST_MODEL_BYTES, validate_contract as validate_fast_contract
from scnet_infer.sonarcan_worker import MODEL_ID, STEM_NAMES, _write_float_wave_data, build_parser, validate_contract
from sonarcan_mlx_worker.worker import main


class WorkerContractTests(unittest.TestCase):
    def test_contract_imports_do_not_require_inference_packages(self) -> None:
        script = """
import sys
for name in ('mlx', 'numpy', 'torch'):
    sys.modules[name] = None
import scnet_infer
from scnet_infer.fast_demucs import validate_contract as validate_fast_contract
from scnet_infer.sonarcan_worker import validate_contract
from sonarcan_mlx_worker.worker import main
validate_contract()
validate_fast_contract()
assert callable(main)
"""
        result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_uses_the_pinned_four_stem_scnet_contract(self) -> None:
        validate_contract()
        spec = get_spec(MODEL_ID)
        self.assertEqual(spec.family, "scnet")
        self.assertEqual(spec.size, 168_852_258)
        self.assertEqual(set(spec.sources), set(STEM_NAMES))
        self.assertEqual(spec.license, "MIT")

    def test_scnet_large_uses_the_shared_memory_safe_batch(self) -> None:
        spec = get_spec(MODEL_ID)
        backend = MLXBackend(object(), spec)
        self.assertEqual(spec.batch_size, 2)
        self.assertEqual(backend._plan.batch_size, 2)

    def test_uses_the_pinned_four_stem_fast_contract(self) -> None:
        validate_fast_contract()
        self.assertEqual(FAST_MODEL_BYTES, 84_141_911)
        self.assertEqual(MLX_BATCH_SIZE, 2)

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
            samples = struct.pack("<ffff", 0.0, 0.25, 0.5, -0.5)
            _write_float_wave_data(destination, samples, frames=2)
            wave = destination.read_bytes()
            self.assertEqual(wave[:4], b"RIFF")
            self.assertEqual(wave[8:12], b"WAVE")
            self.assertEqual(int.from_bytes(wave[20:22], "little"), 3)


if __name__ == "__main__":
    unittest.main()
