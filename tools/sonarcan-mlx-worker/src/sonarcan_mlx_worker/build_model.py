"""Release-only conversion of the pinned official Demucs model."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any


def without_training_metadata(package: dict[str, Any]) -> dict[str, Any]:
    """Drop unused training metadata rejected by demucs-mlx 1.4.6.

    Model class, constructor arguments, state tensors and official source
    identity remain validated by demucs-mlx's restricted loader.
    """
    sanitized = dict(package)
    sanitized.pop("training_args", None)
    return sanitized


def build(output_dir: Path) -> None:
    from demucs_mlx import secure_demucs
    from demucs_mlx.mlx_convert import convert_htdemucs_weights

    original_validate = secure_demucs._validate_package

    def validate_without_training_args(package: dict[str, Any], torch: Any) -> dict[str, Any]:
        return original_validate(without_training_metadata(package), torch)

    secure_demucs._validate_package = validate_without_training_args
    try:
        convert_htdemucs_weights("htdemucs_6s", output_dir=str(output_dir), verbose=True)
    finally:
        secure_demucs._validate_package = original_validate


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    build(args.output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
