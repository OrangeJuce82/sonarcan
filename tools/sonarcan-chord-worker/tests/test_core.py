import unittest

from sonarcan_chord_worker.core import pitch_class, reduced_label, sonarcan_label
from sonarcan_chord_worker.engine import bpm_from_beats, normalize_segment_boundaries


class CoreTests(unittest.TestCase):
    def test_bpm_is_an_indication_derived_from_detected_beat_intervals(self):
        self.assertEqual(bpm_from_beats([0.1, 0.6, 1.1, 1.6]), 120.0)
        self.assertIsNone(bpm_from_beats([0.1]))

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

    def test_chord_boundaries_never_overlap_after_rounding(self):
        segments = [
            {"startSeconds": 0.0, "endSeconds": 4.0008},
            {"startSeconds": 4.0, "endSeconds": 8.0006},
            {"startSeconds": 8.0, "endSeconds": 12.0},
        ]

        normalized = normalize_segment_boundaries(segments)

        self.assertEqual(normalized[0]["endSeconds"], 4.0)
        self.assertEqual(normalized[1]["endSeconds"], 8.0)
        self.assertTrue(all(
            previous["endSeconds"] <= current["startSeconds"]
            for previous, current in zip(normalized, normalized[1:])
        ))

    def test_chord_boundary_normalization_does_not_hide_large_model_overlaps(self):
        segments = [
            {"startSeconds": 0.0, "endSeconds": 4.01},
            {"startSeconds": 4.0, "endSeconds": 8.0},
        ]

        normalized = normalize_segment_boundaries(segments)

        self.assertEqual(normalized[0]["endSeconds"], 4.01)

if __name__ == "__main__":
    unittest.main()
