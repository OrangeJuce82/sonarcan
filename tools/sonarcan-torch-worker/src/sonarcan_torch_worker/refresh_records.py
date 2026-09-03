"""Refresh wheel RECORD hashes after Apple code signing changes Mach-O bytes."""

from __future__ import annotations

import base64
import csv
import hashlib
import sys
from pathlib import Path


def file_record(path: Path) -> tuple[str, str]:
    data = path.read_bytes()
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"sha256={digest}", str(len(data))


def refresh(site_packages: Path) -> int:
    runtime_root = site_packages.parents[2].resolve()
    updated = 0
    for record in site_packages.glob("*.dist-info/RECORD"):
        rows: list[list[str]] = []
        with record.open(newline="", encoding="utf-8") as source:
            for row in csv.reader(source):
                if len(row) != 3:
                    raise RuntimeError(f"invalid wheel RECORD row in {record}")
                target = (site_packages / row[0]).resolve()
                if target == record.resolve():
                    rows.append([row[0], "", ""])
                elif target.is_file() and target.is_relative_to(runtime_root):
                    digest, size = file_record(target)
                    rows.append([row[0], digest, size])
                else:
                    rows.append(row)
        temporary = record.with_suffix(".tmp")
        with temporary.open("w", newline="", encoding="utf-8") as destination:
            csv.writer(destination, lineterminator="\n").writerows(rows)
        temporary.replace(record)
        updated += 1
    return updated


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: refresh_records.py SITE_PACKAGES")
    print(f"Refreshed {refresh(Path(sys.argv[1]))} wheel RECORD files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
