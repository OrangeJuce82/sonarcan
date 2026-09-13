"""Bounded resident yt-dlp metadata search worker for SonArcan."""

from __future__ import annotations

import json
import sys

MAX_REQUEST_BYTES = 4 * 1024
MAX_ERROR_CHARACTERS = 2 * 1024
SEARCH_RESULT_COUNT = 10


def _compact_entry(entry: object) -> dict[str, object]:
    if not isinstance(entry, dict):
        return {}
    compact: dict[str, object] = {}
    limits = {
        "id": 64,
        "title": 256,
        "channel": 160,
        "uploader": 160,
        "webpage_url": 2048,
        "url": 2048,
    }
    for key, limit in limits.items():
        value = entry.get(key)
        if isinstance(value, str):
            compact[key] = value[:limit]
    view_count = entry.get("view_count")
    if isinstance(view_count, int) and view_count >= 0:
        compact["view_count"] = view_count
    verified = entry.get("channel_is_verified")
    if isinstance(verified, bool):
        compact["channel_is_verified"] = verified
    return compact


def _write(message: dict[str, object]) -> None:
    sys.stdout.write(json.dumps(message, ensure_ascii=False, separators=(",", ":")))
    sys.stdout.write("\n")
    sys.stdout.flush()


def _search(ydl: object, request: object) -> dict[str, object]:
    if not isinstance(request, dict):
        raise ValueError("request must be an object")
    query = request.get("query")
    provider = request.get("provider")
    if not isinstance(query, str) or not query.strip() or len(query.encode()) > 180:
        raise ValueError("invalid search query")
    if provider not in ("ytsearch", "scsearch"):
        raise ValueError("invalid search provider")
    result = ydl.extract_info(
        f"{provider}{SEARCH_RESULT_COUNT}:{query.strip()}", download=False
    )
    entries = result.get("entries", []) if isinstance(result, dict) else []
    return {"ok": True, "entries": [_compact_entry(entry) for entry in entries]}


def _build_ytdlp() -> object:
    import yt_dlp

    return yt_dlp.YoutubeDL(
        {
            "extract_flat": True,
            "extractor_args": {"youtubetab": {"skip": ["webpage"]}},
            "ignoreconfig": True,
            "lazy_playlist": True,
            "no_warnings": True,
            "playlistend": SEARCH_RESULT_COUNT,
            "quiet": True,
        }
    )


def main() -> int:
    ydl = _build_ytdlp()
    _write({"ready": True})
    for raw_line in sys.stdin.buffer:
        if len(raw_line) > MAX_REQUEST_BYTES:
            _write({"ok": False, "error": "search request is too large"})
            continue
        try:
            request = json.loads(raw_line)
            _write(_search(ydl, request))
        except Exception as error:
            message = str(error).strip() or type(error).__name__
            _write({"ok": False, "error": message[:MAX_ERROR_CHARACTERS]})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
