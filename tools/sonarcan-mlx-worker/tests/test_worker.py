from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from sonarcan_mlx_worker.worker import JsonProgress, MODEL_CONFIG_FIELDS, STEM_NAMES, validate_model_files
from sonarcan_mlx_worker.build_model import without_training_metadata
from sonarcan_mlx_worker.refresh_records import file_record, refresh


def model_config(digest: str) -> dict[str, object]:
    config: dict[str, object] = {field: None for field in MODEL_CONFIG_FIELDS}
    config.update({
        "format_version": 1, "model_name": "htdemucs_6s", "model_class": "HTDemucsMLX",
        "args": [], "kwargs": {"sources": list(STEM_NAMES)}, "per_model_args": [[]],
        "per_model_kwargs": [{"sources": list(STEM_NAMES)}],
        "per_model_classes": ["HTDemucsMLX"], "num_models": 1,
        "source_artifacts": [{"signature": "5c90dfd2", "checksum": "34c22ccb"}],
        "mlx_version": "0.31.2", "conversion_date": "2026-08-29T00:00:00+00:00",
        "verification_passed": False, "safetensors_sha256": digest,
    })
    return config


class WorkerContractTests(unittest.TestCase):
    def test_model_validation_accepts_the_exact_six_stem_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            model_dir = Path(directory)
            weights = model_dir / "htdemucs_6s.safetensors"
            weights.write_bytes(b"safe-test-weights")
            digest = hashlib.sha256(weights.read_bytes()).hexdigest()
            (model_dir / "htdemucs_6s_config.json").write_text(
                json.dumps(model_config(digest)),
                encoding="utf-8",
            )
            self.assertEqual(validate_model_files(model_dir)[0], weights)

    def test_model_validation_rejects_a_modified_weight_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            model_dir = Path(directory)
            (model_dir / "htdemucs_6s.safetensors").write_bytes(b"modified")
            (model_dir / "htdemucs_6s_config.json").write_text(
                json.dumps(model_config("0" * 64)),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "checksum"):
                validate_model_files(model_dir)

    def test_progress_events_are_monotonic_and_bounded(self) -> None:
        output = StringIO()
        with redirect_stdout(output):
            progress = JsonProgress(total=3)
            progress.update()
            progress.update(2)
        events = [json.loads(line) for line in output.getvalue().splitlines()]
        values = [event["progress"] for event in events]
        self.assertEqual(values, sorted(values))
        self.assertGreaterEqual(min(values), 0.0)
        self.assertLessEqual(max(values), 1.0)
        self.assertEqual(events[-1]["completed"], 3)

    def test_model_builder_removes_only_unused_training_metadata(self) -> None:
        package = {"klass": "HTDemucs", "args": {"sources": list(STEM_NAMES)}, "state": {"x": 1}, "training_args": {1: "unused"}}
        sanitized = without_training_metadata(package)
        self.assertNotIn("training_args", sanitized)
        self.assertEqual(sanitized["state"], package["state"])
        self.assertIn("training_args", package)

    def test_refreshes_wheel_records_after_native_signing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory)
            site_packages = runtime / "lib/python3.13/site-packages"
            package = site_packages / "example"
            dist_info = site_packages / "example-1.0.dist-info"
            package.mkdir(parents=True)
            dist_info.mkdir()
            native = package / "native.so"
            native.write_bytes(b"signed-native-bytes")
            record = dist_info / "RECORD"
            record.write_text("example/native.so,sha256=old,1\nexample-1.0.dist-info/RECORD,,\n")
            self.assertEqual(refresh(site_packages), 1)
            digest, size = file_record(native)
            self.assertIn(f"example/native.so,{digest},{size}", record.read_text())


if __name__ == "__main__":
    unittest.main()
