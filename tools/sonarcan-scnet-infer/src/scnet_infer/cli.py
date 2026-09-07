"""Command-line adapter over the public one-shot facade.

The CLI contains no inference implementation; it writes each returned stem as a
float WAV in the requested directory.
Reads: scnet_infer.api and soundfile.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import soundfile as sf

from .api import separate


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="scnet-infer", description="Separate music into SCNet stems")
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--model", dest="model_id")
    parser.add_argument("--device", default="auto")
    parser.add_argument("--backend", default=None, choices=["torch", "mlx", "auto"])
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--cache-dir", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    result = separate(
        args.input,
        model_id=args.model_id,
        device=args.device,
        backend=args.backend,
        checkpoint_path=args.checkpoint,
        cache_dir=args.cache_dir,
    )
    args.output.mkdir(parents=True, exist_ok=True)
    for name, audio in result.stems.items():
        sf.write(args.output / f"{name}.wav", audio.T, result.sample_rate, subtype="FLOAT")
    return 0
