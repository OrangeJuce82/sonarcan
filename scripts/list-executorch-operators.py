"""Print the union of ExecuTorch operators used by one or more PTE files."""

from __future__ import annotations

import argparse
import contextlib
import io
from pathlib import Path

from codegen.tools.gen_oplist import _get_kernel_metadata_for_model


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("models", nargs="+", type=Path)
    arguments = parser.parse_args()

    operators: set[str] = set()
    for model in arguments.models:
        if not model.is_file():
            raise FileNotFoundError(f"missing ExecuTorch program: {model}")
        # ExecuTorch's private metadata helper currently prints every operator.
        # Keep stdout machine-readable for the CMake argument generated below.
        with contextlib.redirect_stdout(io.StringIO()):
            operators.update(_get_kernel_metadata_for_model(str(model.resolve())))

    if not operators:
        raise RuntimeError("the supplied ExecuTorch programs use no registered operators")
    print(",".join(sorted(operators)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
