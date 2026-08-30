import unittest

import numpy as np

from sonarcan_chord_worker.core import FrameLabel, decode_triad_class, native_triad_frames, pitch_class, reduced_label, segment_frames, sonarcan_label


class CoreTests(unittest.TestCase):
    def test_native_head_contains_exactly_six_families_and_n(self):
        self.assertEqual(decode_triad_class(0, 0.7).label, "N")
        self.assertEqual(decode_triad_class(1, 0.7).label, "C")
        self.assertEqual(decode_triad_class(15, 0.7).label, "Dm")
        self.assertEqual(decode_triad_class(28, 0.7).label, "D#sus4")
        self.assertEqual(decode_triad_class(41, 0.7).label, "Esus2")
        self.assertEqual(decode_triad_class(54, 0.7).label, "Fdim")
        self.assertEqual(decode_triad_class(67, 0.7).label, "F#aug")

    def test_argmax_is_the_only_level_b_decision(self):
        probabilities = np.zeros((2, 73), dtype=np.float32)
        probabilities[0, 1] = 0.6
        probabilities[1, 14] = 0.8
        self.assertEqual([frame.label for frame in native_triad_frames(probabilities)], ["C", "C#m"])

    def test_segments_keep_n_but_present_it_as_dash(self):
        n = FrameLabel("N", "N", None, 0.9)
        self.assertEqual(segment_frames([n], 0.5)[0]["displayLabel"], "-")

    def test_extension_reduction_does_not_change_root_or_triad(self):
        self.assertEqual(reduced_label("C:maj7").label, "C")
        self.assertEqual(reduced_label("D:min9").label, "Dm")
        self.assertEqual(reduced_label("B:dim7").label, "Bdim")
        self.assertEqual(reduced_label("F#:sus4").label, "F#sus4")
        self.assertEqual(reduced_label("X").label, "N")
        self.assertEqual(pitch_class("Db"), pitch_class("C#"))
        self.assertEqual(sonarcan_label("C:maj"), "C")
        self.assertEqual(sonarcan_label("E:min7"), "Em7")
        self.assertEqual(sonarcan_label("B:hdim7"), "Bm7b5")
        self.assertEqual(sonarcan_label("D:7/3"), "D7/F#")
        self.assertEqual(sonarcan_label("Bb:min/b3"), "Bbm/Db")

if __name__ == "__main__":
    unittest.main()
