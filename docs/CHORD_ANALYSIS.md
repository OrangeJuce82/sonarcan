# Chord analysis

SonArcan uses LV-Chordia as its only automatic chord-recognition engine. There
is no Librosa feature worker, template scorer, tonal correction, stem fusion,
beat/downbeat constraint, or SonArcan harmonic decoder in the production path.

## Production path

```text
original audio
  -> LV-Chordia CQT and five-model learned ensemble
  -> native factor probabilities
  -> official LV-Chordia dictionary decode
  -> bounded timed-chord JSON
  -> Rust validation, cancellation, and versioned cache
  -> interface
```

The Python process owns model inference and the official LV-Chordia HMM
dictionary decode. Rust never changes a chord label. It rejects malformed,
oversized, non-finite, out-of-order, late, or superseded output and never sends
PCM through JSON IPC.

## User modes

- **Essentiel**: official `ismir2017` dictionary.
- **Standard**: official `submission` dictionary and application default.
- **Complet**: official `full` dictionary; exposed as experimental because the
  original research repository calls it untested and not recommended.

Extensions are converted only from Harte notation to compact display notation
(`C:maj7` to `Cmaj7`, `D:min7` to `Dm7`). This does not reinterpret the signal.
`N` remains a machine-readable No Chord and is displayed as `-`.

Each timed segment also retains the original LV-Chordia JAMS/Harte label as
`sourceLabel`. The compact `label` is presentation-only. The shared frontend
parser accepts both spellings, including explicit interval lists, additions,
omissions, alterations, and degree-based slash basses from the complete
dictionary. Piano, guitar, and ukulele views therefore derive from the
same model identity instead of maintaining independent chord-name tables.

The confidence value is the uncalibrated probability of the associated native
triad class. The interface can filter it dynamically, but the filter does not
change cached analysis.

## Runtime and trust boundary

The runtime is pinned to Python 3.12 and LV-Chordia revision
`9d7de7bbf45efa6731ec8dc62d35280f141c0702`. Python 3.13 is not used because
the upstream `pydub` path still imports the removed `audioop` module.

All five pretrained checkpoint files are SHA-256 verified before `torch.load`.
The release runtime is generated with `npm run chords:runtime` and verified by
the Tauri release build. Development uses the same locked `uv` project.

## Presentation

The interface supports vertical timed cards, automatic playback following, a
dynamic confidence filter, colors by confidence or by the 12 roots, and an
alphabetical repertoire of unique chords. Clicking a repertoire chord updates
the selected harmony view without seeking the track. The piano exposes real
close/open inversions over three octaves. Guitar and ukulele positions are
bounded, generated from their standard tunings, and validated against the
parsed pitch set and requested bass; unavoidable omissions are labelled as
adapted rather than silently substituted. The timeline preserves LV-Chordia's native
regions exactly; Beat This! downbeats never split them into extra cards.

User chord corrections are stored separately in the track's bounded practice
state and applied as a presentation overlay for one LV-Chordia vocabulary and
one existing timed region. They never rewrite the disposable analysis cache or
alter a model result. Shift-validation can create the same override for every
region whose currently effective label matches the label selected for editing.
