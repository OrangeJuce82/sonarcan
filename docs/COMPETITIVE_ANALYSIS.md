# Competitive analysis and product gap backlog

**Snapshot date:** 2026-08-31  
**Product version reviewed:** `0.1.0-beta.2`  
**Scope:** desktop tools for learning, analysing, isolating, transcribing, and
rehearsing existing music.

Prices and product plans change. The amounts below are the public French App
Store or publisher prices visible on the snapshot date, before any future price
change, promotion, currency conversion, or local tax adjustment.

## Executive summary

SonArcan occupies a credible space between Moises, Capo, and Anytune: it combines
local six-stem separation, timed chord and beat analysis, a real-time practice
player, a synchronized metronome, and a progressive loop trainer in portable
projects. No mature free competitor reviewed here combines that complete
workflow in one application.

Its strongest differentiation is not a single model. It is the combination of:

- local-first processing without an account, upload quota, or subscription;
- six-stem separation and mixing inside the practice session;
- a Rust-owned real-time engine for playback, looping, stretching, pitch,
  metronome, and stem mixing;
- beat-timestamp-driven rhythm views instead of a synthetic constant-BPM grid;
- portable, inspectable `.sac` projects that keep original media separate from
  disposable caches;
- a free MIT-licensed product.

The principal competitive problem is product maturity rather than feature
ambition. SonArcan is an Apple-silicon-only beta distributed without Apple
notarization. It lacks several mature practice and transcription workflows:
named markers and sections, editable chords, multiple saved practice regions,
count-in, synchronized notes or lyrics, hands-free control, and broader platform
support. Its model quality also has not yet been demonstrated in a reproducible
head-to-head benchmark.

The recommended strategy is therefore to make the existing differentiation
trustworthy and comfortable before adding more analysis models.

## Paid competitors

| Product | Public price on snapshot date | Free access | Principal advantage over SonArcan |
| --- | --- | --- | --- |
| **Moises** | Premium: **EUR 6.99/month** or **EUR 49.99/year**; Pro: **EUR 34.99/month** | Limited free plan | Cross-platform cloud library, mobile apps, automatic sections, lyrics, collaboration, mastering, and a broader creator ecosystem |
| **Capo for Mac** | Pro: **EUR 6.99/month** or **EUR 39.99/year**; Platinum Mac + iOS: **EUR 49.99/year** | Core analysis and playback are free, but edits are not fully saved | Editable chords and beats, instrument chord diagrams, named regions, visual note transcription, MIDI export, and mature ear-learning workflow |
| **Capo touch** | Pro: **EUR 5.99/month** or **EUR 19.99/year** | Limited free edition | iPhone and iPad practice plus project synchronization |
| **Anytune for Mac** | **EUR 39.99 one-time** | Trial and a separate freemium mobile edition | Mature markers, setlists, lyrics and tabs, progressive training, MIDI control, live input, and stage workflow |
| **Transcribe!** | **USD 39 one-time**, plus applicable taxes | Full 30-day evaluation | Mature, lightweight Windows/macOS/Linux transcription workflow and broad hardware support |
| **RipX DAW** | **USD 99 one-time** | Full 21-day trial | Six-plus-stem separation and note-level editing of mixed audio |
| **RipX DAW Pro** | **USD 198 one-time** | Full 21-day trial | Production integrations and deeper audio editing |
| **SonArcan** | **Free** | Complete product | Local-first integrated analysis and rehearsal with no subscription or account |

Sources:

- [Moises App Store listing](https://apps.apple.com/fr/app/moises-lappli-du-musicien/id1515796612)
- [Moises plan capabilities](https://play.google.com/store/apps/details?id=ai.moises)
- [Capo for Mac App Store listing](https://apps.apple.com/fr/app/capo-3/id696977615)
- [Capo touch App Store listing](https://apps.apple.com/fr/app/capo-touch/id887497388)
- [Anytune for Mac App Store listing](https://apps.apple.com/fr/app/anytune-pratique-perfectionn%C3%A9/id722444976?mt=12)
- [Transcribe! pricing](https://www.seventhstring.com/xscribe/buy.html)
- [RipX pricing](https://hitnmix.com/buy-ripx-wp/)

At constant prices, three years of Moises Premium or Capo Platinum cost
EUR 149.97, while Capo Pro for Mac costs EUR 119.97. Anytune and Transcribe!
remain inexpensive one-time purchases, so SonArcan should not market itself only
as an alternative to subscriptions. Privacy, integration, project ownership,
and the six-stem practice workflow are more durable differentiators than price.

## Competitor-by-competitor assessment

### Moises

Moises is the closest commercial all-in-one competitor. It offers stem
separation, chords, tempo and pitch controls, smart metronome, loops, detected
sections, setlists, export, and cloud synchronization across desktop, web, and
mobile devices. Paid tiers add more stem choices and higher-fidelity models.

SonArcan is stronger when the user wants local processing, inspectable project
data, no upload or subscription constraint, sample-owned playback, and an
integrated progressive loop trainer. Moises is stronger in accessibility,
cross-device continuity, lyrics, automatic song structure, collaboration,
support, and general product maturity.

### Capo

Capo is the closest competitor for learning by ear. It detects beats, chords,
and key; lets users edit the musical result; provides chord shapes for several
string instruments; supports named regions, count-in, metronome, note
annotations over a spectrogram, and MIDI export.

SonArcan is stronger in true six-stem separation, real-time stem mixing,
progressive loop training, local project transparency, and price. Capo is
stronger for a guitarist or teacher who needs to correct, annotate, and export a
musical interpretation rather than only inspect model output.

### Anytune

Anytune has a mature rehearsal workflow: markers, loops, setlists, lyrics and
tabs, MIDI or Bluetooth control, live instrument input, export, and progressive
tempo training. Its ReFrame isolation is based on stereo position and frequency
filtering and is not equivalent to SonArcan's six learned stems.

SonArcan has the stronger isolation and analysis proposition. Anytune remains
stronger for hands-free rehearsal, stage use, annotation, and cross-device
practice.

### Transcribe!

Transcribe! is deliberately narrower. It provides slow-down, pitch adjustment,
loops, spectrum and keyboard-assisted manual transcription on Windows, macOS,
and Linux. SonArcan automates more of the workflow, but Transcribe! is mature,
lightweight, inexpensive, and available on far more computers.

### RipX

RipX is an AI audio editor rather than a focused practice player. It offers
six-plus-stem separation and note-level manipulation that SonArcan should not
attempt to reproduce without a clear practice use case. SonArcan is simpler,
free, and better focused on learning and rehearsal; RipX is much stronger for
remixing and production.

## Free alternatives

There is no single mature free application in this review that replaces all of
SonArcan. A user can approximate the workflow with several tools:

| Free tool | Best use | Important limitation compared with SonArcan |
| --- | --- | --- |
| [Moises Free](https://play.google.com/store/apps/details?id=ai.moises) | Easy cloud stems, chords, BPM, metronome, and setlists | Usage and stem choices are limited; processing and library depend on the service |
| [Capo Free](https://apps.apple.com/fr/app/capo-3/id696977615) | Chord, beat, and key assistance with high-quality slow-down | Does not fully save edits without a subscription; no learned six-stem mixer |
| [RepShed](https://repshed.com/) | Browser-based A/B looping and progressive speed training | No stems, chords, metronome analysis, or durable project package |
| [StemRoller](https://www.stemroller.com/) | Simple local Demucs four-stem separation with YouTube search | No integrated practice, chord, or real-time mixer workflow |
| [Ultimate Vocal Remover](https://github.com/HundredBillion/UltimateVocalRemover) | Advanced local separation with a choice of model families | Technical interface and installation; produces files rather than a practice session |
| [Sonic Visualiser](https://sonicvisualiser.org/) | Detailed visualization, annotation, and plugin-based musical analysis | Analysis-oriented interface; no stem separation or focused loop trainer |
| [Audacity](https://www.audacityteam.org/features/) | Free audio preparation, editing, pitch/tempo changes, and export | Not designed as a seamless rehearsal player; traditional looping remains limited |
| [EarCopy](https://apps.apple.com/us/app/earcopy-music-practice/id6759784749) | Free iPhone/iPad slow-down, pitch, waveform, and A/B loop | No stems, chords, beat analysis, or desktop project workflow |

The strongest free substitute is therefore a stack: Ultimate Vocal Remover or
StemRoller for stems, Sonic Visualiser for analysis, RepShed for practice, and
Audacity for editing or export. SonArcan's advantage is eliminating the manual
exports, alignment, and context switching between those applications.

## Competitive capability map

Legend: **Yes** = mature advertised capability; **Partial** = limited or
different implementation; **No** = not currently part of the reviewed product.

| Capability | SonArcan | Moises | Capo | Anytune | Transcribe! | RipX |
| --- | --- | --- | --- | --- | --- | --- |
| Local six-stem separation | Yes | No, service-backed | No | No | No | Yes |
| Stem mixer in the learning workflow | Yes | Yes | Partial | Partial | No | Yes |
| Timed chord detection | Yes | Yes | Yes | No | Manual aid | Partial |
| Edit detected chords | No | Partial | Yes | No | Manual notes | Yes, note-level |
| Beat/downbeat-synchronized metronome | Yes | Yes | Yes | Partial | No | Partial |
| Seamless A/B loop | Yes | Yes | Yes | Yes | Yes | Yes |
| Progressive loop trainer | Yes | Partial | No | Yes | Scriptable/partial | Partial |
| Named markers and song sections | No | Yes | Yes | Yes | Yes | Yes |
| Multiple saved practice regions | No | Yes | Yes | Yes | Yes | Yes |
| Count-in or loop restart delay | No | Yes | Yes | Yes | Partial | Yes |
| Lyrics, tabs, or timed text notes | No | Yes | Yes | Yes | Yes | Partial |
| MIDI or foot-controller operation | No | No | Partial | Yes | Partial | Yes |
| Live instrument input or take recording | No | Partial | No | Yes | No | Yes |
| Chord chart, MIDI, or printable export | No | Partial | Yes | No | Text/manual | Yes |
| Mobile companion | No | Yes | Yes | Yes | No | Limited |
| Windows support | No | Yes | No | Partial | Yes | Yes |
| Linux support | No | No | No | No | Yes | No |
| Account-free local projects | Yes | No | Yes | Yes | Yes | Partial |
| Notarized mainstream installation | No | Yes | Yes | Yes | Yes | Yes |

This map compares workflows, not audio or model quality. No claim that one stem,
chord, beat, or time-stretch implementation sounds better than another should be
made until a controlled benchmark exists.

## Prioritized product gap backlog

The backlog follows the repository priority order: stabilize, clean, simplify,
extract, harmonize, then optimize. Priority reflects competitive impact and
product risk, not implementation novelty.

### P0 — make the existing value trustworthy

- [ ] **Ship a Developer ID-signed and Apple-notarized release.**
  - Remove the Gatekeeper workaround from normal installation.
  - Verify the exact distributed DMG and every bundled worker/runtime on a clean
    supported Mac.
  - Make upgrade, downgrade, uninstall, and retained-project behavior explicit.
  - Competitive reason: every established paid competitor provides a normal,
    trusted installation path.

- [ ] **Complete crash-safe project recovery and backups.**
  - Recover the last durable project state after a process or machine failure.
  - Never replace an intact project with a partial manifest or partial import.
  - Expose recovery clearly instead of silently changing user data.
  - Competitive reason: portable local projects are only an advantage if users
    can trust them with a working repertoire.

- [ ] **Finish retry, pruning, and queuing for background jobs.**
  - Keep the existing import cancellation and supervised analysis termination
    behavior covered by integration tests.
  - Add bounded retry and completed-import pruning.
  - Queue separation requests safely across track switches.
  - Resume or discard interrupted work deterministically.
  - Competitive reason: cloud competitors hide job supervision; a local product
    must make expensive operations equally predictable.

- [ ] **Publish a reproducible quality and performance benchmark.**
  - Use a legally distributable corpus covering dense rock, acoustic music,
    electronic music, live tempo drift, detuned recordings, and short loops.
  - Measure stem reconstruction/separation metrics where references exist,
    chord and beat accuracy, cold/warm latency, peak memory, and audio dropouts.
  - Include supported Apple-silicon generations and record the exact versions,
    settings, and hardware.
  - Add a small blinded musician evaluation for slow-down, pitch, loops, and
    separated practice usefulness.
  - Competitive reason: model names do not demonstrate superiority over Moises,
    Capo, UVR, or RipX.

- [ ] **Reconcile documentation and product state.**
  - Split completed chord analysis from the still-missing chord editor.
  - Mark implemented import cancellation, track deletion, and playlist
    reordering accurately.
  - Keep supported formats, platforms, release trust, and roadmap checkboxes
    synchronized with each release.

### P1 — reach practice-workflow parity without losing focus

- [ ] **Add named markers, sections, and saved practice regions.**
  - Support instant markers and A/B regions with name, color, and optional notes.
  - Allow several non-overlapping or overlapping regions per track.
  - Show them consistently in detailed and overview waveforms.
  - Jump to previous/next marker and select a region from the keyboard.
  - Preserve the current single A/B interaction as the zero-configuration path.
  - Competitive reason: Capo, Anytune, Transcribe!, and RipX all preserve more
    than one place of interest per song.

- [ ] **Add count-in and an optional loop restart delay.**
  - Count detected beats/downbeats before playback or a selected practice region.
  - Keep count-in and silence scheduling in the Rust engine.
  - Allow the progressive trainer to use the same options.
  - Competitive reason: musicians need time to put their hands back on the
    instrument; Capo, Moises, and Anytune already address this.

- [ ] **Make chord analysis correctable and durable.**
  - Edit, insert, delete, merge, and split timed chord segments.
  - Preserve the untouched model result separately from user corrections.
  - Allow song-key, concert-pitch, spelling, and time-signature overrides.
  - Reproject chord display after pitch changes without rewriting source truth.
  - Ensure a model refresh never destroys user-authored corrections.
  - Competitive reason: Capo's practical advantage is not only detection, but
    the ability to turn imperfect detection into a trusted learning document.

- [ ] **Add useful practice exports.**
  - Export the current stem mix with tempo, pitch, metronome, and count-in as
    explicit opt-in choices.
  - Export markers, sections, chords, and beats in a documented open format.
  - Add MIDI chord/beat export and a printable/PDF chord chart only after the
    editable musical data model is stable.
  - Competitive reason: Capo, Moises, Anytune, and RipX let users carry results
    into lessons, notation tools, a DAW, or the stage.

### P2 — support serious rehearsal and teaching

- [ ] **Add keyboard-remappable MIDI and foot-controller commands.**
  - Cover play/pause, jumps, marker navigation, set A/B, loop toggle, trainer,
    speed, and next track.
  - Keep accessibility and ordinary keyboard shortcuts fully usable without a
    controller.
  - Competitive reason: hands-free operation is one of Anytune's strongest
    practical advantages.

- [ ] **Add synchronized lyrics, text tabs, and timed notes.**
  - Start with user-authored plain text and timestamped notes.
  - Keep online lyric lookup optional and separate from local project behavior.
  - Avoid turning the feature into a proprietary tablature marketplace.

- [ ] **Add an optional live-practice input and take recorder.**
  - Mix a selected input with playback without sending it through JSON IPC.
  - Record a bounded practice take outside the real-time callback.
  - Make latency, monitoring, permissions, and feedback risks explicit.
  - Competitive reason: Anytune and RipX let users hear or capture themselves
    against the source; this is valuable, but lower priority than core workflow.

- [ ] **Add a model and cache manager.**
  - Show installed model revisions, disk usage, compatibility, and cache impact.
  - Let users remove disposable analysis/stem caches safely.
  - Add alternative models only when benchmark results demonstrate a useful
    quality, speed, or memory tradeoff.

- [ ] **Add audio-device diagnostics and controlled device switching.**
  - Report sample rate, channel layout, callback errors, underruns, and recovery.
  - Restart the engine safely when the selected output changes.
  - Competitive reason: a rehearsal tool must recover predictably in studios,
    classrooms, and interfaces with changing devices.

### P3 — broaden reach after the Mac product is dependable

- [ ] **Package Windows and Linux builds.**
  - Preserve the same Rust audio and project boundaries.
  - Select and benchmark a non-MLX separation runtime rather than pretending the
    Apple-specific worker is portable.
  - Resolve the reviewed Linux dependency maintenance notices before release.
  - Start with playback/practice parity, then qualify model features explicitly.

- [ ] **Design an optional mobile companion, not a mandatory cloud service.**
  - Prioritize playback of prepared projects, markers, sections, chords, and
    practice settings.
  - Keep local file transfer possible without an account.
  - Treat encrypted synchronization as an optional later service with clear
    ownership, deletion, conflict, and privacy semantics.

- [ ] **Add explicit project interchange and collaboration.**
  - Stabilize relative paths, relinking, migrations, and safe archive import
    before advertising project sharing.
  - Preserve user-authored edits when two project versions are reconciled.

## Deliberate non-goals

The following competitor features should not enter the backlog without a new,
evidence-backed product decision:

- full DAW or RipX-style note-level spectral editing;
- AI mastering, voice cloning, or generative music;
- a compulsory cloud library or account;
- DRM or streaming-platform circumvention;
- social feeds, content marketplaces, or unrelated lesson subscriptions;
- a model selector that exposes complexity without a measured user benefit.

These features consume substantial product and security surface while weakening
SonArcan's current promise: a focused, local workspace to understand and
rehearse music.

## Recommended delivery order

1. Notarized release, recovery, job lifecycle, and honest benchmarks.
2. Markers/sections/regions, count-in, and complete playlist maintenance.
3. Editable chords and durable user annotations.
4. Practice, chord, beat, and mix exports.
5. MIDI/foot control, synchronized text, and device diagnostics.
6. Optional live input and practice recording.
7. Windows/Linux packaging, then a deliberately scoped mobile companion.

This order closes the largest competitive gaps while protecting real-time audio,
project safety, privacy, accessibility, and the focused non-DAW positioning.
