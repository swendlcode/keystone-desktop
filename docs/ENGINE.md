# Keystone — analysis engine plan

The build contract for the analysis core. Read `SPEC.md` first for what the product
does; this file says how the engine does it, in the order it has to be built.

Four research passes fed this. Where they disagreed, the call is made here and the
reason is given in one sentence. Where a number is genuinely unknown, it says so
instead of guessing.

**Licence rule, repeated because it drives half the decisions below:** Keystone ships
under MIT. MIT / Apache-2.0 / BSD only, plus MPL-2.0 which is file-level copyleft and
reaches only the dependency's own files. No GPL, no AGPL, no CC-BY-NC, no unlicensed
code. Everything runs locally; nothing in the analysis path touches a network.

---

## 1. The bar, stated so it cannot be fudged

`SPEC.md` says "Mixed In Key measures around 76% agreement". That figure is real but
it is a **MIREX weighted score**, not exact-match accuracy, and the distinction is the
whole argument.

On **GiantSteps Key** (the EDM benchmark where commercial tools have actually been
scored independently):

| System | Exact | MIREX weighted |
|---|---|---|
| **Mixed In Key 8.3** | **69.37%** | **75.70** |
| AllConv / KeyNet (Korzeniowski 2018) | 67.9% | 74.6 |
| Mixed In Key 7 | 67.2% | 74.2 |
| keynet.pt (MIT, trainable) | 66.72% | 73.51 |
| bgate (template, multi-profile) | 64.1% | 72.5 |
| S-KEY (Deezer, MIT) | — | 72.1 |
| KeyFinder | 60.4% | 69.9 |
| rekordbox 7.12 | 56.79% | 65.53 |

MIK 7 was measured by Faraldo (AES 2017); MIK 8.3 and rekordbox 7.12 by the
MusicalKeyCNN repo. Two independent measurements of MIK five years apart agree within
2 points, which is unusually good evidence.

**The number to beat is 75.70 MIREX weighted / 69.37% exact on GiantSteps.**

One researcher reported finding no primary source for the 76% claim and put state of
the art at 74.3; that pass simply had less of the literature in hand, and the two
independent MIK measurements settle it. Two traps follow from this table:

- **MIK's marketed 85.2% counts relative major/minor as correct and drops tracks every
  app failed.** Relative confusion is the hardest error class. 85.2% is not the bar.
- **Serato and Traktor have no published MIREX-scored number on any public dataset.**
  The circulating figures (Traktor 75.4%, Serato 64.2%) come from the same
  relative-counts-as-correct community test. Do not quote them as comparable.

Nothing openly licensed beats 75.70 today. S-KEY, the best shippable model, is 3.6
points short. §7 says exactly how we close that and how we measure whether we did.

---

## 2. The pipeline, end to end

One decode feeds everything. Decode is roughly 90% of the per-track cost, so the only
optimisation that matters at this level is not decoding twice.

Per-track wall-clock budget, 5-minute track, measured or extrapolated as marked:

| Stage | Cost | Basis |
|---|---|---|
| decode + downmix + resample | 0.3–1.8 s | ffmpeg measured 0.43 s CPU on a 197 s 320k MP3; symphonia is pure Rust and typically 2–4× off ffmpeg's MP3 path, so the upper figure is an **extrapolation, not a measurement** |
| tuning + HPCP + key template | ~0.15 s | ~870 FFTs of 4096 |
| S-KEY inference (ONNX, 1 thread) | ~1.3 s | measured, 4.1 ms per audio-second on an M2 Max |
| onset front end (100 fps, 64 bands) | 0.24 s | measured on a 6-min track |
| tempogram | 5 ms | measured |
| mel + SSM + novelty, 7 features | ~0.6 s | 156 ms mel measured; HPSS roughly triples it |
| cue selection | <10 ms | measured, 1.2 ms per feature at 384 strong beats |
| **total, no neural beat tracker** | **~2.5–4 s CPU** | |

At 8 workers that is ~0.4 s wall per track: **5,000 tracks ≈ 35 min, 20,000 ≈ 2.3 h.**
All timings are from an Apple M2 Max with 8 performance cores. **There is no Windows
measurement**, and Windows is roughly two thirds of installs — re-measure before any
progress estimate is shown to a user.

### S0 — Walk and identity
**In:** watched roots. **Out:** one row per audio file, `index_status='pending'`.

`id = xxh3(path)`. Per-path identity, exactly as Stack's migration 008: content-hash
identity made byte-identical files at different paths collapse into one row and
ping-pong on every reconcile. DJ libraries are more duplicate-heavy than sample
libraries (the same track as 320k MP3 and as AIFF, promo copies, folder-per-gig), so
this matters more here, not less.

**`content_hash` must hash the audio data region, not the file bytes.** Stack's
`hasher.rs` hashes size + first 256 KB + last 256 KB of the *file*, and ID3 sits at
the head — so Keystone's own re-tag pass would change every hash and wipe its own
analysis cache. Hash `(codec, sample_rate, n_frames, first 256 KB from the first audio
packet offset, last 256 KB)`. Get this right before the cache is built; it will look
like it works until the first re-tag.

### S1 — Decode
**In:** path. **Out:** mono f32 at 44,100 Hz and at 22,050 Hz, plus true-peak, clipped-
sample count, and streamed waveform peaks at each cached zoom level.

symphonia, arithmetic-mean downmix, rubato for the resample. Downsample for peaks
*inside* the decode loop with Stack's streaming accumulator shape
(`metadata/audio_analyzer.rs`), O(target_bars) memory. **Never hold a decoded track:**
a 10-minute 48 k stereo f32 buffer is 230 MB.

Peaks go to disk under app data keyed by `content_hash`, never into the `tracks` row —
Keystone's tracks are full length with multiple zoom levels, and the virtualized table
selects that row on every page. This is also exactly what the spec's "Cache audio wave
image during analysis" setting means.

### S2 — Tuning estimate
**In:** 44.1 k mono. **Out:** `tuning_cents` in [-50, +50], and a corrected 22.05 k
buffer when |offset| > 10 cents.

STFT 4096 Hann, hop 1024. Parabolic interpolation of spectral peaks, then a
**magnitude-weighted circular mean** of `mod(12·log2(f/(440/16)), 1)` folded to
[-0.5, 0.5) semitones (Dressler & Streich). Clamp the implied reference to
**431.6–448.5 Hz** (±1/3 semitone, Faraldo's range). Resample by `2^(-offset/12)`
when |offset| > 10 cents; otherwise skip and save the resample.

**This stage runs first and is not optional.** S-KEY's VQT is 12 bins/octave — one bin
per semitone, no sub-semitone resolution — and it was measured collapsing to the wrong
tonic *and* wrong mode at ±50 cents (E♭ major read as G minor), while staying correct
at +20 c and perfectly equivariant at +100 c. Correcting the detune restored the
original prediction to three decimals.

Honest caveat for anyone budgeting accuracy points to this: both Faraldo (neutral on
GiantSteps) and Sha'ath (who disabled tuning handling in KeyFinder by default because
it did not help on dance music) measured **no aggregate gain** on digitally-produced
EDM at A440. The ±50 c experiment was n=1 and the pitch shift also moved tempo ~3%.
Tuning correction is cheap insurance against catastrophic tail failures — vinyl rips,
pitched edits, live recordings, turntable-pitched hip-hop — not a headline win. For
this audience that tail is the audience.

### S3 — Chroma / HPCP
**In:** tuned 44.1 k mono. **Out:** one 36-bin PCP vector per track, and per-strong-beat
12-bin chroma for S9/S11.

Faraldo's settings, exactly: **window 4096 Hann, hop 16384** (note hop > window —
frames are *sampled* every 371 ms, each 93 ms long, deliberately not overlapped),
**25–3500 Hz, 36 bins** (3 per semitone), weighting 1 semitone. Plus bgate's
additions, in order:

1. **200 Hz high-pass** before the FFT, killing percussive low-frequency noise.
2. **Spectral whitening** (Röbel & Rodet 2005) on the magnitude spectrum.
3. Aggregate all frames, **normalise so the vector sums to 1**.
4. **Gate: zero every bin below 0.2.** Once, after normalisation.
5. Detuning correction as a ±1 shift of the 3 bins/semitone.

### S4 — Key
**In:** 36-bin PCP; tuned 22.05 k mono. **Out:** `key_root` 0–11, `key_mode`,
`key_confidence`, `key_alt_root`/`key_alt_mode`, and per-chunk keys.

Two voters, fused.

**(a) S-KEY via ONNX.** Peak-normalise to 1.0, feed `[1, N]` f32 at 22,050 Hz, read
`[1,24]` already-softmaxed scores. Label order is **12 majors from A, then 12 minors
from B** (G major = 10, A minor = 22). Minimum 3 s. **Chunk at 15 s and average the
probability vectors** rather than feeding the whole file: whole-file gave top-1 0.268
on the one test track against 0.480 for chunk-averaging, and per-chunk accuracy rose
with window length (3 s 30%, 5 s 50%, 10 s 67%, 15 s 100%). That is n=1 — treat the
accuracy claim as a hypothesis for the eval harness — but the confidence lift is real
and chunking yields per-region key for free, which the deferred deck view needs. VQT,
dB scaling, cropping and ChromaNet are all inside the graph, so there is no DSP to
reimplement.

**(b) bgate template.** Cosine distance (Sha'ath found cosine consistently beats
correlation on dance music) against **four** profiles: one major, two minor (the second
for tracks whose ♮3 outweighs ♭3, common in EDM because rich synth spectra reinforce
the tonic's fifth harmonic), and one **amodal** profile with all energy on the tonic.
**Amodal results are reported as minor.**

Use **bgate**, not edmm, despite edmm scoring 72.0 vs 72.5 on GiantSteps: edmm's major
profile is flat so it labels everything minor, and GiantSteps is 84.8% minor. On the
modally balanced Beatport set edmm drops to 63.8 while bgate holds 73.4, and edmm
produced 48% parallel errors on The Beatles. Temperley is actively harmful on EDM
(48.6) though best on Beatles (81.2).

**Profiles must be derived from the papers' descriptions or our own corpus.** Sha'ath's
numeric coefficients exist only inside GPL-3.0 libKeyFinder source (his thesis plots
them as bar charts with no table) and `edmkey`, the bgate reference implementation,
declares no licence at all, which means all rights reserved. Both shortcuts are closed.

**Fusion:** sum the two probability vectors with weights fitted on the eval set
(start 0.6 S-KEY / 0.4 template). `key_confidence` = top probability; store the
runner-up as `key_alt_*`. **Voter disagreement is the signal that drives the spec's
`Improve Tracks` collection and `Display a message if the file can be improved`.**
When mode confidence is low, the UI shows the relative pair (`8A/8B`) rather than
committing to a letter — most residual error is mode, which is exactly Camelot's
A/B, and a DJ would rather see acknowledged ambiguity than a confident wrong letter.
This is the one thing Mixed In Key does not do.

### S5 — Camelot mapping
**In:** `key_root` (0 = C), `key_mode`. **Out:** `camelot`, and the wheel's
compatibility set.

```
major:  n = ((7*p + 7) mod 12) + 1     # C -> 8B, G -> 9B, D -> 10B
minor:  n = ((7*p + 4) mod 12) + 1     # Am -> 8A, Em -> 9A, Bm -> 10A
```
Verified against Mixed In Key's own wheel page: 8B = C major, 8A = A minor, +1 = up a
perfect fifth. Compatibility set for `(n, letter)` is
`{(n, letter), (n, other letter), (n−1 wrap 12↔1, letter), (n+1 wrap, letter)}` — that
is the whole wheel filter. MIK's documented advanced moves (+2 energy boost, −5 jaws,
−3 pay attention) stay **out** of the default set.

### S6 — Onsets
**In:** 44.1 k mono. **Out:** one full-band novelty curve at 100 fps plus three
band-limited curves.

STFT `n_fft 2048`, `hop 441` → **100 fps**, Hann. 64 log-spaced bands from 30 Hz,
magnitude compressed as `log(1 + 1000·|X|)`. Half-wave-rectified first-order difference
summed across bands. Local-mean subtraction over a **1.5 s (150-frame)** window,
half-wave rectify, divide by the global sd. Keep band-limited curves for **30–250 Hz
(kick), 250–2500, >2500 (hats)** — the kick curve is what settles the tempo octave and
makes four-to-the-floor trivial.

**Use a sparse triangular filterbank stored as `(start_bin, weights)` per band, never
a dense matmul.** Measured 45 ms vs 1.07 s for the same spectrogram — a 24× penalty for
a two-line difference.

### S7 — Tempo and beat grid
**In:** novelty curves. **Out:** `tempo_raw`, `tempo_display`, `tempo_octave_shift`,
`tempo_confidence`, and a beat grid.

ACF over lags 1…120 frames (down to 50 BPM), then a comb filter on a **0.1 BPM grid
from 50 to 220**: `score = Σ_{k=1..4} ACF(round(k·lag)) / k`. Verified: on a 120 BPM
test signal this returned 119.5 BPM at salience 1.500 with the half-tempo second at
1.000, so the 1/k weighting already suppresses the sub-octave. Run the tempogram on the
**densest 60 s window by novelty RMS**, not the whole file, so long ambient intros do
not poison it.

Octave resolution, four steps **in this order**:

1. Comb filter with 1/k weighting.
2. Log-Gaussian perceptual prior centred at **120 BPM, σ ≈ 0.35 octaves**. Cap it:
   never let it overturn a ≥2:1 salience margin.
3. **Beat-level evidence, which is the step that actually settles it.** For the winning
   lag `L`, compare novelty strength at odd vs even multiples of `L/2` in the kick and
   hat bands. Kick on every candidate beat → the faster candidate is right; alternate
   beats carrying only hats → the slower one. Autocorrelation cannot tell you this.
4. **Fold last**, by repeated ×2 / ÷2 into `[lo, hi]` (default 79–192).

Two rules on folding: **fold the chosen tempo, never choose the candidate by what folds
into range** (otherwise a correct 170 BPM DnB track and a wrong 85 BPM estimate are
indistinguishable in the report); and store the three tempo columns separately so
changing the Tempo range setting re-folds from the database with **no re-analysis**.

**Two-decimal BPM cannot come from the tempogram.** Integer lag quantisation at 100 fps
gives a BPM step of 1.32 at 90, 2.35 at 120, 2.66 at 128, 5.04 at 174; 200 fps only
halves it. It comes from a **least-squares fit of time against beat index over the whole
grid**: with 10 ms beat jitter, sd(BPM) ≈ 0.019 at 200 beats, 0.010 at 400. **Report two
decimals only when the grid residual sd is under 15 ms**; otherwise report one decimal
or an integer and flag the track variable-tempo. The spec's two-decimal option is false
precision without this.

`tempo_confidence` = (second-best non-octave peak ÷ best), combined with the grid
residual sd and the fraction of the track covered by a consistent grid. The user can
always type a tempo.

### S8 — Downbeats, strong beats, phrase period
**In:** beat grid, band novelty, per-beat chroma. **Out:** `strong_beat` grid (beats 1
and 3), a downbeat phase with confidence, and a 4-bar phrase period and phase.

**Build the cue pipeline on the strong-beat grid, not the bar grid.** Zehren et al.
state they could not reliably distinguish the downbeat from beat 3 in EDM even using
madmom, and fell back to strong beats for exactly that reason. A 2-beat grid is
reliable; a 4-beat grid is not. Treat a confident 4-beat downbeat as a bonus. Designing
on the reliable grid is what stops a systematic half-bar offset leaking into every cue.

Classical phase selection: over the four candidate phases, maximise
`low-band flux at candidate downbeats + chroma change at candidate downbeats + bar-level
self-similarity`. Works on four-to-the-floor because kick-plus-new-bass-note lands on
bar 1.

Phrase period: from the eight candidate strong-beat phase offsets, choose the one
maximising the **RMS of novelty sampled 4 bars apart** across the track (Zehren's
modification of Vande Veire — RMS, not a peak count). Then promote 4-bar boundaries to
8/16/32 by novelty strength at each level. Reliability on EDM is roughly 90%; wrong
period accounted for 45% of Zehren's false negatives. Poor on non-multiple-of-4 intros,
3/4 and 6/8, and everything non-electronic.

**Beat This! is behind a gate, not in the build by default.** Beat This! (CPJKU) is MIT
for code *and* weights and is the only permissively licensed path to state-of-the-art
beat and downbeat tracking with no DBN post-processing (madmom's DBN models are
CC BY-NC-SA and unusable). But the fp32 ONNX export is **190 MB** and measured
**68× realtime at best** (442 ms per 30 s window, 8 threads; the machine saturates at
60–68× however threads or parallel sessions are arranged, and the CoreML EP is **3×
slower** than the CPU EP — do not spend a week on it). Full-track inference is 4.2 h for
3,000 tracks.

So: **first measure whether we need it.** Run the classical S8 phase against Beat This!
on a 200-track EDM fixture set. If classical agrees with the model on >90% of tracks at
±70 ms, Beat This! is dropped from v1 and 190 MB is not bundled. If it is kept, run it
on **three sampled 30 s windows at 25/50/75% of the core** (median inter-beat interval,
modal downbeat phase) and propagate the grid with a DP beat tracker over the free
classical novelty curve — ~1.3 s instead of 5.3 s per track. Validate propagation
against a held-out fourth window at ±70 ms and fall back to full-track inference when it
fails. Either way the model must be int8-quantised (~48 MB; the 2–3× speedup is
**expected but unmeasured**) — a 190 MB bundled model is not distributable, and a
download-on-first-run contradicts the offline premise.

The published export is **batch-1 only**: `[12,1500,128]` fails with a Reshape error, so
windows go one at a time. Its front end, from the shipped config: 22,050 Hz mono,
`n_fft 1024`, `hop 441` → 50 fps, periodic Hann, `center=True` reflect-pad, magnitude
÷ `sqrt(n_fft)`, 128 mel, `log1p(1000·x)`, windows of 1500 frames, `borderSize 6`,
logits `> 0`, peak kernel 7, threshold 0.0, dedupe width 1, each downbeat snapped to its
nearest detected beat.

### S9 — Structure
**In:** strong-beat grid. **Out:** per-feature novelty curves, retained phrase
boundaries, and `track_regions` rows.

Seven strong-beat-synchronous features. Sparse/transient features aggregate by **sum of
onsets** per strong beat; continuous ones by **RMS**:

1. kick-band flux 30–150 Hz
2. snare-band flux 150–800 Hz, transient-emphasised
3. hat-band flux > 6 kHz
4. harmonic-component energy after HPSS
5. percussive-component energy after HPSS
6. an 84-bin log-frequency magnitude vector, `f_min 32.7 Hz`, hop 512 @ 44.1 kHz
7. a 12-bin chroma, 7 octaves, `f_min 27.5 Hz`

HPSS by median filtering, ~17 frames × 17 bins at hop 512 (median across time →
harmonic, across frequency → percussive). Zehren used a CNN drum transcriber for 1–3;
band-limited flux is the permissive substitute and gives up some precision — say so in
the changelog rather than claiming parity.

Per feature: SSM with **standardised-Euclidean** distance, convolved with a **Gaussian
checkerboard kernel sized to compare 4 bars against 4 bars** — an 8-bar kernel = 32
beats = 16 strong beats. **Valid** padding, *except* **zero** padding for the two
loudness features (4 and 5), which is the single detail that gives the intro a novelty
value at all and so makes "intro start" detectable. Measured 1.2 ms per feature at 384
strong beats, under 10 ms for all seven — this stage is free, so do not economise on
features.

Sections = consecutive retained boundaries at the 8- or 16-bar level.

### S10 — Energy, 1–10 integer
**In:** the core region, strong-beat features. **Out:** `energy` 1–10, `energy_raw`,
`energy_model_version`, plus a per-section energy on every `track_regions` row.

**Core region:** the contiguous span between the first and last downbeat where
short-term loudness is within **6 LU** of the track's 95th-percentile short-term
loudness. Without this a 7-minute club edit reads lower than its own radio edit.

Seven features, each per bar, aggregated as a **10%-trimmed mean**:

| # | Feature | Definition |
|---|---|---|
| 1 | `onset_rate` | onsets **per beat** (tempo-normalised) |
| 2 | `flux_mean` | mean HWR spectral flux **÷ frame RMS** |
| 3 | `low_ratio` | energy 30–150 Hz ÷ total |
| 4 | `high_ratio` | energy > 6 kHz ÷ total |
| 5 | `centroid` | spectral centroid, log-Hz |
| 6 | `perc_ratio` | percussive ÷ (percussive + harmonic) after HPSS |
| 7 | `dynamic_span` | 95th − 50th percentile of **EBU R128 short-term (3 s) loudness**, inverted |

Then `z = clamp((log x − μ_f)/σ_f, −3, 3)`, `u = Φ(z)`,
`raw = Σ w_f · u_f` with starting weights `onset_rate 0.22, flux 0.20, low_ratio 0.14,
high_ratio 0.14, centroid 0.10, perc_ratio 0.14, dynamic_span 0.06`, and
`energy = clamp(round(1 + 9·Q(raw)), 1, 10)` where `Q` is a monotone piecewise-linear
map placing the calibration corpus's decile boundaries on the integer boundaries.

**μ_f, σ_f, the weights and the quantile table are constants compiled into the binary**,
derived once from a calibration corpus, then frozen and stamped as
`energy_model_version`. Freezing is the whole trick: it is what makes a 7 mean the same
thing in 2029, and what lets us re-tag a library without re-analysing a file. Energy
numbers get written into the user's file tags, so the constants are a permanent
compatibility surface — changing them is a migration, not a tweak.

Three constraints that fix this design:

- **It is not loudness.** Loudness alone explains arousal at R² ≈ 0.43–0.55 while timbre
  feature sets reach 0.69–0.75, and every modern master sits near −8 LUFS so integrated
  loudness is nearly constant across a commercial library.
- **Genre stability without a genre classifier.** Six of the seven features are ratios or
  per-beat rates; the only absolute quantity, `dynamic_span`, carries the lowest weight.
  Do not normalise per genre — that needs a genre detector and the user's library is not
  a sample of the world. Test the invariance instead (§7).
- **Patent.** US 8,865,993 (Mixed In Key LLC, filed 2013, granted 2014, **Active**,
  anticipated expiry 2033) has, in **all three** independent claims, the step of
  estimating energy by correlating an attribute profile against **a stored database of
  reference compositions with known energy values**. A direct feature→score formula with
  published fixed coefficients, no reference store and no correlation step reads outside
  the claims as written. That is why the constants are baked in rather than the score
  coming from a nearest-neighbour regressor over labelled references. **This is not legal
  advice — get an opinion before shipping the energy feature**, and keep the definition
  documented and reference-free.

**Track energy is the trimmed mean over the core, not the mean of section energies.** It
is deliberately drop-weighted. Document that, because someone will file it as a bug.
Section energy uses the identical frozen constants so the numbers are comparable.

### S11 — Eight cue points
**In:** novelty curves, strong-beat grid, phrase period, HPSS energies.
**Out:** eight `cue_points` rows with `kind`, `position_ms`, `bar_index`,
`phrase_level`, `confidence`, `source='detected'`.

**The reality check first.** On EDM-CUE (4,710 tracks, 21,461 cue points placed by four
professional DJs in rekordbox): Automix 0.13 F1, **Mixed In Key 10 scores 0.22**,
CUE-DETR r8 0.39. On Zehren's own 134-track set, where annotators were given his rules
as guidelines, his algorithm reached **86% precision / 49% recall** against MIK's 65%
precision, and his subjective follow-up found **95.8% of generated candidates usable** by
at least one expert at 8.9 candidates per track.

So: exact agreement with a particular DJ is not achievable by anyone including the
incumbent. Design the feature and the copy around "eight musically defensible,
phrase-aligned starting points you will drag", not "the eight right answers" — the spec
already makes the markers draggable. The useful corollary: MIK's cue points (0.22) are
far weaker than its key detection, so **cue points are the easier place to beat it**.

Pipeline, a port of Zehren's rules from Automix (MIT, portable freely):

1. **Peak-pick per novelty curve:** local maximum within a **4-bar window**, amplitude
   above **0.3 × that curve's maximum**. Union across all seven curves.
2. **Period filter:** keep only candidates on the estimated 4-bar period and phase.
3. **Salience filter:** require mean harmonic energy over the **4 bars following** the
   candidate to exceed **0.4 × the track's maximum harmonic energy**. This encodes "the
   section after this point can stand on its own in a mix" and is what takes precision
   from ~30% to ~86%. **Do not skip it.**
4. **Assign the eight slots by role** — no paper covers this, and it is where the spec's
   eight fixed slots bite. Taking the top eight by salience clusters them in one section
   and makes the feature useless even when every detection is correct.

| Slot | Role | Rule |
|---|---|---|
| CUE 1 | intro start | first candidate at/after the first downbeat whose preceding 4 bars are below 0.25 × max percussive energy and whose following 4 bars are above it; else the first downbeat |
| CUE 2 | first beat | first downbeat where kick-band energy crosses 0.5 × its track median over a 4-bar window — on four-to-the-floor the most reliable cue in the set |
| CUE 3/4 | breakdowns | percussive energy over the following 8 bars below 0.4 × its track median *while* harmonic energy stays above 0.4 × its own median; take the two longest such regions' starts |
| CUE 5 | build | start of a monotone rise in high-band energy and onset density lasting ≥ 4 bars and ending within 2 bars of a drop candidate |
| CUE 6/7 | drops | combined kick + percussive energy over the following 4 bars jumps ≥ 1.5× relative to the preceding 4, on a 16-bar boundary; two highest jumps, forced ≥ 32 bars apart |
| CUE 8 | outro | last candidate ≥ 32 bars before the end where percussive energy begins a monotone decline; else the last 16-bar boundary |

5. **Enforce coverage:** partition the core into 8 equal spans; if one span holds no cue
   while another holds three, move the weakest-salience duplicate to the best candidate
   in the nearest empty span. Minimum spacing 8 bars.
6. Everything snaps to a downbeat, or to a strong beat when downbeat-phase confidence is
   low. **Never to a raw novelty peak.**

Role assignment also gives each marker a label the UI can show, which is worth more to a
DJ than an unlabelled confidence ranking.

### S12 — Persist
**In:** everything above. **Out:** committed rows; `stack://`-style events to the
frontend.

Batched transactions through a **single dedicated writer task**, `UPSERT_BATCH_SIZE =
100`, lock-retry backoff. Analysis workers never touch the pool. Write
`analysis_version`, `energy_model_version`, `analyzed_at`, and an `analysis_cache` row
keyed by `content_hash`.

Invalidation rules, and they are **different for tags and analysis**: tag reads
invalidate on **mtime**; analysis invalidates on **`content_hash` + `analysis_version`**.
A tag edit changes mtime but not audio and must not cost a re-analysis.

`analysis_cache` keyed by content hash is the highest-value piece of the model: a moved
file, a file Keystone itself renamed, and a duplicate all hit it with zero DSP. That is
what makes re-scan nearly free and the rename feature free instead of a re-analysis
storm.

---

## 3. Rust module layout

Mirrors Stack: commands are thin, all SQL lives in `db/`, core domain logic has no Tauri
dependency. Stack's file caps apply — **Rust modules ≤ 300 lines, functions ≤ 50 lines**;
extract a sub-module rather than letting a file grow.

```
src-tauri/src/
  main.rs            Bin stub; calls keystone_lib::run().
  lib.rs             tauri::Builder chain; invoke_handler lists every command.
  state.rs           AppState: db pool, repos, indexer, analysis queue, watcher, Arc<RwLock<Settings>> over settings.json.
  error.rs           KeystoneError + serde-serializable Result alias.

  models/
    track.rs         The Track struct as the frontend sees it, serde(rename_all = "camelCase").
    cue.rs           CuePoint, CueKind, BeatgridMarker.
    collection.rs    Collection and its membership row.
    analysis.rs      AnalysisResult, KeyResult, TempoResult, EnergyResult, TrackRegion, confidence fields.
    settings.rs      The whole settings surface from SPEC.md §3, flat, serialized to settings.json.

  db/
    connection.rs    Copied verbatim from Stack: WAL pragmas, mmap, busy_timeout, pool max_size 4.
    migrations.rs    Const slice of (name, sql) applied one transaction each, exactly Stack's runner.
    track_repo.rs    Reads and upserts tracks, including the batched writer path.
    cue_repo.rs      Cue points and beatgrid rows, with the user/detected source guard.
    collection_repo.rs  Collections and ordered membership; Stack's stack_repo with `position` renamed.
    analysis_repo.rs Analysis cache lookups by content_hash, plus track_regions.
    query_builder.rs Builds the library query: Camelot set, collection, FTS MATCH, sort, and path_upper_bound range scans.

  core/
    scanner.rs       Recursive walk with a skip list, audio extensions only.
    watcher.rs       notify-debouncer-full with FileIdMap, debounced into a WatchEvent channel.
    hasher.rs        xxh3 over the audio data region, never the file head.
    indexer.rs       Semaphore-bounded IO stage with live set_concurrency and the exclude()/unexclude() choke points.
    analysis_queue.rs  The rayon-backed CPU stage behind a bounded channel; resumable across restarts, since SPEC.md makes it a sidebar object.
    reconciler.rs    Startup diff: chunked existing_paths then touch_last_seen_by_paths, marks missing.
    renamer.rs       Journal-first renames with the case, NFD, idempotence and Windows hazards handled.
    peaks.rs         Streaming waveform peak accumulation per zoom level, written to disk by content_hash.

  analysis/
    mod.rs           Orchestrates S1..S12 for one track and returns one AnalysisResult.
    decode.rs        symphonia decode, downmix, resample; the one shared decode pass (shared crate with Capstone).
    frontend.rs      STFT, sparse triangular filterbank, HPSS, band splits — everything shared by onsets, structure and energy.
    tuning.rs        S2: magnitude-weighted circular mean over interpolated peaks, and the corrective resample.
    chroma.rs        S3: 36-bin HPCP with high-pass, whitening, normalisation and the 0.2 gate.
    key/mod.rs       S4: runs both voters and owns the fusion weights.
    key/skey.rs      S-KEY ONNX session, chunking, label order, probability averaging.
    key/template.rs  Cosine matching of the PCP against the profile set.
    key/profiles.rs  The four bgate-style profiles, derived from the papers, with their provenance recorded in comments.
    key/fuse.rs      Probability fusion, confidence, runner-up, and the low-mode-confidence ambiguity flag.
    camelot.rs       S5: key -> Camelot number/letter and the compatibility set the wheel filter uses.
    onset.rs         S6: the 100 fps novelty curve and its three band-limited siblings.
    tempo.rs         S7: ACF comb tempogram, the four-step octave rule, folding, and the least-squares BPM refit.
    beats.rs         S8: beat grid, strong-beat grid, downbeat phase, phrase period; the optional Beat This! arbiter lives here.
    structure.rs     S9: strong-beat features, SSMs, checkerboard novelty, retained boundaries, track_regions.
    energy.rs        S10: the seven features, the frozen constants, and the quantile map.
    cues.rs          S11: peak-pick, period and salience filters, role assignment, coverage enforcement.
    version.rs       ANALYSIS_VERSION and ENERGY_MODEL_VERSION constants, and what each bump invalidates.

  tags/
    mod.rs           Dispatches by container to the format-specific tag type.
    read.rs          Reads artist/title/album/genre/grouping/comment/year/bpm/key and cover art.
    write.rs         Copy-modify-verify-rename writes with mtime restoration.
    initial_key.rs   The per-container Initial Key field map and the TKEY legality rule.
    patterns.rs      SPEC.md §3's "what to write / where to write it" composition, and the exact-strip inverse.
    backup.rs        Stores replaced tag bytes in tag_backup before every write.

  export/
    mod.rs           Target detection, path override, and the per-target dispatch.
    safety.rs        Running-app refusal, dated backups, atomic rename, post-write verification, dry run, resumable log. Built first.
    rekordbox.rs     Streams a DJ_PLAYLISTS document with a COLLECTION and one named playlist node.
    traktor.rs       Event-stream rewrite of collection.nml, splicing only CUE_V2 and the key, preserving every byte it does not model.
    serato/          v1.1. Markers2/Markers_/BeatGrid/Autotags serialisation plus the per-container envelopes.

  eval/              Dev-only binary, never shipped in the bundle.
    mod.rs           Harness entry: runs the engine over an annotated corpus.
    giantsteps.rs    Loads GiantSteps+ audio and key annotations.
    mirex.rs         The five MIREX categories, the weighted score, exact match, mode accuracy and KSEA.

  commands/
    library_commands.rs     Add/remove watched folders, scan, reconcile, clean cache, library tree-free folder listing.
    track_commands.rs       search_tracks, get_track, rating, remove from library, reveal, rename.
    analysis_commands.rs    Queue control, re-analyze, progress, get_waveform, get_regions.
    cue_commands.rs         Read/add/move/delete cue points, honouring source='user'.
    collection_commands.rs  CRUD and ordered membership for the sidebar collections.
    tag_commands.rs         Per-track edits, global replace, remove extra spaces, re-tag existing files.
    export_commands.rs      Detect targets, dry-run counts, export, restore backup.
    player_commands.rs      Returns a path for convertFileSrc; no decoding on this path.
    settings_commands.rs    get/update settings; re-folds tempo display without re-analysis.
    update_commands.rs      Updater plumbing, copied from Stack.
```

---

## 4. Dependencies

### Accepted

| Crate | Version | Licence | For |
|---|---|---|---|
| `tauri` + plugins | 2 | MIT/Apache-2.0 | Shell, dialog, fs, opener, updater, process |
| `symphonia` | **0.5.x** | MPL-2.0 | Decoding MP3/AIFF/WAV/FLAC/M4A |
| `rubato` | 5.0.1 | MIT/Apache-2.0 | Resampling to 22.05 kHz and the tuning correction |
| `rustfft` | 6.4.1 | MIT/Apache-2.0 | Every FFT in the pipeline |
| `realfft` | 3.5.1 | MIT | Real-input wrapper, roughly 2× on real signals |
| `ndarray` | 0.17 | MIT/Apache-2.0 | Spectrograms, SSMs, feature matrices |
| `ort` | **pinned exactly** 2.0.0-rc.15 | MIT/Apache-2.0 (ONNX Runtime itself MIT) | S-KEY inference, and Beat This! if it survives the §2 gate |
| `ebur128` | 0.1.11 | MIT | EBU R128 short-term loudness for `dynamic_span` |
| `lofty` | 0.25.5 | MIT/Apache-2.0 | ID3v2, MP4 ilst, Vorbis comments, APE, WAV, AIFF in one API; also exposes GEOB as a binary frame, which Serato needs later |
| `filetime` | 0.2.29 | MIT/Apache-2.0 | Restoring mtime after a tag write |
| `quick-xml` | 0.42 | MIT | Streaming rekordbox XML and the Traktor NML rewrite |
| `rusqlite` | 0.31 (bundled) | MIT | SQLite |
| `r2d2` / `r2d2_sqlite` | 0.8 / 0.24 | MIT/Apache-2.0 | Connection pool |
| `rayon` | 1.12 | MIT/Apache-2.0 | The CPU-bound analysis stage |
| `tokio` | 1 | MIT | Tauri's managed runtime; the IO stage |
| `notify` + `notify-debouncer-full` | 6 / 0.3 | MIT/Apache-2.0 | File watching, with `FileIdMap` |
| `walkdir` | 2 | MIT/Apache-2.0 | The scan walk |
| `xxhash-rust` | 0.8 (xxh3) | BSL-1.0/MIT | Path ids and audio-region content hashes |
| `unicode-normalization` | 0.1.26 | MIT/Apache-2.0 | NFC/NFD filename comparison for renames |
| `same-file` | 1.0.6 | Unlicense/MIT | Rename collision identity, instead of comparing path strings |
| `tempfile` | 3 | MIT/Apache-2.0 | `persist()` for atomic replace, which plain rename cannot do on Windows |
| `criterion` (dev) | 0.8.2 | MIT/Apache-2.0 | The decode benchmark that must be the first task of the build |

**Model assets:** S-KEY ONNX, **324,536 bytes**, opset 17, fp32, input `[1, samples]`
f32 mono @ 22,050 Hz, output `[1,24]` already softmaxed. Trivially bundled.

Two notes on `ort`: it is a release candidate with an ABI pinned to a specific ONNX
Runtime version, so **pin it exactly, no caret**. On Windows ship the DLLs beside the
binary — a stale system `onnxruntime.dll` will shadow the bundled one. The macOS arm64
dylib is 31.5 MB per platform; a minimal build drops `.onnx` format support, which
defeats the purpose. `tract-onnx` 0.21 (MIT/Apache-2.0) is the pure-Rust fallback if the
C++ dependency complicates cross-compilation, at the cost of transformer throughput.

**symphonia is pinned to 0.5, not 0.6.** One researcher recommended 0.6.1 and another
0.5; 0.5 wins because Stack's proven `audio_analyzer.rs` streaming decode is a 0.5 call
site and 0.6 is a declared significant breaking change (audio primitives redesigned,
`MediaSourceStream` lifetime-generic, track timing moved out of `codec_params`, readers
returning `Ok(None)` at end of media). Port to 0.6 as its own scheduled task, in a
decode module **shared with Capstone**, so the port is paid for once.

**MPL-2.0 note:** symphonia is MPL-2.0, which is not on the stated MIT/Apache/BSD list.
It is file-level copyleft: the obligation attaches to symphonia's own files if we modify
and redistribute them, and does not reach Keystone's MIT code linking against it. Stack
already ships on that basis. **Accept it, list it in `THIRD-PARTY-LICENSES`, and never
patch its sources in-tree.**

### Rejected on licence grounds

| Rejected | Licence | Replaced by |
|---|---|---|
| **libKeyFinder**, `libkeyfinder-sys` | GPL-3.0(-or-later) | Hand-rolled HPCP on `rustfft`. Also means **Sha'ath's numeric profile coefficients are off-limits** — they exist in numeric form only in GPL source |
| **Essentia**, `essentia-rs` | AGPL-3.0 / GPL-3.0-or-later | Hand-rolled. Network copyleft would also reach any future web mode |
| **aubio**, `aubio-rs`, `aubio-sys` | GPL-3.0(-or-later) | Hand-rolled onset/tempo. Also unmaintained since 2021 |
| `bliss-audio` | GPL-3.0-only | n/a — it wraps aubio |
| **madmom models** | code BSD-2, **all model files CC BY-NC-SA 4.0** | Beat This! (MIT code *and* weights). The LICENSE explicitly names the pickled models and directs commercial users to contact the authors, so the published AllConv weights are unusable |
| **hendriks73/key-cnn** | AGPL-3.0 | S-KEY |
| `chord_detector` | GPL-3.0 | Not needed in v1 |
| **angelfaraldo/edmkey** | **no licence declared** = all rights reserved | Implement bgate from the papers, not the code |
| **MusicFM+MLP** (MIREX 2026, 77.62) | no licence stated | Not shippable. Useful only as proof the ceiling is ~78 |
| **Mixxx** | GPL-2.0-or-later | Read it to understand *what* Serato and Traktor do; **never port a line.** It is the best reference in open source and therefore the most likely way this project acquires a copyleft problem |
| **qm-dsp / QM Vamp**, **BTrack** | GPL family | Hand-rolled |
| `rbox`, `prolink-rekordbox` | GPL-3.0-only (and `rbox` contradicts itself across docs.rs and PyPI) | rekordbox XML generation |
| **crate-digger** (Deep Symmetry) | EPL-2.0, and Java | Documentation only; its write-ups are the best public record of the ANLZ formats |
| **M-DJCUE** cue annotations | GPL-3.0 | Fine as a **local dev/test set**; must not be redistributed, and nothing derived from it (fitted thresholds baked into the binary) may ship. Keep that boundary explicit in the repo |

### triseratops — resolved, and the resolution is verified

The two researchers who looked at this disagreed, and **both were right about different
artifacts**. Verified directly:

- **Every published crates.io version is `AGPL-3.0-or-later`** — 0.0.1 (2021-02-04),
  0.0.2 (2022-02-19), 0.0.3 (2023-11-30).
- **The repository was relicensed to MPL-2.0 and never republished.** `main` at
  `8e92aae1794c4f02a2405eb88ea72f251b077f0c` (2025-11-24) carries
  `license = "MPL-2.0"` in `Cargo.toml` and `SPDX-License-Identifier: MPL-2.0` headers
  in the 2025 source files.

So `triseratops = "0.0.3"` pulls an AGPL-labelled artifact into an MIT product and is
**forbidden**. The MPL-2.0 `main` is usable and genuinely valuable — it already *writes*,
not just parses (`write_markers2`, `write_cue_marker`, `write_loop_marker`,
`write_color_marker`, `write_unknown_marker`, plus `write_id3`/`write_flac`/`write_mp4`/
`write_ogg` envelope writers), which collapses the hardest-looking export target from a
reverse-engineering job to an integration job.

Decision: **Serato is out of v1 anyway (§5), so this is not a v1 dependency.** When
Serato lands, pin that git rev, vendor the LICENSE file, and record the SPDX header
relied on in `THIRD-PARTY-LICENSES`. The crate is 0.0.x and self-describes as under
heavy development with breaking API changes, so vendoring is protection as well as
diligence. The fallback, if the repo state ever becomes unclear again, is clean-room
implementation from the CC BY-SA 4.0 `Holzhaus/serato-tags` documentation — read the
spec, never paste its prose or byte tables into source comments, or ShareAlike attaches
to them.

---

## 5. SQLite schema

Patterns Stack already paid for, carried over.

**Copy verbatim:** `db/connection.rs` (WAL, `synchronous=NORMAL`, `temp_store=MEMORY`,
`mmap_size=256MB`, `cache_size=-32000`, `busy_timeout=10000`, pool `max_size=4` — the
small pool is deliberate, since WAL gives concurrent readers but one writer and a bigger
pool just multiplies contention) and the migration runner (const slice of `(name, sql)`,
`schema_migrations` table, one transaction per migration).

**Build FTS5 into migration 001**, even though a few thousand tracks would be fine on a
LIKE. Stack retrofitted FTS in migrations 010/011, the rebuild left the index empty, and
that is the entire reason `core::genre_backfill` exists as a ~9 s-per-launch repair pass.
Put it in at the start and that backfill is never written.

```sql
-- 001_initial

CREATE TABLE tracks (
  id                   TEXT PRIMARY KEY,          -- xxh3 of the path
  path                 TEXT NOT NULL UNIQUE,
  filename             TEXT NOT NULL,
  original_filename    TEXT,                      -- pre-rename, for the exact strip
  rename_pattern       TEXT,                      -- which pattern was applied
  extension            TEXT NOT NULL,
  content_hash         TEXT,                      -- xxh3 of the AUDIO REGION

  -- file facts
  size_bytes           INTEGER,
  duration_ms          INTEGER,
  sample_rate          INTEGER,
  channels             INTEGER,
  bit_depth            INTEGER,
  bitrate_kbps         INTEGER,
  codec                TEXT,

  -- tags as read from the file
  artist               TEXT,
  title                TEXT,
  album                TEXT,
  genre                TEXT,
  grouping             TEXT,
  comment              TEXT,
  year                 INTEGER,
  tag_bpm              REAL,
  tag_key              TEXT,
  has_cover_art        INTEGER DEFAULT 0,

  -- analysis: tempo
  tempo_raw            REAL,                      -- as detected, unfolded
  tempo_display        REAL,                      -- folded into the user's range
  tempo_octave_shift   INTEGER,                   -- -1 / 0 / +1
  tempo_confidence     REAL,
  tempo_residual_ms    REAL,                      -- grid fit sd; <15 gates two decimals
  tempo_variable       INTEGER DEFAULT 0,

  -- analysis: key
  key_root             INTEGER,                   -- 0-11, 0 = C
  key_mode             TEXT,                      -- 'major' | 'minor'
  camelot              TEXT,                      -- '8A'
  key_confidence       REAL,
  key_alt_root         INTEGER,
  key_alt_mode         TEXT,
  key_mode_ambiguous   INTEGER DEFAULT 0,
  tuning_cents         REAL,

  -- analysis: energy and levels
  energy               INTEGER,                   -- 1-10
  energy_raw           REAL,
  peak_db              REAL,
  true_peak_db         REAL,
  lufs_integrated      REAL,
  clipped_peaks        INTEGER,

  -- user data
  rating               INTEGER,                   -- 0-5
  date_added           INTEGER NOT NULL,
  last_played          INTEGER,
  play_count           INTEGER DEFAULT 0,

  -- provenance and lifecycle
  key_source           TEXT,                      -- 'detected' | 'user' | 'tag'
  tempo_source         TEXT,
  energy_source        TEXT,
  analysis_version     INTEGER DEFAULT 0,
  energy_model_version INTEGER DEFAULT 0,
  analyzed_at          INTEGER,
  index_status         TEXT DEFAULT 'pending',    -- pending|indexed|missing|error
  error_message        TEXT,

  created_at           INTEGER NOT NULL,
  updated_at           INTEGER NOT NULL,
  last_seen_at         INTEGER NOT NULL
);

-- Results keyed by CONTENT, so a moved, renamed or duplicated file costs no DSP.
CREATE TABLE analysis_cache (
  content_hash         TEXT NOT NULL,
  analysis_version     INTEGER NOT NULL,
  energy_model_version INTEGER NOT NULL,
  payload              TEXT NOT NULL,             -- JSON: key, tempo, energy, cues, regions, beatgrid
  created_at           INTEGER NOT NULL,
  PRIMARY KEY (content_hash, analysis_version)
);

CREATE TABLE cue_points (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  slot          INTEGER NOT NULL,                 -- 0-7, the eight CUE markers
  kind          TEXT NOT NULL,                    -- intro|first_beat|breakdown|build|drop|outro|loop|user
  position_ms   REAL NOT NULL,                    -- REAL: rekordbox 1/150 s frames, Serato ms, Traktor ms float
  end_ms        REAL,                             -- loops only
  bar_index     INTEGER,
  phrase_level  INTEGER,                          -- 4 | 8 | 16 | 32
  label         TEXT,
  color         TEXT,
  confidence    REAL,
  source        TEXT NOT NULL DEFAULT 'detected', -- 'detected' | 'user'
  created_at    INTEGER NOT NULL,
  UNIQUE (track_id, slot)
);

-- One row per tempo segment, mirroring rekordbox's TEMPO element so export is musically correct.
CREATE TABLE beatgrid (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position_ms   REAL NOT NULL,                    -- Inizio
  bpm           REAL NOT NULL,
  beats_per_bar INTEGER NOT NULL DEFAULT 4,       -- Metro
  beat_in_bar   INTEGER NOT NULL DEFAULT 1        -- Battito
);

-- The read model the deferred deck view needs. Costs nothing now; a migration later.
CREATE TABLE track_regions (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  start_ms      REAL NOT NULL,
  end_ms        REAL NOT NULL,
  bar_index     INTEGER,
  bars          INTEGER,
  kind          TEXT,                             -- intro|build|drop|breakdown|outro|unknown
  energy        INTEGER,
  key_root      INTEGER,
  key_mode      TEXT
);

CREATE TABLE collections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'user',       -- 'user' | 'smart'
  rule        TEXT,                               -- smart only, e.g. Improve Tracks
  color       TEXT,
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL
);

CREATE TABLE collection_tracks (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL DEFAULT 0,
  added_at      INTEGER NOT NULL,
  PRIMARY KEY (collection_id, track_id)
);

-- What makes RE-TAG EXISTING FILES a shippable button rather than a terrifying one.
CREATE TABLE tag_backup (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  container   TEXT NOT NULL,                      -- id3v2 | ilst | vorbis | ape
  raw         BLOB NOT NULL,                      -- the replaced tag bytes
  written_at  INTEGER NOT NULL
);

-- Per-file transaction log, so an interrupted 5,000-track export is reportable and resumable.
CREATE TABLE export_log (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL,
  target      TEXT NOT NULL,                      -- rekordbox | traktor | serato
  status      TEXT NOT NULL,                      -- planned | written | verified | failed | skipped
  detail      TEXT,
  run_id      TEXT NOT NULL,
  written_at  INTEGER NOT NULL
);

-- Journal-first renames: the row exists before fs::rename and is cleared after the UPDATE.
CREATE TABLE file_ops (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL,
  op          TEXT NOT NULL,                      -- 'rename'
  from_path   TEXT NOT NULL,
  to_path     TEXT NOT NULL,
  state       TEXT NOT NULL,                      -- intended | renamed | done
  created_at  INTEGER NOT NULL
);

CREATE VIRTUAL TABLE tracks_fts USING fts5(
  artist, title, album, genre, grouping, comment, filename,
  content='tracks', content_rowid='rowid', tokenize='unicode61'
);
CREATE VIRTUAL TABLE tracks_vocab USING fts5vocab(tracks_fts, 'row');
```

### Indexes, and the two rules behind them

```sql
-- Always-present leading filters are the Camelot selection and the collection,
-- so composites lead with those.
CREATE INDEX idx_tracks_camelot        ON tracks(camelot)            WHERE index_status != 'missing';
CREATE INDEX idx_tracks_camelot_energy ON tracks(camelot, energy)    WHERE index_status != 'missing';
CREATE INDEX idx_tracks_camelot_tempo  ON tracks(camelot, tempo_display) WHERE index_status != 'missing';
CREATE INDEX idx_tracks_energy         ON tracks(energy)             WHERE index_status != 'missing';
CREATE INDEX idx_tracks_tempo          ON tracks(tempo_display)      WHERE index_status != 'missing';
CREATE INDEX idx_tracks_genre          ON tracks(genre)              WHERE index_status != 'missing';
CREATE INDEX idx_tracks_rating         ON tracks(rating)             WHERE index_status != 'missing';
CREATE INDEX idx_tracks_path           ON tracks(path);
CREATE INDEX idx_tracks_content_hash   ON tracks(content_hash);
CREATE INDEX idx_tracks_index_status   ON tracks(index_status);
CREATE INDEX idx_tracks_last_seen      ON tracks(last_seen_at);
CREATE INDEX idx_tracks_analysis_ver   ON tracks(analysis_version)   WHERE index_status != 'missing';
-- The default sort column WITH ITS COLLATION.
CREATE INDEX idx_tracks_artist_nocase  ON tracks(artist COLLATE NOCASE);
CREATE INDEX idx_tracks_date_added     ON tracks(date_added);
CREATE INDEX idx_coll_tracks_pos       ON collection_tracks(collection_id, position);
CREATE INDEX idx_cues_track_slot       ON cue_points(track_id, slot);
CREATE INDEX idx_regions_track         ON track_regions(track_id, start_ms);
```

1. **Spell the filter predicate identically in the WHERE clause and in the partial
   index.** Stack's facet queries filtered `index_status != 'missing'` while the indexes
   did not carry it, so every branch fetched all 164k rows to re-check — 320 ms → 30 ms
   once fixed. Then single-column partial indexes lost to `idx_assets_type` the moment a
   type tab was selected, costing a temp B-tree per branch — 340 ms → 81 ms with
   composites. Hence the Camelot-leading composites above.
2. **Never `LIKE 'prefix%'` on a path.** SQLite's LIKE is case-insensitive by default,
   which disables the prefix optimisation and forces a full scan. Use the range form
   `path >= ? AND path < ?` via a copy of Stack's `query_builder::path_upper_bound`.

And index the **default sort column with its exact collation**: without Stack's
`idx_assets_filename ON assets(filename COLLATE NOCASE)`, every page load sorted the
whole table through a temp B-tree carrying all projected columns — roughly 300 MB of
sorter traffic at 164k rows.

Settings live in `settings.json` behind `Arc<RwLock<Settings>>`, Stack's pattern, not in
a table: Keystone's settings surface is large but flat and single-user.

---

## 6. Cue-point export

Three targets, three completely different problems. Ranked by difficulty:
**rekordbox XML < Traktor NML < Serato.**

### The safety layer, built first. 3–4 days, not optional.

These are irreplaceable libraries. Every rule below exists because of a specific way to
destroy one.

- **Refuse to write while the DJ app is running.** Process-name detection (`Serato DJ
  Pro`, `Traktor`, `rekordbox`, `rekordboxAgent`) **plus** a lock probe: attempt an
  exclusive open of `master.sqlite` / `master.db`; failure means running. A **blocking
  modal**, not a dismissible warning. For Traktor this is not politeness — Traktor holds
  the collection in memory and rewrites `collection.nml` on exit, so any external edit
  made while it runs is **silently discarded**, and a half-written file can be read in
  and re-saved, laundering corruption into the backup rotation.
- **Back up before the first write to each target, keep N dated copies.**
  `<app data>/backups/traktor/collection-<ISO8601>.nml`. Expose "Restore backup" in
  Settings. This is the only thing that matters when it goes wrong.
- **Atomic write, always.** Temp file in the *same* directory, `fsync` the file, `fsync`
  the directory on unix, then rename over the target. Windows `std::fs::rename` fails
  when the destination exists — use `tempfile`'s `persist` or `ReplaceFileW`. Never write
  in place.
- **For audio files: copy → modify the copy → verify → atomic rename.** Verify by
  re-parsing: the tag reads back, and the audio stream length and first-frame offset are
  unchanged. A crash mid-write on an MP3 whose ID3v2 block grew destroys **audio**, not
  just metadata.
- **Never write a tag you did not fully parse.** Preserve unknown Serato Markers2 entry
  types verbatim; byte-copy every NML element not modelled.
- **Restore the original mtime after a tag write** (`filetime`). Otherwise Serato,
  Traktor and rekordbox re-analyse the user's entire library because Keystone re-tagged
  it, and Keystone's own mtime fast path invalidates itself.
- **Dry run by default.** `Overwrite cue points already in the collection` defaults off,
  and the export action shows a count of what would change before doing anything.
- **Per-file transaction log** (`export_log`), so an interrupted run across 5,000 tracks
  is reportable and resumable rather than silently half-applied.

One more hard rule, and it belongs here because it is the highest-consequence line in
the app: **never round-trip a DJ library file through lofty's generic `Tag`.** Its own
docs say tag→`Tag` conversion discards "anything without a mapping", so a bulk REPLACE
ALL over the generic type would silently delete every Serato GEOB blob, Traktor blob,
ReplayGain and iTunNORM frame in the library in one click, invisibly, until the user
opened Serato. Use `Id3v2Tag`, `Ilst`, `VorbisComments`, `ApeTag` only, and gate the
write path behind a round-trip fixture test on a Serato-tagged MP3, AIFF and FLAC.
Related: `ItemKey::Bpm` (decimal) is unsupported in ID3v2 — only `IntegerBpm` maps to
TBPM and the MP4 `tmpo` atom, so the spec's decimal tempo display must come from our own
DB, never a tag round-trip. And lofty always upgrades v2.2/2.3 to **v2.4** on write;
test a re-tagged v2.3 file in rekordbox and Traktor before shipping.

### v1 — rekordbox XML. 2–3 days.

The only officially documented format (AlphaTheta's `xml_format_list.pdf` v1.0.0). We
write our own file; nothing of the user's is touched.

`DJ_PLAYLISTS` → `PRODUCT` + `COLLECTION` (with `Entries`) → `TRACK`, plus a `PLAYLISTS`
→ `NODE` tree. Per track: `Location` (a `file://localhost/...` URI, essential),
`Tonality` (the key string), `AverageBpm`, `TotalTime`, `Rating`
(0/51/102/153/204/255), plus:

- **`TEMPO`**: `Inizio` (seconds, float), `Bpm`, `Metro` (`"4/4"`), `Battito` (1–4).
  **Two markers for a constant-tempo track, not one per beat** — rekordbox 5.8.6+ writes
  one per beat, which is why real exports are enormous. rekordbox needs its own BPM and
  grid to agree with ours or **every imported cue drifts**, so correctness matters more
  than fidelity to rekordbox's own output.
- **`POSITION_MARK`**: `Name`, `Type` (0 cue, 1 fade-in, 2 fade-out, 3 load, 4 loop),
  `Start`/`End` (seconds, float), `Num`. The 2011 spec documents `Num` 0/1/2 for hot cues
  A–C; modern rekordbox uses **A–H = 0–7**, which maps our eight slots one-to-one. The
  undocumented `Red`/`Green`/`Blue` attributes carry hot-cue colour. Both are
  **community-verified only** and could change without notice.

**Stream it with `quick_xml::Writer`, never build a DOM** — a 5,000-track collection runs
to tens of MB and millions of elements. Cost: ~1–2 s total for 5,000 tracks.

Emit the full `COLLECTION` **plus one named playlist node** (`Keystone Cue Points`). That
matters more than the format, because **Keystone cannot push anything into rekordbox**:
the user must set Preferences → Advanced/Bridge → Database → Imported Library, enable the
`rekordbox xml` panel, right-click → **Import To Collection**, and answer **Yes** to
*"Do you want to load information in the tag of the library being imported?"* — the step
that actually applies the cues. One playlist node gives them a single thing to
right-click. Mixed In Key uses exactly this pattern, which is why their workflow is
followable. **Whether an incoming hot cue replaces an existing one is not documented
anywhere and is reported inconsistent in the wild.** The export screen must spell the
four manual steps out, or every rekordbox user files the same ticket.

### v1 — Traktor NML. ~1 week.

`~/Documents/Native Instruments/Traktor <x.y.z>/collection.nml`, the same shape on macOS
and Windows. Traktor creates a **new root directory on every version update**, so glob
the parent and pick by version then mtime; and the root is user-overridable in
Preferences → File Management → Directories, so detection is best-effort and the spec's
`CHANGE` button is mandatory, not a nicety.

Hand-roll an **event-stream rewriter** on `quick-xml`. Parsing NML is trivial; preserving
it is not. Entries carry `AUDIO_ID`, `LOUDNESS`, `COVERARTID`, `STEMS`, `FLAGS` and
`MODIFICATION_INFO` that Keystone has no model for, and a parse-into-structs-and-
reserialise pass silently destroys them. Copy bytes through verbatim and splice only the
`CUE_V2` and key children of matched `ENTRY` elements, keeping `COLLECTION/@ENTRIES` in
sync with the child count. ~400 lines. Reading logic may be ported from
`JoergAtGithub/Traktor-Bridge`, which is MIT.

Match tracks on the **`LOCATION` triplet** — `@DIR` (which uses **`/:` as the path
separator**), `@FILE`, and `@VOLUME`/`@VOLUMEID`. Get this wrong and cues land on the
wrong song.

`CUE_V2` attributes: `NAME`, `DISPL_ORDER`, `TYPE` (0 Cue, 1 Fade-In, 2 Fade-Out, 3 Load,
4 Grid), `START`, `LEN`, `REPEATS`, `HOTCUE`. A loop is a cue with `LEN > 0`. The beat
grid is a `CUE_V2` named `AutoGrid` with `TYPE="4"`; the BPM itself lives in the separate
`<TEMPO>` element.

**Two things here are unresolved in public sources and must be settled empirically before
a single key or cue is written. Budget half a day.**

1. **`CUE_V2/@START` units.** A real fixture shows `START="52.315876"` on a 192.078-second
   track, which reads as 52.3 **milliseconds**. A seconds/ms mixup puts every cue at the
   head of the track — the exact symptom of a known Engine DJ bug with the same cause.
2. **`MUSICAL_KEY/@VALUE` numbering.** Mixxx casts it straight into its `ChromaticKey`
   enum (`1 = C major … 12 = B major, 13 = C minor … 24 = B minor`) with the comment
   "Traktor happens to use the same key numbering"; community guides say `0–23`. They
   cannot both be right, the fixture cannot settle it (its `INFO/@KEY` is the independent
   Key Text field, read from the file tag), and **getting it wrong transposes the user's
   whole library silently — no unit test catches it.** Analyse a dozen known-key tracks
   in a real Traktor install and read the numbers back. The `HOTCUE` slot may also be
   off by one from the displayed number; same method.

**Default to writing `INFO/@KEY` (Key Text), with `MUSICAL_KEY` as an advanced opt-in.**
Traktor's Key column is reserved for its own analysis and is backed by `MUSICAL_KEY`,
which Traktor may overwrite on re-analysis; Key Text is the user-facing field read from
the file tag, and it is the only one Mixed In Key writes. Writing `MUSICAL_KEY` directly
is more aggressive than the incumbent and more likely to be clobbered.

Cost: ~1–3 s total for 5,000 tracks, single-pass streaming rewrite.

### v1.1 — Serato. 1.5–2 weeks.

Deferred not because it is undoable — the MPL-2.0 `triseratops` makes it tractable — but
because it is the only target that **rewrites thousands of irreplaceable audio files**,
it spans four containers, and its testing tail is longer than its implementation. The two
XML targets ship first so the safety harness earns its keep where a mistake costs a
restore rather than a re-rip. If the schedule demands one hard target in v1, ship
**Serato MP3-only** and defer AIFF, FLAC and M4A.

What it will involve, recorded now so the estimate is not relitigated:

- **Write `Serato Markers2` *and* `Serato Markers_`, consistently.** Markers2 is base64
  with a linefeed every 72 characters, minimum ~470 bytes null-padded, entries as
  NUL-terminated type string + big-endian `u32` length + payload. `Markers_` is **fixed
  length at 14 slots** (first 5 cues + 9 loops) in `serato32` 7-bit packing — **writing a
  short one destroys the user's existing loops with no error.** When the two disagree,
  Serato prefers `Markers_`.
- Also write `Serato BeatGrid` and `Serato Autotags`. **Skip `Serato Overview`** — it is
  the waveform, tens of KB per file, and Serato regenerates it. **Test `Serato Analysis`:**
  whether omitting or mis-versioning it triggers a full re-analysis is unknown. Leave
  `Serato Offsets_` alone; it is not reverse engineered.
- Containers: ID3v2.4 `GEOB` frames for MP3/AIFF; base64 Vorbis comments for FLAC;
  `----:com.serato.dj:*` freeform atoms for MP4 (note `autgain`, sic). **Never Ogg
  Vorbis** — its layout differs significantly and is undocumented even by Holzhaus, so
  there is nothing to implement against.
- **Serato reads only the first 5 cues from the MP4 `markers` field**, so on M4A the
  legacy tag carries a partial set and Markers2 must carry all eight.
- **FLAC and MP4 cannot be round-tripped losslessly** — Serato writes junk bytes even
  when nothing changed.
- **Stored cue colours are not the colours Serato displays**; there is an undocumented
  palette remap. Verify against a real install.
- **Writing tags does not update Serato's library.** 3.x caches in `_Serato_/database V2`;
  Serato DJ Pro 4 moved to `master.sqlite` under Application Support / `%LOCALAPPDATA%`.
  The user must run **Rescan ID3 Tags** in the Files panel. Without that sentence in the
  UI, the feature gets reported as broken by people for whom it worked perfectly.

Cost: **2–15 minutes** for 5,000 tracks. Tag serialisation is 5–50 ms; it is dominated by
rewriting the audio file when the ID3 block grows and the stream has to shift
(20–200 ms/track on SSD).

### Never — rekordbox ANLZ / `master.db`

`rekordcrate` (MPL-2.0) **parses only**, and there is an open **$200 bounty on its issue
#127** for exactly this export, uncollected. `master.db` is SQLCipher-encrypted (the key
is a constant, but the community extraction already broke once when Pioneer obfuscated
`app.asar`), WAL, and locked while rekordbox runs. And **ANLZ files are not the source of
truth** — rekordbox reads the database, so writing ANLZ without matching DB rows does
nothing at all. Reproducing AlphaTheta's row, UUID and sequence conventions is
research-grade and breaks on point releases.

**Say this in the UI** rather than leaving a checkbox that implies it.

### The `Initial Key` field, per container

| Container | Field | Read by |
|---|---|---|
| MP3, AIFF, WAV | **`TKEY`** (ID3v2.3/2.4) | Serato's Key column; Traktor's **Key Text**; rekordbox |
| MP4, M4A | **`----:com.apple.iTunes:initialkey`**, plus a Comment fallback | Patchy — a Serato report has AAC showing key only in Comments, so write both |
| FLAC, Ogg | **`INITIALKEY`** and `KEY` | Write both; there is no single convention |

**`TKEY` has a spec-legal value set and Camelot is not in it.** ID3v2 restricts it to at
most 3 characters from `A`–`G`, `b`, `#`, `m`, plus `o` for off-key — so `Am`, `F#m`, `Bb`
are legal and `11A` is not. Mixed In Key and others write Camelot anyway and the DJ apps
accept it, but third-party taggers may reject or mangle it. **Tie this to the existing Key
Notation setting and default the tag write to sharps/flats, with Camelot an explicit
opt-in flagged non-standard** — do not invent a second control.

**Unknown:** nothing public states which notation Mixed In Key puts in rekordbox XML's
`Tonality`. Settle it by inspecting one MIK export.

---

## 7. Risks, ranked, and how key accuracy gets measured

### 7.0 Status: what has actually been measured, as of 2026-10-31

This section was written before the engine existed. The engine now runs S1 to S11 on a
real file, which means several of the tests below have been performed and the rest can
now be told apart from the ones that have not. Read this before the plan underneath it.

**The ship gate in §7.1 has not been run.** The 600-track GiantSteps+ set is not on this
machine, and nothing has been scored against an independently annotated corpus. The
gate stands exactly as written, untested, and **Keystone still publishes no accuracy
number.**

**What was run instead: `docs/ACCURACY.md`.** 1,063 real files from one DJ's library,
scored against the Camelot labels Mixed In Key had written into their filenames, through
the shipped `eval::mirex` scorer unmodified. That is **agreement with a competitor, not
accuracy** — the references are Mixed In Key's opinion and where the two disagree
neither is known to be right — and its figures are **not comparable with the 75.70 or
the 76.0 gate**, which are scored against GiantSteps' human annotations. Different
corpus, different reference, different quantity. The full method, every caveat and the
reproduction commands are in that document; what belongs here is what it changes about
the risks:

- **The engine reads real audio end to end.** 1,062 of 1,063 files decoded and keyed; the
  one failure was a truncated container, reported as such. Six more were keyed and then
  refused by a later stage for want of a beat grid. Nothing was dropped silently.
- **A real defect was found that synthetic testing could not see, and it is the
  headline.** `key::fuse`'s `SPREAD_FLOOR` (0.08) and `SPREAD_FULL` (0.14) were fitted
  on synthetic chords and read near zero on real records, so confidence collapses:
  **exactly 0.000 on 64.3% of files, below the ambiguity floor on 88.4%.** The product
  would show a DJ no key badge on almost their whole library. The answers underneath
  those zeros are *not* bad — full club tracks in the lowest confidence band still match
  Mixed In Key 54.91% exactly — so the ordering carries information and only the mapping
  onto 0–1 is wrong. This is a calibration fault, it is the one unambiguous defect in the
  run, and nothing else matters until it is fixed.
- **The residual key error has a direction.** Over full-track answers with the right
  letter, 21.7% sit one step clockwise (the dominant, +7 semitones) against 5.0% on the
  subdominant — 4.3x asymmetry, so a per-track tie-break failure, not a constant
  rotation (rotating every estimate by −7 drops full tracks from 71.00 to 19.90). The
  runner-up is Mixed In Key's answer in 75% of those cases, which is the first real
  evidence that the honest-ambiguity UI earns its place rather than merely sounding
  honest.
- **Material difficulty dominates everything.** Full club tracks against isolated
  acapellas is 71.00 weighted versus 40.35. Risk 10 below has arrived on schedule.
- **There is no minor bias.** Keystone's estimates are *less* minor than the reference
  labels (0.788 against 0.894), so the `edmm` failure mode — post a good weighted score
  by always answering minor — is not what is happening.
- **Tempo is in materially better shape than key**, and is not where the remaining error
  is: 88.29% integer-exact on full tracks with a 0.00–0.01 BPM median absolute error.
  The Accuracy1-to-Accuracy2 gap is 4.00 points on full tracks and 12.38 on sparse-beat
  vocals, which is §7.4's octave problem showing up exactly where it was predicted to.
- **The measurement is stable.** 42 track identities present as two or more files
  (usually an MP3 and an AIFF of the same recording) returned the same key on 42 of 42,
  with a median tempo spread of 0.00 BPM. The engine is deterministic and codec-robust
  across those pairs, so the disagreements are method differences and not run noise.
- **Tuning does real work on roughly one file in six.** Median −2.5 cents, and 17.9% of
  files beyond ±20 cents. Worth recording against §2's note that Faraldo and Sha'ath
  both measured no aggregate gain on digitally-produced EDM: this corpus is not all
  digitally-produced EDM.

**Risk-by-risk deltas.** Numbering follows §7.2.

| # | Change |
|---|---|
| 1 | Unchanged and still the deciding risk. **S-KEY has not been re-scored** — there is no bundled graph in the tree (`SkeySession::from_path` is `todo!()`), so everything measured so far is the template voter alone. Read `ACCURACY.md` §3 as a floor for the engine's eventual configuration, not as its score. |
| 6 | Untouched. The energy quantile ladder still has no calibration corpus behind it, and the ten tracks spot-checked during the build scored 8, 9 or 10 — a scale that cannot separate those is not yet a scale. Fixing it is an `ENERGY_MODEL_VERSION` bump, so do it once, from a corpus. |
| 8 | **Partly measured, and its premise is reversed.** The whole pipeline ran 1,063 files in 656 s wall at six workers in a release build on an M-series machine (~1.6 tracks/s sustained, so a 20,000-track library is roughly three and a half hours here — still over the line the risk was worried about). But decode is **not** the 90% the plan assumed: measured through the finished pipeline, the structure stage's HPSS median filters cost more. `frontend/hpss.rs` is ~3.6 s of a five-minute track. The `criterion` decode benchmark exists (`benches/decode.rs`) and **has not been run against a real multi-format corpus** — it needs `KEYSTONE_BENCH_CORPUS` and reports that it was skipped rather than inventing a figure from synthetic audio. |
| 10 | **Arrived, as predicted.** Acapellas sit 30 weighted points below full club tracks, and five had no usable beat grid at all. A thin monophonic vocal may simply not have a key the 24-class taxonomy can carry; the honest output is a refusal, but it has to be a refusal the UI *explains* rather than the current silent all-zero confidence. The awkward-material fixture set is still not assembled. |
| 12 | **Done and tested.** `core/renamer.rs` is journal-first through `db/track_repo/journal.rs`, with the preview as the undo and `recover_journal` replayed at launch. All five named hazards are pinned by tests in `core/renamer_apply_tests.rs`, which drive the wired `plan`/`apply` pair against a real database and a real directory, plus a sixth for a crash *between* the two steps of a case-only rename. |
| 14 | Unchanged. **Every timing in this document and in `ACCURACY.md` is from an M-series Mac.** There is still no Windows measurement, and Windows is roughly two thirds of installs. |

Risks 2, 3, 4, 5, 7, 9, 11 and 13 are unchanged by this run.

### 7.1 The key-accuracy gate — exactly how we measure it

This is the stated goal, so it is a release gate, not a milestone.

- **Dataset: GiantSteps+ EDM Key, Zenodo record 1095691.** `audio.zip` is
  855,761,126 bytes and verified reachable; `keys.zip` is 168 KB. **CC BY-SA 4.0.** The
  original Beatport preview URLs the 604-track set used are **404 dead** — every one
  tested. This is the revised **600-track** set with confidence levels and key-change
  annotations, **not byte-identical to the 604 the published numbers use**, so our
  re-scores will differ slightly from the literature. **State that every time we report
  a number.** Differences of a point or two mean nothing.
- **Scoring: the MIREX weighted score**, `w = r_correct + 0.5·r_fifth + 0.3·r_relative +
  0.2·r_parallel` (ISMIR 2018 §3.2). The harness reports **all five categories plus exact
  match, mode accuracy and key-signature accuracy separately** — never just the weighted
  total. Two reasons: most residual error is **mode**, which is exactly Camelot's A/B
  letter and the thing a DJ sees; and GiantSteps is **84.8% minor**, so a minor-biased
  model posts a good weighted score while being useless (edmm is the worked example —
  72.0 on GiantSteps, 63.8 modally balanced, 48% parallel errors on The Beatles).
- **The number to beat: 75.70 MIREX weighted / 69.37% exact.** Ship gate: the fused
  engine must reach **≥ 76.0 weighted on the 600-track set** before Keystone publishes any
  accuracy claim. Below that, ship the product and **publish no number** — publishing one
  without naming the measure is precisely what we criticise MIK for. **Status: not run.**
  The corpus is not on this machine, the gate is neither met nor tested, and the
  agreement run in `docs/ACCURACY.md` does not substitute for it.
- **Licence boundary:** evaluating against CC BY-SA data does not infect MIT code.
  **Deriving and shipping key profiles from it does** raise ShareAlike, so profiles come
  from the papers' descriptions or the owner's own annotated library.
- **Build the harness before the detector.** First commits: the harness, then re-score the
  community S-KEY ONNX export ourselves to confirm the claimed 72.1 — nobody has done
  that, and the export is a community conversion, not an official Deezer release
  (author-reported torch parity <4e-6, and it does reproduce the repo's expected
  `D# Major` on the reference file). **Status: the harness is built** (`src/eval/`,
  feature `eval`; `eval/mirex.rs` has eight tests and was cross-checked against an
  independent reimplementation, which caught one bug in the checking script and none in
  the scorer). **The S-KEY re-score has not happened** — there is no graph in the tree.

**Expected trajectory, stated as expectations not promises:** S-KEY alone ~72.1;
+ tuning correction, no measured aggregate gain on EDM but tail insurance; + bgate
ensemble, the cheapest real gain because the two voters have uncorrelated error profiles
(a CNN on VQT and a cosine match on whitened HPCP fail on different tracks); + a
fine-tuned head, ~76–78. **The last step is research-grade.** Fine-tuning the MIT
`MusicalKeyCNN` AllConv (73.51 already) on GiantSteps-MTG's 1,077 high-confidence
excerpts plus the owner's library with ±4/+7 semitone pitch augmentation, exported to
ONNX, is the step that actually beats the bar — and it is not a download. **Plan v1 on the
ensemble and treat beating MIK as a v1.1 research track**, with the honest-ambiguity UI as
the differentiator that does not depend on winning.

### 7.2 Ranked risks

| # | Risk | Cheapest early test |
|---|---|---|
| 1 | **No openly licensed model beats MIK today.** S-KEY is 3.6 MIREX points short, and the gap closes only through ensembling plus training our own head. Anyone planning on "bundle S-KEY and we beat MIK" is planning on a deficit. | The eval harness, in week one, scoring S-KEY alone and S-KEY + bgate on GiantSteps+. That one number decides whether the research track is funded. |
| 2 | **Traktor's `MUSICAL_KEY` numbering and `CUE_V2/@START` units are unresolved.** Either one wrong writes a silently broken library for every Traktor user, and no unit test catches it. | Half a day with a real Traktor install: analyse a dozen known-key tracks, read the numbers back, place a cue at a known time and read `START`. Do this **before** writing the exporter, not after. |
| 3 | **Cue points will not be "accurate" in the sense a user expects.** Published F1 against real DJ cues is 0.22 for MIK and 0.39 for the best model in the literature. If the UI presents eight markers as answers, the feature reads as broken however good the DSP is. | Settle the copy and the interaction (labelled, draggable, "starting points") before the detector is tuned. Then measure against M-DJCUE locally, treating 8.9 usable candidates per track as the real target rather than F1. |
| 4 | **A careless bulk tag write destroys every DJ tag in the library.** lofty's generic `Tag` discards unmapped frames, so one REPLACE ALL could delete every Serato GEOB and ReplayGain frame invisibly. | A round-trip fixture test on a Serato-tagged MP3, AIFF and FLAC, as a hard gate on the write path. One afternoon, and it must exist before the first write ships. |
| 5 | **Stack's hasher would silently wipe the analysis cache.** It hashes the file head, where ID3 lives, so our own re-tag pass changes every `content_hash`. It looks like it works until the first re-tag. | Fix it before the cache is built: hash from the first audio packet offset, and add a test that re-tagging a file leaves `content_hash` unchanged. |
| 6 | **Energy feeling right across genres is not solvable by analysis alone.** If per-genre mean energies end up differing by more than the per-genre spread, the scale has silently become a genre detector and the column loses its meaning. | The five-test ladder below — tests 1 and 2 are free and automated and catch the loudness trap on day one. |
| 7 | **Neural inference cost is the schedule risk.** Beat This! saturates at 60–68× realtime however threads are arranged: 4.2 h for 3,000 tracks full-track. The sampled-window approach is unvalidated and int8 quantisation is unmeasured. | The §2 gate: 200-track agreement test between the classical downbeat phase and Beat This!. If classical wins, 190 MB and the whole risk disappear. |
| 8 | **The decode budget is extrapolated, not measured.** The 0.43 s figure is ffmpeg's hand-optimised MP3 path; symphonia's 2–4× multiplier is an assumption. At worse than 4×, a 20,000-track library crosses two hours. | A `criterion` benchmark over twenty real tracks spanning MP3/AIFF/FLAC/M4A. **First task of the build.** Build the analysis queue resumable across restarts regardless. |
| 9 | **The energy patent.** US 8,865,993 is Active until ~2033, held by the direct competitor, and an open-source MIT release is a visible target. The transparent-formula design reads outside the claims as written, but that is an engineering opinion. | A real legal opinion before the energy feature ships. Keep the definition documented and reference-free in the meantime. |
| 10 | **Everything here is tuned for four-to-the-floor.** Swung house and UK garage break the autocorrelation; half-time trap makes the octave genuinely ambiguous rather than wrong; live, acoustic, classical and rubato material break the beat grid, the phrase model and the cue pipeline together. Key detection also degrades by genre — blues under 50 MIREX, jazz and hip-hop weak, country over 90 — and the 24-key taxonomy simply does not fit some of it. | Decide **now** whether those tracks get a low-confidence result or an explicit "could not analyse". Assemble a 50-track awkward-material fixture set (swung, half-time, live, acoustic, ambient) and run it at every milestone. No model work fixes a taxonomy mismatch; the honest answer is to show ambiguity. |
| 11 | **S-KEY's weights have no separate licence file.** They sit inside an MIT repo, which is a reasonable reading but an inference rather than a written weight licence. This is the single legal dependency the whole key plan rests on. | One email to Deezer Research, sent in week one. |
| 12 | **Renaming is the only irreversible operation**, and one setting runs it automatically after processing. APFS case-insensitivity and macOS NFD filenames both produce silent wrong behaviour rather than errors. | Journal-first with a dry-run preview and an undo, plus unit tests for: case-only rename via a temp name, NFD vs NFC comparison, idempotence (`Track - 8A - 8A` must be impossible), Windows-illegal characters and reserved names, and the 255-byte component limit truncating the *Original* segment never the key/tempo suffix. |
| 13 | **Verification of cue export cannot be automated.** Byte formats unit-test fine, but "Serato shows the cue in the right place" needs three applications installed and a fixture library on a real machine. That manual loop, not the parsing, is probably where the schedule actually goes. | Build the fixture library (MP3/AIFF/FLAC/M4A, known cue positions) at the same time as the safety layer, and budget the manual loop explicitly. |
| 14 | **All timings are from an M2 Max with 8 performance cores. There is no Windows measurement**, and Windows is roughly two thirds of installs. | Re-measure the full pipeline on a mid-range Windows laptop before any progress estimate or time remaining is shown to a user. |

### 7.3 Validating energy without labelling thousands of tracks

Five tests, cheapest first. The first two are free and automated.

1. **Invariance.** Energy must not move by more than ±1 under ±6 dB gain, MP3 320/192/128
   re-encode, 44.1↔48 kHz resample, and ±1 dB of gentle wide-Q EQ. Fifty tracks, as a
   unit test. Any feature that fails is mis-specified. **This catches the loudness trap on
   day one.**
2. **Within-track ordering.** Per-section energies must satisfy `intro ≤ build ≤ drop` and
   `breakdown < drop` on tracks with unambiguous structure. The segmentation already
   exists, so this costs nothing and is a genuinely strong test.
3. **Edit-pair test.** Radio edit vs extended mix of the same track must land within 1.
   ~100 pairs assembled out of a real library by filename matching, no listening.
4. **Pairwise human anchors.** ~300 random pairs stratified across the range, forced
   choice, "which is more intense in a club" — about 45 minutes of listening. Score with
   **Kendall's tau**; **target tau ≥ 0.6, and below 0.45 the formula is wrong rather than
   miscalibrated.** Fit the seven weights on these with a Bradley–Terry loss; 300
   comparisons is plenty for 7 parameters.
5. **Cross-genre consistency — the test that answers the actual requirement.** Restrict
   the pairwise set to cross-genre pairs (techno vs house vs DnB vs hip-hop vs ambient).
   If tau on cross-genre pairs is materially below tau within genre, the score tracks
   genre rather than energy. Also require that per-genre **mean** energies differ by less
   than the per-genre **spread**. That is the measurable version of "a 7 on techno and a 7
   on house mean the same thing".

Budget two or three iterations of 4 and 5. It will not be right the first time.

### 7.4 Tempo validation

**GiantSteps Tempo** (664 EDM tracks; the UPF record says CC-BY while mirdata lists
BY-SA — **check the repo before relying on it**), plus its ISMIR 2018 crowdsourced v2
annotations. The re-annotation paper exists precisely because state-of-the-art estimators
do badly on it, so expect humbling numbers rather than flattering ones. Report Accuracy1
and Accuracy2 separately — the gap between them *is* the octave problem.

---

## 8. What v1 does not do

Listed so nobody builds it by accident and nobody promises it.

- **The Mashup / DJ Mix two-deck surface is out of v1.** Confirmed in `SPEC.md` §5. It is
  effectively a second product, and stem separation alone is a large dependency. The data
  model does not foreclose it: `cue_points`, `track_regions` (per-section energy and key)
  and `beatgrid` are already addressable per track region, which is what a deck view would
  read from, and chunked key inference already produces per-region key.
- **Stem separation and acapella extraction.** Nothing.
- **Writing into rekordbox's own database.** rekordbox XML only, and the UI says so rather
  than offering a checkbox that implies more.
- **Serato export.** v1.1. If forced into v1, MP3-only, with AIFF/FLAC/M4A deferred.
- **Serato on Ogg Vorbis.** Never. The layout is undocumented even by the people who
  reverse-engineered the rest.
- **Beating Mixed In Key's published key accuracy.** v1 ships the tuning-first ensemble
  and expects to land near, not above, 75.70 MIREX. The trained head that actually beats
  it is a v1.1 research track. v1 ships honest ambiguity instead and **publishes no
  accuracy number** until the gate in §7.1 is met.
- **Audio-derived BPM and key for material that is not broadly 4/4 and tempo-stable.**
  Live, acoustic, classical, rubato, swung and half-time tracks get a low-confidence
  result or an explicit "could not analyse". No confident wrong answers.
- **Two-decimal BPM on variable-tempo tracks.** Gated on a grid residual sd under 15 ms.
- **Chord detection, per-bar harmony, and key-change timelines in the UI.** The per-chunk
  keys are stored; nothing in v1 displays them.
- **Genre classification.** `genre` is whatever the file's tag says.
- **Audio correction** — clip repair, loudness normalisation, de-essing. That is Capstone.
  Keystone only flags (`Display a message if the file can be improved`) and points at it.
- **Writing Traktor's `MUSICAL_KEY` by default.** Advanced opt-in only.
- **Camelot in `TKEY` by default.** Non-standard; explicit opt-in.
- **Any cloud or network call in the analysis path.** Including model downloads: if a model
  cannot be bundled, it does not ship.
