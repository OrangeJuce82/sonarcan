# Waveform generation and navigation

Generated peak data is cached in each project under `Analysis/waveform`. Waveforms already viewed during the current session are also retained in a frontend display cache, avoiding disk reads and JSON parsing when switching back to a track. This cache contains visualization peaks only; it never participates in audio playback.

Loop interaction follows a direct gesture hierarchy: grabbing anywhere on the
detailed waveform pans it, clicking the background seeks, and dragging an A/B
flag adjusts that endpoint. The highlighted loop region is visual only and no
longer moves as a block, so A/B resize handles always take priority over waveform
navigation. A/B controls and sliders are mirrored in the detailed and full-song
views. Defining A shows its marker immediately; defining B adds the highlighted
loop region. A and B markers and the region are visible on the waveforms only
when Loop is on. For tracks without saved bounds, the inactive default is A at
the beginning and B at the end of the track.

The `A` and `B` keyboard shortcuts move the corresponding loop point to the
playhead, `L` toggles looping, `M` toggles the metronome, and `Escape` clears
the range. Beat snapping has an explicit button and no keyboard shortcut. `C` toggles the
application console and `H` toggles contextual help. Global shortcuts are
suspended while a dialog is open and never intercept text inputs, text areas,
selects, editable content, IME composition, or modified system shortcuts such
as Command/Ctrl+C. Transport shortcuts also yield to focused buttons and links.

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

- vertical mouse-wheel/trackpad movement or trackpad pinch: zoom around the pointer;
- horizontal two-finger trackpad movement: pan like a waveform grab;
- horizontal drag: navigate through the detailed waveform;
- click: seek to an exact visible position;
- overview background click: seek and recenter the detailed viewport;
- overview viewport drag: move the detailed window without changing its zoom;
- overview viewport edge drag: resize the detailed window and update its zoom;
- overview vertical mouse-wheel/trackpad movement or pinch: zoom around the pointer;
- overview horizontal two-finger trackpad movement: move the detailed viewport.

Wheel gestures lock to their first dominant axis until input pauses briefly, so
a diagonal trackpad gesture cannot pan and zoom at the same time.

Zoom is bounded between `1×` and `128×`. Loop markers share the same time-to-screen transform as the playhead so they remain aligned at every zoom level.

Beat This! timestamps appear as light vertical lines behind the detailed
waveform from `1.5×` zoom onward. Detected downbeats are emphasized. The
full-song overview and the widest detailed view intentionally omit these lines
to avoid visual noise. Beat This! provides beats and downbeats but no reliable
subdivision positions, so SonArcan never invents half-beats from the indicative
BPM.

The optional magnet control snaps A and B to the nearest detected beat when
they are placed from the playhead or dragged on either waveform. Disabling it
restores exact free placement. Snapping is presentation-side interaction only:
it never changes the model timeline, BPM, audio clock, or stored analysis.

The loading state uses a symmetric waveform-shaped SVG with a moving highlight, matching the geometry of the final peak view. Loop overlays use a separate violet palette so the editable range remains visually distinct from the teal source waveform.
