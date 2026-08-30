import unittest
import tempfile
import struct
import wave
from pathlib import Path

try:
    import numpy as np
except ModuleNotFoundError:  # release/runtime integration supplies locked dependencies
    np = None

from sonarcan_mlx_worker.chords import _ambiguity, _boundaries, _key, _normalized, _stem_signal, extract


@unittest.skipUnless(np is not None, "numpy is provided by the locked worker runtime")
class ChordFeatureTests(unittest.TestCase):
    def test_normalizes_chroma_without_nan(self):
        np.testing.assert_array_equal(_normalized(np.zeros(12)), np.zeros(12))
        self.assertAlmostEqual(float(_normalized(np.ones(12)).sum()), 1.0)

    def test_estimates_c_major_and_a_minor_profiles(self):
        c_major = np.zeros(12)
        c_major[[0, 4, 7]] = [1.0, 0.8, 0.8]
        a_minor = np.zeros(12)
        a_minor[[9, 0, 4]] = [1.0, 0.8, 0.8]
        self.assertEqual(_key(c_major), (0, False))
        self.assertEqual(_key(a_minor), (9, True))

    def test_segments_on_harmonic_change_without_beats(self):
        chroma = np.zeros((12, 60))
        chroma[[0, 4, 7], :30] = 1.0
        chroma[[7, 11, 2], 30:] = 1.0
        boundaries = _boundaries(chroma, 60 * 512 / 22_050)
        self.assertTrue(any(abs(boundary - 30) <= 4 for boundary in boundaries))
        self.assertEqual(boundaries[0], 0)
        self.assertEqual(boundaries[-1], 60)

    def test_long_stable_passages_are_bounded_for_temporal_decoding(self):
        chroma = np.zeros((12, 500))
        chroma[[0, 4, 7], :] = 1.0
        boundaries = _boundaries(chroma, 500 * 512 / 22_050)
        self.assertLessEqual(max(np.diff(boundaries)), round(4.0 * 22_050 / 512))

    def test_diffuse_chroma_is_ambiguous_but_a_noisy_triad_is_not(self):
        self.assertGreater(_ambiguity(np.ones(12)), 0.9)
        noisy_triad = np.full(12, 0.02)
        noisy_triad[[0, 4, 7]] = [0.3, 0.25, 0.27]
        self.assertLess(_ambiguity(noisy_triad), 0.2)

    def test_extracts_bounded_observations_without_assigning_labels(self):
        sample_rate = 22_050
        time = np.arange(sample_rate * 2) / sample_rate
        frequencies = [261.63, 329.63, 392.0]
        signal = sum(np.sin(2 * np.pi * frequency * time) for frequency in frequencies) / 4
        pcm = np.asarray(np.clip(signal, -1, 1) * 32_767, dtype="<i2")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "c-major.wav"
            with wave.open(str(path), "wb") as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(sample_rate)
                output.writeframes(pcm.tobytes())
            result = extract(path)
        self.assertEqual(result["featureVersion"], 3)
        self.assertEqual(result["analysisSource"], "mix")
        self.assertGreater(len(result["segments"]), 0)
        self.assertLessEqual(len(result["segments"]), 4_096)
        self.assertNotIn("label", result["segments"][0])

    def test_reads_the_validated_sonarcan_stem_contract(self):
        stereo = np.asarray([[0.2, 0.4], [-0.3, 0.1]], dtype="<f4")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bass.pcm"
            path.write_bytes(b"SACSTM02" + struct.pack("<IQ", 44_100, 2) + stereo.tobytes())
            signal, sample_rate = _stem_signal(path)
        self.assertEqual(sample_rate, 44_100)
        np.testing.assert_allclose(signal, [0.3, -0.1])


if __name__ == "__main__":
    unittest.main()
