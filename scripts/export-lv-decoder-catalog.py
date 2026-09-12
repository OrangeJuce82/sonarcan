#!/usr/bin/env python3
"""Export LV-Chordia's static HMM dictionaries for the native Rust decoder.

This is a development-only conversion step. The application embeds the small,
validated JSON catalog and never imports LV-Chordia or Python at runtime.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DICTIONARIES = {
    "essential": "ismir2017",
    "standard": "submission",
    "complete": "full",
}
HEAD_WIDTHS = (73, 13, 4, 4, 3, 3)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "src-tauri/src/lv_decoder_catalog.json",
    )
    arguments = parser.parse_args()

    from sonarcan_chord_worker.core import pitch_class, reduced_label, sonarcan_label
    from lv_chordia.extractors.xhmm_ismir import XHMMDecoder
    import importlib.resources

    modes = {}
    for mode, dictionary in DICTIONARIES.items():
        template = importlib.resources.files("lv_chordia.data").joinpath(
            f"{dictionary}_chord_list.txt"
        )
        with importlib.resources.as_file(template) as template_path:
            decoder = XHMMDecoder(template_file=str(template_path))
        chords = []
        for array, raw_label in decoder.known_chord_array:
            if any(int(array[index]) >= width for index, width in enumerate(HEAD_WIDTHS)):
                continue
            reduced = reduced_label(raw_label)
            if reduced.family == "N":
                strength_class = 0
            else:
                quality = (
                    "Major",
                    "Minor",
                    "Sus4",
                    "Sus2",
                    "Diminished",
                    "Augmented",
                ).index(reduced.family)
                strength_class = 1 + quality * 12 + pitch_class(reduced.root)
            chords.append(
                {
                    "array": [int(value) for value in array],
                    "raw": raw_label,
                    "label": sonarcan_label(raw_label),
                    "strengthClass": strength_class,
                }
            )
        modes[mode] = chords

    payload = json.dumps(
        {"formatVersion": 1, "transitionPenalty": 30.0, "modes": modes},
        separators=(",", ":"),
        sort_keys=True,
    ) + "\n"
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(payload, encoding="utf-8")
    print(f"Wrote {len(payload.encode('utf-8'))} bytes to {arguments.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
