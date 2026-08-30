"""Pure transformations used around LV-Chordia's learned outputs.

There is deliberately no tonal prior, beat constraint, chord-neighbour rule,
stem fusion, or family-specific threshold in this module.
"""

from __future__ import annotations

from dataclasses import dataclass
PITCH_NAMES = ("C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B")
FLAT_PITCH_NAMES = ("C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B")
QUALITY_SUFFIX = {
    "Major": "",
    "Minor": "m",
    "Diminished": "dim",
    "Augmented": "aug",
    "Sus2": "sus2",
    "Sus4": "sus4",
}
FLAT_PITCH_CLASS = {"Cb": 11, "Db": 1, "Eb": 3, "Fb": 4, "Gb": 6, "Ab": 8, "Bb": 10}


@dataclass(frozen=True)
class FrameLabel:
    label: str
    family: str
    root: str | None


def pitch_class(note: str) -> int:
    if note in FLAT_PITCH_CLASS:
        return FLAT_PITCH_CLASS[note]
    try:
        return PITCH_NAMES.index(note)
    except ValueError as error:
        raise ValueError(f"invalid chord root: {note}") from error


def reduced_label(raw_label: str) -> FrameLabel:
    """Drop extensions from a dictionary label without changing root/triad."""
    if raw_label in {"N", "X"}:
        return FrameLabel("N", "N", None)
    root, separator, suffix = raw_label.partition(":")
    if not separator:
        raise ValueError(f"invalid LV-Chordia label: {raw_label}")
    quality = suffix.split("/")[0]
    if quality.startswith("min") or quality in {"m", "hdim", "dim7"}:
        family = "Diminished" if quality in {"hdim", "dim7"} else "Minor"
    elif quality.startswith("dim"):
        family = "Diminished"
    elif quality.startswith("aug"):
        family = "Augmented"
    elif quality.startswith("sus2"):
        family = "Sus2"
    elif quality.startswith("sus4") or quality == "sus":
        family = "Sus4"
    else:
        family = "Major"
    return FrameLabel(f"{root}{QUALITY_SUFFIX[family]}", family, root)


def sonarcan_label(raw_label: str) -> str:
    """Convert LV-Chordia's Harte spelling to SonArcan's compact spelling."""
    if raw_label in {"N", "X"}:
        return "N"
    root, separator, suffix = raw_label.partition(":")
    if not separator:
        raise ValueError(f"invalid LV-Chordia label: {raw_label}")
    quality, slash, bass_degree = suffix.partition("/")
    aliases = {
        "maj": "", "min": "m", "hdim7": "m7b5", "hdim": "m7b5",
        "min7": "m7", "min6": "m6", "min9": "m9", "min11": "m11",
        "min13": "m13", "minmaj7": "mmaj7",
    }
    compact = aliases.get(quality, quality)
    if not slash:
        return f"{root}{compact}"
    degree_semitones = {
        "1": 0, "b2": 1, "2": 2, "#2": 3, "b3": 3, "3": 4,
        "4": 5, "#4": 6, "b5": 6, "5": 7, "#5": 8, "b6": 8,
        "6": 9, "#6": 10, "b7": 10, "7": 11,
    }
    if bass_degree not in degree_semitones:
        raise ValueError(f"unsupported LV-Chordia bass degree: {raw_label}")
    spellings = FLAT_PITCH_NAMES if "b" in root else PITCH_NAMES
    bass = spellings[(pitch_class(root) + degree_semitones[bass_degree]) % 12]
    return f"{root}{compact}/{bass}"
