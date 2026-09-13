import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
WORKER_PATH = ROOT / "src-tauri/resources/ytdlp-search/search_worker.py"
SPEC = importlib.util.spec_from_file_location("sonarcan_ytdlp_worker", WORKER_PATH)
WORKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WORKER)


class FakeYtDlp:
    def __init__(self):
        self.calls = []

    def extract_info(self, query, download=False):
        self.calls.append((query, download))
        return {
            "entries": [
                {
                    "id": "video-id",
                    "title": "x" * 300,
                    "channel": "Artist",
                    "view_count": 123,
                    "channel_is_verified": True,
                    "formats": ["must not cross the worker protocol"],
                }
            ]
        }


class WorkerTests(unittest.TestCase):
    def test_search_returns_only_bounded_metadata_used_by_the_app(self):
        ydl = FakeYtDlp()

        response = WORKER._search(
            ydl, {"query": "Artist Song", "provider": "ytsearch"}
        )

        self.assertEqual(ydl.calls, [("ytsearch10:Artist Song", False)])
        self.assertTrue(response["ok"])
        self.assertEqual(len(response["entries"][0]["title"]), 256)
        self.assertNotIn("formats", response["entries"][0])
        self.assertEqual(response["entries"][0]["view_count"], 123)
        self.assertTrue(response["entries"][0]["channel_is_verified"])

    def test_search_rejects_unknown_provider_without_calling_ytdlp(self):
        ydl = FakeYtDlp()

        with self.assertRaisesRegex(ValueError, "invalid search provider"):
            WORKER._search(ydl, {"query": "Song", "provider": "unknown"})

        self.assertEqual(ydl.calls, [])


if __name__ == "__main__":
    unittest.main()
