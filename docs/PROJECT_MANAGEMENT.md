# Project management

## Autosave model

SonArcan project metadata is saved immediately after an explicit edit such as renaming a project, renaming a track, or importing media. The current vertical slice therefore does not expose a misleading Save command when there are no pending changes.

## Rename Project

Rename Project updates the human-readable project name in `project.json`. It does not rename the package directory, because changing an open package path can invalidate external references and recent-project entries.

Use Save As when both a new project identity and a new package name are required.

## Rename Track

Track renaming changes the playlist display name only. It does not rename the imported media file. This keeps cache keys and media references stable while allowing musicians to use rehearsal-friendly names.

## Save As

Save As copies the complete `.sac` package to a new destination, creates a new project identifier, changes the project name, and rebases imported media paths to the copied `Audio/` directory. Existing destinations are never overwritten.

## Open Recent

The desktop UI stores up to ten package paths locally. Opening a recent project moves it to the top of the list. A future iteration will add missing-path cleanup and native operating-system recent-document integration.

