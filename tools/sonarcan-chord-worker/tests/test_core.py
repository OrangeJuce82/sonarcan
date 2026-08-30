import unittest

from sonarcan_chord_worker.core import pitch_class, reduced_label, sonarcan_label


class CoreTests(unittest.TestCase):
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
