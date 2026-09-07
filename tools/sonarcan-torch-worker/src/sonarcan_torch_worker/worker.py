from __future__ import annotations

from typing import Sequence

from scnet_infer.sonarcan_worker import run


def main(arguments: Sequence[str] | None = None) -> int:
    return run(arguments, backend="torch", program="sonarcan-torch-worker")


if __name__ == "__main__":
    raise SystemExit(main())
