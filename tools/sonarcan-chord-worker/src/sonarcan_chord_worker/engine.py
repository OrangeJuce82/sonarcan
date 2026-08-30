"""Pinned LV-Chordia inference helpers used by the production worker."""

from __future__ import annotations

import hashlib
import importlib.resources
from pathlib import Path

import numpy as np

from .core import pitch_class, reduced_label

FACTOR_NAMES = ("triad", "bass", "seventh", "ninth", "eleventh", "thirteenth")
DICTIONARIES = {"essential": "ismir2017", "standard": "submission", "complete": "full"}
SOURCE_REVISION = "9d7de7bbf45efa6731ec8dc62d35280f141c0702"
CHECKPOINT_SHA256 = {
    "s0": "921b42d5d1cf9ce1c0c0e45a74d409b8066e0acec46058ef74e24ee0fb540761",
    "s1": "bcb75859e0efa256696cf5da396b320093317b9b1d9560c304f46c25fe1f8b17",
    "s2": "acddf85c3fff29954c4877021177d72e2cba9f729ce80c1010f054c477bf3f61",
    "s3": "65d81a3ab73435aaaade586981b4cabdf57b8953d76052703e6968c32ef8421c",
    "s4": "5ff6b0ec85640e17a09a9b3de68c93fdd45adc24488e8fa9be5715c28d561122",
}


def verify_checkpoints() -> None:
    from lv_chordia.config import model_names
    from lv_chordia.mir.common import CACHE_DATA_PATH

    for index, model_name in enumerate(model_names()):
        checkpoint = Path(CACHE_DATA_PATH) / f"{model_name}.sdict"
        digest = hashlib.sha256(checkpoint.read_bytes()).hexdigest()
        if digest != CHECKPOINT_SHA256[f"s{index}"]:
            raise RuntimeError(f"LV-Chordia checkpoint s{index} failed SHA-256 verification")


def dictionary_decode(entry, probabilities, dictionary: str) -> list[dict]:
    from lv_chordia.extractors.xhmm_ismir import XHMMDecoder

    template = importlib.resources.files("lv_chordia.data").joinpath(f"{dictionary}_chord_list.txt")
    with importlib.resources.as_file(template) as template_path:
        decoder = XHMMDecoder(template_file=str(template_path))
    decoded = decoder.decode_to_chordlab(entry, probabilities, False, use_beats=False)
    frame_seconds = entry.prop.hop_length / entry.prop.sr
    triad = probabilities[0]
    result = []
    for start, end, raw_label in decoded:
        reduced = reduced_label(str(raw_label))
        first = max(0, int(round(start / frame_seconds)))
        last = min(triad.shape[0], max(first + 1, int(round(end / frame_seconds))))
        if reduced.family == "N":
            score = float(np.mean(triad[first:last, 0]))
        else:
            quality = ("Major", "Minor", "Sus4", "Sus2", "Diminished", "Augmented").index(reduced.family)
            score = float(np.mean(triad[first:last, 1 + quality * 12 + pitch_class(reduced.root)]))
        result.append({"rawLabel": str(raw_label), "startSeconds": round(float(start), 6), "endSeconds": round(float(end), 6), "strength": score})
    return result
