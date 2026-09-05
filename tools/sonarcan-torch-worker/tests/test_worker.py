from __future__ import annotations

import struct
import tempfile
import unittest
from pathlib import Path
from sonarcan_torch_worker.worker import (
    ACCELERATOR_SHIFTS,
    CPU_SHIFTS,
    INFERENCE_OVERLAP,
    MODEL_NAME,
    MODEL_SOURCE,
    build_parser,
    inference_settings,
    mlx_parameter_name,
    validate_model_files,
    write_float_wave,
)


class WorkerTests(unittest.TestCase):
    def test_exposes_a_bounded_accelerator_probe_command(self) -> None:
        parsed = build_parser().parse_args([
            "accelerator-self-test", "--model-dir", "/tmp/model",
        ])
        self.assertEqual(parsed.command, "accelerator-self-test")
        self.assertEqual(parsed.model_dir, Path("/tmp/model"))

    def test_uses_the_documented_fast_cpu_inference_settings(self) -> None:
        self.assertEqual(inference_settings("cpu"), (CPU_SHIFTS, INFERENCE_OVERLAP))
        self.assertEqual(inference_settings("mps"), (ACCELERATOR_SHIFTS, INFERENCE_OVERLAP))
        self.assertEqual(CPU_SHIFTS, 0)
        self.assertEqual(ACCELERATOR_SHIFTS, 1)
        self.assertEqual(INFERENCE_OVERLAP, 0.10)

    def test_maps_convolution_and_mlx_wrapper_names(self) -> None:
        convolution = type("Conv2d", (), {})()
        self.assertEqual(
            mlx_parameter_name("encoder.0.conv.weight", convolution),
            "model_0.encoder.0.conv.conv.weight",
        )
        self.assertEqual(
            mlx_parameter_name("encoder.0.dconv.layers.1.3.weight", convolution),
            "model_0.encoder.0.dconv.layers.1.layers.3.conv.weight",
        )

    def test_maps_attention_and_group_norm_names(self) -> None:
        linear = type("Linear", (), {})()
        self.assertEqual(
            mlx_parameter_name("crosstransformer.layers.0.self_attn.out_proj.weight", linear),
            "model_0.crosstransformer.layers.0.attn.out_proj.weight",
        )
        self.assertEqual(
            mlx_parameter_name("crosstransformer.layers.0.norm_out.weight", linear),
            "model_0.crosstransformer.layers.0.norm_out.gn.weight",
        )

    def test_validates_existing_model_identity_and_digest(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        model_dir = repository / "src-tauri" / "resources" / "models" / "demucs-mlx"
        weights = model_dir / f"{MODEL_NAME}.safetensors"
        if not weights.is_file():
            self.skipTest("converted release model is not present")
        validated, config = validate_model_files(model_dir)
        self.assertEqual(validated, weights)
        self.assertEqual(config["source_artifacts"], MODEL_SOURCE)

    def test_writes_ieee_float_wave(self) -> None:
        try:
            import torch
        except ImportError:
            self.skipTest("Torch is not installed in the lightweight test environment")
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "stem.wav"
            samples = torch.tensor([[0.0, 0.5], [0.25, -0.5]], dtype=torch.float32)
            write_float_wave(destination, samples, 48_000)
            wave = destination.read_bytes()
            self.assertEqual(wave[:4], b"RIFF")
            self.assertEqual(wave[8:12], b"WAVE")
            self.assertEqual(struct.unpack("<H", wave[20:22])[0], 3)
            self.assertEqual(struct.unpack("<I", wave[52:56])[0], 16)


if __name__ == "__main__":
    unittest.main()
