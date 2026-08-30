# Chord analysis

SonArcan treats automatic chord estimation as a sequence-recognition problem,
not as a call to a chord-naming function. Librosa supplies bounded signal
observations; the Rust engine owns vocabulary, harmonic priors, temporal
decoding, confidence policy, and final labels.

## Current front end

- Apply harmonic/percussive separation with a strong margin to reject attacks.
- Fuse 36-bin CQT chroma with CENS chroma. CENS contributes robustness to
  dynamics, timbre, and articulation; the higher-resolution CQT preserves more
  pitch detail.
- Segment on smoothed harmonic novelty, not on a visible beat grid.
- Measure bass separately and use it as supporting evidence, not as the chord
  root oracle.
- When validated stems exist, analyse `other + guitar + piano`, analyse `bass`
  separately, and omit `vocals` and `drums`. Otherwise use the enhanced mix.
- Preserve silence, ambiguity, global key, and several candidates per segment.

The simple vocabulary (major, minor, diminished, half-diminished, augmented)
and complete vocabulary have separate Viterbi decodes. This prevents an
uncertain extended label from determining the basic chord before presentation.

## Why this architecture

Librosa intentionally provides feature extraction, decomposition, segmentation,
and sequence primitives rather than a trained, production chord recognizer.
Its enhanced-chroma example combines strong-margin HPSS with temporal
filtering, while CENS is designed for invariance to dynamics, timbre, and
articulation. These are appropriate observations, but template scoring alone
cannot learn the harmonic language and duration statistics found in annotated
music.

Source separation is useful when its cache already exists because it removes
two major interferers: drums and lead vocals. It is not mandatory: separation
has a material compute cost and can introduce artifacts. SonArcan therefore
uses stems opportunistically, fingerprints the observation source in the cache,
and retains an enhanced-mix fallback.

## Professional validation path

1. Build a reproducible evaluation corpus from public Isophonics annotations,
   synthetic inversions, and separately licensed/user-annotated mixes.
2. Report weighted chord-symbol recall under both the simple and complete
   vocabularies, root/quality/bass accuracy, segmentation over/under-segmentation,
   `N` rate, false changes per minute, latency, peak memory, and stem/mix deltas.
3. Tune thresholds and calibrate confidence only on a development split. Keep a
   song-disjoint test split and publish regression fixtures for every fixed bug.
4. Benchmark the current CQT/CENS templates against an NNLS-chroma front end and
   a context model such as the bi-directional Transformer for chord recognition.
5. Adopt a learned model only if it wins the real-mix benchmark with acceptable
   model size, runtime, redistribution licence, and deterministic fallback.
6. Replace the current framewise Viterbi duration preference with a semi-Markov
   duration model and explicit modulation state after the corpus can measure the
   change. Do not tune this by listening to a single song.

## Research references

- Librosa, [Enhanced chroma and chroma variants](https://librosa.org/doc/latest/auto_tutorials/03-advanced/plot_chroma.html).
- Meinard Müller and Sebastian Ewert, [Towards Timbre-Invariant Audio Features for Harmony-Based Music](https://www.audiolabs-erlangen.de/resources/MIR/chromatoolbox), IEEE TASLP 2010.
- Matthias Mauch and Simon Dixon, [Approximate Note Transcription for the Improved Identification of Difficult Chords](https://archives.ismir.net/ismir2010/2010_ISMIR_Proceedings.pdf#page=147), ISMIR 2010.
- Jongho Park et al., [A Bi-Directional Transformer for Musical Chord Recognition](https://archives.ismir.net/ismir2019/paper/000075.pdf), ISMIR 2019.
- Filip Korzeniowski and Gerhard Widmer, [Improved Chord Recognition by Combining Duration and Harmonic Language Models](https://archives.ismir.net/ismir2018/paper/000300.pdf), ISMIR 2018.
- Centre for Digital Music, [Isophonics reference annotations](https://isophonics.net/content/reference-annotations.html).
