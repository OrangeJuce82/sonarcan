# yt-dlp search resource

SonArcan uses the official platform-independent `yt-dlp` zipimport artifact for
metadata-only YouTube searches. It starts with the already bundled Python
runtime and avoids the multi-second self-extraction cost of the standalone
macOS executable. Downloads continue to use the standalone release.

`manifest.json` pins the upstream version and SHA-256 digest. Generate the
ignored release artifact with:

```bash
npm run ytdlp:search
```
