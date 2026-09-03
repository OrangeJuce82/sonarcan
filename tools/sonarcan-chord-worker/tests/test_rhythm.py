import unittest

import torch
from beat_this.model.postprocessor import Postprocessor

from sonarcan_chord_worker.rhythm import (
    prepare_postprocessing_logits,
)


class RhythmPostprocessingTests(unittest.TestCase):
    def test_prepares_float32_cpu_logits_for_mps_incompatible_dbn_conversion(self):
        source = torch.tensor([1.0, -1.0], dtype=torch.float64, requires_grad=True)

        prepared = prepare_postprocessing_logits(source)

        self.assertEqual(prepared.device.type, "cpu")
        self.assertEqual(prepared.dtype, torch.float32)
        self.assertFalse(prepared.requires_grad)

    def test_beat_this_dbn_decodes_a_regular_four_four_grid(self):
        beat_logits = torch.full((400,), -6.0)
        downbeat_logits = torch.full((400,), -6.0)
        beat_logits[::25] = 6.0
        downbeat_logits[::100] = 6.0

        beats, downbeats = Postprocessor(type="dbn")(beat_logits, downbeat_logits)

        self.assertGreater(len(beats), 8)
        self.assertGreater(len(downbeats), 1)
        self.assertTrue(set(downbeats).issubset(set(beats)))

if __name__ == "__main__":
    unittest.main()
