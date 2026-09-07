from __future__ import annotations

import hashlib
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from scnet_infer.checkpoints import ChecksumError, get_spec, resolve_checkpoint


class CheckpointSecurityTests(unittest.TestCase):
    def test_download_is_atomic_verified_and_reports_progress(self) -> None:
        payload = b"scnet-test-checkpoint"
        digest = hashlib.sha256(payload).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.ckpt"
            source.write_bytes(payload)
            spec = replace(
                get_spec(),
                url=source.as_uri(),
                filename="model.ckpt",
                size=len(payload),
                sha256=digest,
            )
            progress: list[tuple[int, int]] = []
            result = resolve_checkpoint(spec, cache_dir=root / "cache", progress=lambda done, total: progress.append((done, total)))
            self.assertEqual(result.read_bytes(), payload)
            self.assertTrue(progress)
            self.assertFalse(any(result.parent.glob("*.part")))

    def test_modified_cached_checkpoint_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            spec = replace(
                get_spec(),
                url="https://models.invalid/model.ckpt",
                filename="model.ckpt",
                size=len(b"modified"),
                sha256="0" * 64,
            )
            cached = root / spec.model_id / spec.filename
            cached.parent.mkdir(parents=True)
            cached.write_bytes(b"modified")
            with self.assertRaises(ChecksumError):
                resolve_checkpoint(spec, cache_dir=root)


if __name__ == "__main__":
    unittest.main()
