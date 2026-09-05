# Native application menus

SonArcan uses Tauri's native menu layer. Project navigation and application-wide actions do not belong in the workspace toolbar.

## Menu ownership

| Menu | Responsibilities |
|---|---|
| SonArcan | About, Preferences, native application visibility, Quit |
| File | New, Open, Open Recent, Save, Save As, Import Audio, Preferences, Rename Project, Close |
| Edit | Native Undo, Redo, Cut, Copy, Paste, Select All |
| View | Application console, waveform zoom, and native fullscreen |
| Playback | Transport jumps and A/B loop actions |
| Songs | Track import, playlist/stem/chord exports, and current lyrics export |
| Window | Native minimize and window ordering |
| Help | Diagnostics and keyboard shortcuts |

Rust constructs the menu and emits stable action identifiers through the `native-menu` Tauri event. Svelte owns dialogs and workspace behavior, so menu actions and keyboard shortcuts call the same functions.

## Recent projects

Recent project paths are persisted by Rust in the platform configuration directory. The native menu is rebuilt whenever a project is created, opened, or saved as a new package. Missing directories are filtered from the list.

## Standard accelerators

- New Project: `CmdOrCtrl+N`
- Open: `CmdOrCtrl+O`
- Import Audio: `CmdOrCtrl+I`
- Save: `CmdOrCtrl+S`
- Save As: `CmdOrCtrl+Shift+S`
- Preferences: `CmdOrCtrl+,`
