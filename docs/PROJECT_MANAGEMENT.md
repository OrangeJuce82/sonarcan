# Project management

## Project lifecycle

When no usable recent project exists, SonArcan creates a randomly named
temporary `.sac` package below the operating system's temporary directory. New
Project follows the same non-blocking path. Temporary projects are autosaved and
remembered, but the application asks whether to promote them with Save before
closing or quitting. Save copies a temporary project to a user-selected package;
Save As always creates a new package.

## Session restoration

At startup, a native document-open request takes precedence; otherwise SonArcan
tries the latest remembered project. Its remembered playlist track is loaded
into the Rust audio engine without starting playback. Like every other track
load, it starts at zero unless Loop is active and the user preference selects
point A. Restoration is best-effort: when the requested or latest project is
missing, inaccessible, or invalid, SonArcan reports it and opens a fresh
temporary project instead.

## Autosave model

SonArcan project metadata is saved immediately after an explicit edit such as
renaming a project, renaming or reordering a track, importing or deleting media,
editing chords, or changing per-track practice state. The Save command therefore
promotes a temporary package; it is not needed to flush ordinary edits in an
already saved project.

## Rename Project

Rename Project updates the human-readable project name in `project.json`. It does not rename the package directory, because changing an open package path can invalidate external references and recent-project entries.

Use Save As when both a new project identity and a new package name are required.

## Rename Track

Track renaming changes the playlist display name only. It does not rename the imported media file. This keeps cache keys and media references stable while allowing musicians to use rehearsal-friendly names.

Clicking a track title opens an inline editor; Enter or loss of focus saves it and Escape cancels it. Tracks can be reordered with native drag and drop. The backend persists the resulting manifest order immediately, so reopening the project restores the same playlist.

Deleting a track removes its imported media, lyrics, stem directory, waveform,
tempo, decoded-audio, and fingerprint caches after validating that the media is
inside the selected project. The manifest is saved before best-effort cleanup of
disposable caches. The interface selects a neighboring track when the deleted
track was active.

## Save As

Save As copies the complete `.sac` package to a new destination, creates a new project identifier, changes the project name, and rebases imported media paths to the copied `Audio/` directory. Existing destinations are never overwritten, destinations inside the source package are rejected, and a failed copy removes only the incomplete destination created by that operation.

## Exports

The Songs menu exports a playlist as JSON or printable Markdown. For the current
track, it can also export cached six-stem audio, the effective chord timeline as
JAMS, and lyrics as synchronized LRC or Markdown when those data are available.
Exports are separate files and never rewrite the source project.

## Open Recent

Rust stores up to ten canonical package paths in the platform configuration
directory. Opening, creating, saving, or saving as a project moves it to the top
of the list. Missing directories are filtered from the native menu; a stale
startup entry is forgotten before SonArcan creates a fresh temporary project.
Native operating-system recent-document integration is not implemented.
