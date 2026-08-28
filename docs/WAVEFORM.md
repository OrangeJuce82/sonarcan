# Waveform generation and navigation

Generated peak data is cached in each project under `Analysis/waveform`. Waveforms already viewed during the current session are also retained in a frontend display cache, avoiding disk reads and JSON parsing when switching back to a track. This cache contains visualization peaks only; it never participates in audio playback.

Loop interaction follows a direct gesture hierarchy: dragging the waveform background pans it, clicking the background seeks, dragging an A/B flag adjusts that endpoint, and dragging the highlighted region moves the complete loop. No intermediate selection mode is required. A/B controls and sliders are mirrored in the detailed and full-song views.

The `A` and `B` keyboard shortcuts always move the corresponding loop point to the playhead, including after a toolbar button has received focus. `L` toggles looping and `Escape` clears the range. Text inputs remain exempt so project and track names can be edited normally.

## Generation

The Rust backend decodes the imported WAV, MP3, or FLAC media with Symphonia. It reduces decoded samples into signed minimum/maximum peak pairs rather than sending raw audio to the frontend. A waveform contains at most 32,768 peak pairs.

Generation runs as a background task invoked when a track is selected. Playback remains independent from the waveform job.

## Cache

Generated data is stored at:

```text
Project.sac/Analysis/waveform/<track-id>.json
```

The cache includes a cache-format version and track identifier. Invalid or outdated cache data is ignored and regenerated. Writes use a temporary sibling followed by an atomic rename.

The next cache revision will include a source-media fingerprint so externally modified or replaced files are detected automatically.

## Views

The detailed view renders only the visible peak range, reduced to a bounded number of SVG lines. The overview renders the complete song and displays both the current playhead and detailed viewport.

Interactions:

- mouse wheel or trackpad pinch: zoom around the pointer;
- horizontal drag: navigate through the detailed waveform;
- click: seek to an exact visible position;
- overview click: seek and recenter the detailed viewport.

Zoom is bounded between `1×` and `128×`. Loop markers share the same time-to-screen transform as the playhead so they remain aligned at every zoom level.

When a track has a grid BPM, both waveform levels render source-time beat lines. Every fourth beat is emphasized. The grid is derived only from numeric timing metadata received from Rust; no audio samples or audio processing enter the frontend. Users can place beat one at the playhead and nudge the complete grid in 10 ms increments.

The loading state uses a symmetric waveform-shaped SVG with a moving highlight, matching the geometry of the final peak view. Loop overlays use a separate violet palette so the editable range remains visually distinct from the teal source waveform.
