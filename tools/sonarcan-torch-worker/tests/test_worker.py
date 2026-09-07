from __future__ import annotations

import unittest
from unittest.mock import patch

from scnet_infer.checkpoints import get_spec
from scnet_infer.fast_demucs import validate_contract as validate_fast_contract
from scnet_infer.sonarcan_worker import MODEL_ID, choose_torch_device, validate_contract


class WorkerTests(unittest.TestCase):
    def test_uses_the_pinned_scnet_contract(self) -> None:
        validate_contract()
        validate_fast_contract()
        self.assertEqual(get_spec(MODEL_ID).batch_size, 2)

    def test_auto_device_prefers_cuda_and_never_selects_unreliable_mps(self) -> None:
        torch = type("Torch", (), {"cuda": type("Cuda", (), {"is_available": staticmethod(lambda: True)})})
        with patch.dict("os.environ", {}, clear=True):
            self.assertEqual(choose_torch_device(torch), "cuda")

    def test_auto_device_falls_back_to_cpu(self) -> None:
        torch = type("Torch", (), {"cuda": type("Cuda", (), {"is_available": staticmethod(lambda: False)})})
        with patch.dict("os.environ", {}, clear=True):
            self.assertEqual(choose_torch_device(torch), "cpu")


if __name__ == "__main__":
    unittest.main()
