# yt-dlp search resource

SonArcan uses the official platform-independent `yt-dlp` zipimport artifact for
metadata-only provider searches and remote downloads. Search runs through a
bounded two-process resident Python worker pool, so repeated queries reuse the
loaded yt-dlp modules. The worker skips YouTube's preliminary webpage request
and returns only the metadata fields used for display and local ranking. The
standalone executable remains a compatibility fallback.

`manifest.json` pins the upstream version and SHA-256 digest. Generate the
ignored release artifact with:

```bash
npm run ytdlp:search
```
