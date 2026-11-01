# Keystone — agreement with Mixed In Key on real audio

**Measured 2026-10-31.** Engine: `analysis_version` 1, energy model 1, template
voter alone (no S-KEY graph in the tree). Machine: Apple M-series, 12 logical
cores, macOS 25.6.

---

## 0. Read this before the numbers

**This measures AGREEMENT WITH A COMPETITOR, NOT ACCURACY.**

Every reference label below was written into a filename by Mixed In Key's own
rename feature. Those labels are Mixed In Key's **opinion**. They are not ground
truth, nobody checked them against a score or an annotator, and where Keystone
and Mixed In Key disagree **neither is known to be right**. A number in this
document says how often two programs say the same thing. It does not say how
often either one is correct, and it must not be quoted, paraphrased or
summarised as an accuracy figure anywhere.

Two further consequences that matter more than they look:

- **These figures cannot be compared with the 75.70 in ENGINE.md §7.1.** That
  75.70 is Mixed In Key scored against GiantSteps' human annotations. The
  figures here are Keystone scored against Mixed In Key. Those are different
  quantities with different references. Putting them in the same sentence as if
  one were above or below the other is wrong, however tempting the arithmetic
  looks.
- **The ship gate in ENGINE.md §7.1 is neither met nor tested by this run.** The
  gate is ≥ 76.0 MIREX weighted on the 600-track GiantSteps+ set against *its*
  annotations. That corpus is not on this machine and was not used. Nothing here
  licenses publishing an accuracy claim.

What this run *is* good for: it is the first time the engine has been pointed at
a thousand real files, so it establishes that the decode and key paths work end
to end on real audio, it shows where Keystone and the incumbent systematically
diverge, and it exposes two defects that synthetic testing could not see.

---

## 1. Corpus

Every file on this machine whose name ends in Mixed In Key's recommended rename
pattern, `<original> - <camelot> - <tempo>`:

```
find ~/Downloads ~/Music ~/Documents ~/Desktop ~/Movies -type f \
  \( -iname '*.mp3' -o -iname '*.wav' -o -iname '*.flac' -o -iname '*.m4a' \
     -o -iname '*.aiff' -o -iname '*.aif' \) \
  | grep -E ' - (1[0-2]|[1-9])[AB] - [0-9]{2,3}\.[A-Za-z0-9]+$'
```

1,098 paths matched. 34 basenames occurred more than once (the same download
filed in several project trees), accounting for 35 paths, leaving **1,063
distinct files**. The three folders named in the brief were never thinned; a
duplicate was always dropped from the other groups.

| group | n | what it is | median duration |
|---|---|---|---|
| `club` | 50 | `~/Downloads/Swendl Crate - Miami House 2026/Songs` — full club tracks | 202 s |
| `vocals` | 316 | `~/Music/Samples/Vocal/Billboard Vocal` — vocal-led pop/rap, mostly Russian-language | 192 s |
| `acapella` | 396 | `~/Music/Samples/Vocal/On Point Mega Acapella Pack` — isolated acapellas | 191 s |
| `capstone` | 42 | `~/Music/Capstone` — full club tracks, AIFF | 209 s |
| `readyfile` | 229 | `~/Music/Ready File/**` — DJ-ready full tracks across 8 subfolders | 174 s |
| `misc` | 30 | full tracks scattered through `~/Music/Project/**`, `~/Desktop/Other` | 170 s |

The brief cited 1,033 and named the first three folders (50 / 316 / 390). The
first three are reproduced exactly at 50 / 316 / 396. The remaining 301 files
are the rest of the machine-wide sweep; they are kept in their own groups and
never folded into the three.

Label distribution: **89.4% of the Mixed In Key labels are minor** (`A` on the
wheel). That is close to GiantSteps' 84.8%, and it is why mode accuracy and both
minor fractions are reported beside every weighted total and never folded into
it: a minor-biased detector posts a respectable weighted score on a corpus like
this while being useless.

Camelot was converted to (pitch class, mode) with the inverse of
`Camelot::to_key` — `B` → `(7·(n−8) mod 12, major)`, `A` → `(7·(n−5) mod 12,
minor)`. Spot-checked: `8B` = C major, `8A` = A minor, `7A` = D minor.

---

## 2. Method

Two passes over the same 1,063 files.

**Pass A — key, scored by the shipped scorer.** Each group was mirrored into a
directory of `.key` annotation files beside an `audio/` directory of symlinks,
which is the layout `eval::giantsteps::load` reads, and scored with the shipped
harness:

```
cargo build --release --features eval --bin keystone-eval
./target/release/keystone-eval <mirror>/<group>
```

That runs `eval::run` → `key_path::run` (S2 tuning → S3 chroma → S4 key) →
`mirex::score`. **Every MIREX figure in §3 came out of
`src-tauri/src/eval/mirex.rs` unmodified.** Note that the harness prints the
hardcoded label `GiantSteps+` because `bin/eval.rs` has that string in it; the
corpus is the one in §1 and is not GiantSteps.

**Pass B — the whole engine, per file.** `keystone-eval --track <file>`, which
runs S1..S11 through `analysis::analyze_track` — the same path the app uses —
and prints key, key confidence, the mode-ambiguity flag, the runner-up, measured
tuning, tempo and tempo confidence. This is where §4, §5 and §6 come from. Six
workers; 1,063 files in 656 s wall.

Both passes run the identical key stage: `key::detect` is
`detect_detailed(..).result`, and `skey` is `None` in both because there is no
bundled graph. Where both passes covered the same files they agreed exactly
(§2.1).

**Tempo** is scored on the engine's *displayed* BPM, folded into the default
79–192 range, against the integer in the filename. Accuracy1 is within 4%;
Accuracy2 also credits the label × {3, 2, ½, ⅓}, so the gap between them is the
octave problem, as ENGINE.md §7.4 asks.

**A refusal is counted as its own outcome, not as an error.** Keystone reports a
result as ambiguous — no key badge — when confidence falls below
`fuse::AMBIGUOUS_CONFIDENCE_FLOOR` (0.25), and shows the relative pair rather
than a letter when `mode_ambiguous` is set. §4 reports both, and §3 scores every
track regardless, because a score computed only over the tracks a detector felt
good about is not a score.

### 2.1 The scorer was cross-checked, and one bug was caught

MIREX was also implemented in Python for the per-track slicing in §4–§6. It is
only trustworthy if it agrees with the Rust original, so:

- All 8 tests in `eval/mirex_tests.rs` were ported assertion by assertion and
  pass against the Python port, as they do against the Rust (`cargo test
  --features eval --lib eval::mirex`: 8 passed).
- On the four groups where pass B reached every file that pass A scored —
  `club` (50), `capstone` (42), `readyfile` (228), `misc` (30), **350 tracks** —
  the Rust harness and the Python port agree to the printed decimal on every
  reported figure: weighted, exact, all five category counts, mode accuracy,
  key-signature accuracy, both minor fractions, the subdominant count and the
  weighted total under the other fifth convention. Recombining the Rust
  per-group counts into the `FULL TRACKS` aggregate also reproduces the Python
  figure exactly (71.00 vs 71.00, n = 350 vs 350).
- A first cut of the repeatability check in §7 collapsed every Cyrillic filename
  to an empty identity and invented a 14-file "duplicate group" out of unrelated
  tracks. It is fixed and the corrected figure is in §7. Recorded because it is
  the exact failure mode this document is supposed to guard against.

Where the two passes cover different files, the Rust figure is the one reported,
because it covers more of the corpus.

### 2.2 What failed to read, counted rather than dropped

| outcome | n | detail |
|---|---|---|
| decoded and keyed | 1,062 | — |
| decode failure | 1 | `Bloody Boy - Dance And Fun Original Mix…- 9A - 128.mp3`: `unsupported or damaged container (end of stream)`. A truncated file, not an engine defect. |
| keyed, but the full pipeline refused the file | 6 | 5 `acapella` + 1 `vocals`. `no usable beat grid (1–7 beats, tempo confidence 0.00–0.41)` on five, `energy needs at least one bar inside the span` on one. |

So §3 is over **1,062** tracks and §4–§6, which need pass B, are over **1,056**.
The six are worth noting in their own right: the key stage produced an answer on
all of them, and a later stage then refused the file outright, which means an
acapella with no detectable beat currently yields no key in the app either.

---

## 3. Key agreement with Mixed In Key — scored by `mirex.rs`

Fifths credited: **dominant only** (`est = ref + 7`, same mode), which is what
`mir_eval` implements. The subdominant count and the weighted total that would
result from crediting it are in the last two columns; a figure computed one way
must never be compared with one computed the other.

### Per folder

| group | n | **weighted** | exact | correct | fifth | relative | parallel | other | mode acc | key-sig acc | MIK minor | Keystone minor | subdom | w (both fifths) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `club` | 50 | **66.60** | 56.00% | 28 (56.0%) | 8 (16.0%) | 1 (2.0%) | 5 (10.0%) | 8 (16.0%) | 84.00% | 58.00% | 0.940 | 0.820 | 4 | 70.60 |
| `capstone` | 42 | **66.90** | 54.76% | 23 (54.8%) | 8 (19.0%) | 1 (2.4%) | 4 (9.5%) | 6 (14.3%) | 85.71% | 57.14% | 0.929 | 0.833 | 3 | 70.48 |
| `readyfile` | 228 | **74.12** | 62.72% | 143 (62.7%) | 43 (18.9%) | 7 (3.1%) | 12 (5.3%) | 23 (10.1%) | 88.16% | 65.79% | 0.917 | 0.833 | 8 | 75.88 |
| `misc` | 30 | **60.33** | 46.67% | 14 (46.7%) | 6 (20.0%) | 3 (10.0%) | 1 (3.3%) | 6 (20.0%) | 66.67% | 56.67% | 0.733 | 0.533 | 0 | 60.33 |
| `vocals` | 316 | **41.08** | 28.16% | 89 (28.2%) | 70 (22.2%) | 6 (1.9%) | 20 (6.3%) | 131 (41.5%) | 81.33% | 30.06% | 0.896 | 0.797 | 27 | 45.35 |
| `acapella` | 396 | **40.35** | 29.29% | 116 (29.3%) | 70 (17.7%) | 14 (3.5%) | 23 (5.8%) | 173 (43.7%) | 79.80% | 32.83% | 0.881 | 0.765 | 35 | 44.77 |

### Aggregates

| set | n | **weighted** | exact | correct | fifth | relative | parallel | other | mode acc | key-sig acc | MIK minor | Keystone minor | subdom | w (both fifths) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| the three named folders | 762 | **42.38** | 30.58% | 233 (30.6%) | 148 (19.4%) | 21 (2.8%) | 48 (6.3%) | 312 (41.0%) | 80.71% | 33.33% | 0.891 | 0.782 | 66 | 46.71 |
| all full tracks (`club`+`capstone`+`readyfile`+`misc`) | 350 | **71.00** | 59.43% | 208 (59.4%) | 65 (18.6%) | 12 (3.4%) | 22 (6.3%) | 43 (12.3%) | 85.43% | 62.86% | 0.906 | 0.805 | 15 | 73.14 |
| everything | 1,062 | **50.67** | 38.89% | 413 (38.9%) | 205 (19.3%) | 32 (3.0%) | 65 (6.1%) | 347 (32.7%) | 82.11% | 41.90% | 0.894 | 0.788 | 77 | 54.29 |

Three things to read off this, none of them a single number:

1. **Material difficulty dominates.** Full club tracks sit at 71.00 weighted /
   59.43% exact; isolated acapellas at 40.35 / 29.29%. That was predicted in the
   brief and it is confirmed: a thin monophonic vocal gives the chroma stage
   almost nothing to correlate against. Quoting the all-corpus 50.67 as if it
   described club performance would be meaningless, which is why the three
   difficulties are never pooled into a headline.
2. **There is no minor bias.** Keystone's estimates are consistently *less*
   minor than Mixed In Key's labels (0.788 vs 0.894 overall, −0.106). The `edmm`
   failure mode — post a good weighted score by always answering minor — is not
   what is happening here. If anything Keystone over-calls major.
3. **Mode is in better shape than the weighted total suggests.** Mode accuracy
   is 85.43% on full tracks against 59.43% exact, so most of the disagreement is
   *which* key, not major-versus-minor. That is the opposite of the residual
   error profile the literature reports for published systems, and §6 says why.

---

## 4. How often Keystone refused to answer

A refusal is a different outcome from a wrong answer, and the product treats it
as such: below `AMBIGUOUS_CONFIDENCE_FLOOR` (0.25) there is no key badge at all.
n = 1,056 (pass B).

| group | n | refused (conf < 0.25) | `mode_ambiguous` set | mean conf | median conf | p95 conf |
|---|---|---|---|---|---|---|
| `club` | 50 | 47 (94.0%) | 47 (94.0%) | 0.062 | 0.000 | 0.280 |
| `capstone` | 42 | 37 (88.1%) | 38 (90.5%) | 0.063 | 0.000 | 0.270 |
| `readyfile` | 228 | 200 (87.7%) | 201 (88.2%) | 0.078 | 0.000 | 0.320 |
| `misc` | 30 | 24 (80.0%) | 25 (83.3%) | 0.134 | 0.090 | 0.510 |
| `vocals` | 315 | 282 (89.5%) | 283 (89.8%) | 0.065 | 0.000 | 0.360 |
| `acapella` | 391 | 343 (87.7%) | 346 (88.5%) | 0.077 | 0.000 | 0.440 |
| all full tracks | 350 | 308 (88.0%) | 311 (88.9%) | 0.079 | 0.000 | 0.320 |
| **everything** | **1,056** | **933 (88.4%)** | **940 (89.0%)** | **0.074** | **0.000** | **0.390** |

**This is the most important finding in the document, and it is a defect, not a
result.** On real audio the engine reports a confidence of **exactly 0.000 on
679 of 1,056 files (64.3%)** — 56.9% of full tracks, 68.0% of the `club`
folder — and would show no key badge at all on 88.4% of this library. The
product as it stands would look to a DJ like it had failed to analyse almost
everything.

The answers under those exact zeros are not bad, and on club tracks they are
not even below average: the 34 `club` files reporting 0.000 confidence match
Mixed In Key **58.82% exactly with 91.18% mode accuracy**, against 56.00% and
84.00% for the `club` folder as a whole. The engine is at its most useful
precisely where it claims to know nothing.

The cause is identifiable and is already flagged in the source. `fuse::SPREAD_FLOOR`
(0.08) and `SPREAD_FULL` (0.14) gate the evidence term, and their own comment
says they were measured on synthetic material — "diatonic progressions at
0.13–0.15" — and are "provisional until calibrated on a real corpus." Real
records land below the 0.08 floor, so the evidence term saturates at zero and
takes the confidence with it. This corpus is that real corpus.

What makes it a calibration fault rather than a detection fault is that the key
answers underneath those zero confidences are fine:

| confidence band | n (all) | weighted | exact | mode acc | | n (full tracks) | weighted | exact |
|---|---|---|---|---|---|---|---|---|
| [0.00, 0.05) | 746 | 47.69 | 35.52% | 86.19% | | 224 | 67.41 | 54.91% |
| [0.05, 0.10) | 72 | 54.44 | 41.67% | 65.28% | | 27 | 68.89 | 55.56% |
| [0.10, 0.15) | 53 | 61.51 | 50.94% | 71.70% | | 32 | 75.00 | 68.75% |
| [0.15, 0.25) | 62 | 53.06 | 38.71% | 66.13% | | 25 | 70.80 | 52.00% |
| [0.25, 1.00] | 123 | 62.28 | 53.66% | 78.86% | | 42 | **88.57** | **83.33%** |

Full club tracks in the lowest band — reported as unanalysable — still match
Mixed In Key 54.91% of the time exactly and 67.41 weighted. The scale is
compressed into the bottom of its range, not inverted: the top band is clearly
the best (88.57 weighted, 83.33% exact on full tracks), so the ordering carries
real information and only the mapping onto 0–1 is wrong. On vocals and acapellas
the bands are not monotonic at all, so there the confidence is not discriminative
even in ordering.

Two notes on what that top band is not. It is 42 full tracks, so 88.57 weighted
carries a wide interval and should not be quoted as a result. And it is a
self-selected subset: scoring only the tracks a detector was confident about is
the standard way to manufacture a flattering number, which is why §3 scores
everything.

---

## 5. Tempo agreement with the Mixed In Key tempo in the filenames

Against the engine's displayed BPM, folded into the default 79–192 range.
n = 1,056.

| group | n | Accuracy1 (±4%) | Accuracy2 (± octaves) | integer-exact | octave-only errors | median abs error |
|---|---|---|---|---|---|---|
| `club` | 50 | **96.00%** | 96.00% | 96.00% | 0.00% | 0.00 BPM |
| `capstone` | 42 | **97.62%** | 97.62% | 97.62% | 0.00% | 0.00 BPM |
| `readyfile` | 228 | **88.60%** | 93.86% | 87.72% | 5.26% | 0.01 BPM |
| `misc` | 30 | **66.67%** | 73.33% | 66.67% | 6.67% | 0.03 BPM |
| `vocals` | 315 | **55.87%** | 68.25% | 41.27% | 12.38% | 1.90 BPM |
| `acapella` | 391 | **56.01%** | 61.64% | 34.02% | 5.63% | 2.50 BPM |
| all full tracks | 350 | **88.86%** | 92.86% | 88.29% | 4.00% | — |
| the three named folders | 756 | **58.60%** | 66.67% | 41.14% | 8.07% | — |
| **everything** | **1,056** | **66.86%** | **73.96%** | **54.17%** | **7.10%** | — |

Tempo is in materially better shape than key. On full club tracks the two
programs agree to the integer 88.29% of the time with a median absolute error of
**0.00–0.01 BPM** — when they agree they agree to two decimals, which is the
grid fit working. The Accuracy1→Accuracy2 gap is 4.00 points on full tracks and
12.38 on vocals, so the octave problem is real but small on four-to-the-floor
material and larger where the beat is sparse, exactly as ENGINE.md §7.4
predicts. Acapellas lose on both counts and are not a tempo benchmark: five of
them have no usable beat grid at all.

---

## 6. The ten most interesting disagreements, and what they say

Full tracks only, ranked by the engine's own confidence, because a *confident*
disagreement is the one that carries information. One row per distinct track.
`step` is the Camelot wheel offset; `r-up` is Keystone's runner-up.

| # | Mixed In Key | Keystone | category | offset | conf | runner-up | track |
|---|---|---|---|---|---|---|---|
| 1 | 8A (A minor) | 11B (A major) | parallel | +0 st / +3 step | 0.33 | 12B | Spiller feat. Sophie Ellis-Bextor — Groovejet |
| 2 | 11A (F# minor) | 12A (C# minor) | fifth | +7 st / +1 step | 0.29 | **11A = MIK** | all things break — dots |
| 3 | 2B (F# major) | 3B (C# major) | fifth | +7 st / +1 step | 0.28 | **2B = MIK** | Swendl — Feel Alive |
| 4 | 5A (C minor) | 6B (A# major) | other | +10 st / +1 step | 0.28 | 6A | Olivia Lunny feat. Bryce Vine — Fix This |
| 5 | 6B (A# major) | 7B (F major) | fifth | +7 st / +1 step | 0.27 | **6B = MIK** | If We'll Ever Be Remembered |
| 6 | 4A (F minor) | 5B (D# major) | other | +10 st / +1 step | 0.27 | 6B | Let's Go |
| 7 | 5A (C minor) | 5B (D# major) | relative | +3 st / +0 step | 0.20 | **5A = MIK** | Swendl — Desire |
| 8 | 7B (F major) | 8B (C major) | fifth | +7 st / +1 step | 0.20 | 8A | Post Malone — I Like You (A Happier Song) |
| 9 | 1B (B major) | 2B (F# major) | fifth | +7 st / +1 step | 0.19 | **1B = MIK** | Bag Raiders — Shooting Stars |
| 10 | 2A (D# minor) | 5B (D# major) | parallel | +0 st / +3 step | 0.19 | **2A = MIK** | Lil Nas X — Industry Baby |

They are not scattered. **Five of the ten are the dominant — one step clockwise
on the wheel, +7 semitones, same mode — and in four of those five the runner-up
is Mixed In Key's own answer.** Two are parallel (right root, wrong letter) and
one is the relative. Only rows 4 and 6 are a genuinely different reading, and
both of those are +10 semitones with the mode flipped, which is the relative
major of the dominant: the same clockwise pull with a mode error on top. Counted
by wheel offset rather than by category, **seven of the ten sit exactly one step
clockwise of Mixed In Key**, and the runner-up is Mixed In Key's answer in six
of the ten.

That holds across the whole corpus, not just the top ten. Over full-track
answers that got the letter right (n = 299):

| wheel offset | count | share |
|---|---|---|
| **+0 (agrees)** | 208 | 69.6% |
| **+1 (dominant, +7 st)** | 65 | 21.7% |
| −1 (subdominant, +5 st) | 15 | 5.0% |
| everything else | 11 | 3.7% |

The dominant is **4.3× more common than the subdominant**, so this is a
directional bias and not symmetric confusion. Three measurements pin down what
it is:

- **It is not a global off-by-one.** Rotating every estimate by −7 semitones
  drops full tracks from 71.00 weighted to 19.90. The bias affects about a fifth
  of tracks, not all of them, so it is a per-track tie-break failure and not a
  constant rotation in the profile table or the Camelot mapping.
- **The runner-up usually rescues it.** Among full-track +7 errors that have a
  runner-up, the runner-up is Mixed In Key's answer in **18 of 24 (75.0%)**. On
  vocals 62.5%, on acapellas 30.4%.
- **So top-2 is far better than top-1.** Counting a hit when either the winner
  or the runner-up matches:

| group | top-1 exact | top-2 exact |
|---|---|---|
| `club` | 56.00% | 66.00% |
| `capstone` | 54.76% | 66.67% |
| `readyfile` | 62.72% | 71.05% |
| `misc` | 46.67% | 60.00% |
| `vocals` | 28.25% | 37.14% |
| `acapella` | 29.41% | 36.06% |

### Where the work is

In order of how much they are worth against how hard they look:

1. **Recalibrate `SPREAD_FLOOR` / `SPREAD_FULL` on this corpus.** The constants
   are synthetic-derived, their own comment says so, and they currently suppress
   the key badge on 88% of a real library while the answer underneath is right
   more than half the time. Nothing else in this document matters until a user
   can see a key at all. This is the one defect that is unambiguously a defect:
   the other findings are disagreements with a competitor, this is the product
   not working.
2. **Break the dominant tie in S4.** A fifth of full-track answers are one
   clockwise step out and the right answer is already sitting in second place
   three times out of four. The template profiles correlate nearly as well with
   the dominant as with the tonic — which is exactly what `fuse::separation`'s
   own comment describes — so the fix lives in profile shape or in a tonic-weight
   prior, not in the fusion. This is the largest single accuracy gain visible in
   the data and it needs no model.
3. **Ship the ambiguity UI; the data supports it.** Top-2 beats top-1 by 8–13
   points on full tracks, and the second answer is usually the competitor's. The
   relative-pair display is the stated differentiator, and this is the first
   evidence that it earns its place rather than merely sounding honest.
4. **Decide what an acapella should do.** 40.35 weighted and five files with no
   beat grid at all. A thin monophonic vocal may simply not have a key the
   24-class taxonomy can carry, and the honest output is a refusal — but it has
   to be a refusal the UI explains, not the current silent all-zero confidence.
   This is ENGINE.md risk 10, arriving on schedule.
5. **Leave tempo alone for now.** 88.29% integer-exact on full tracks with a
   0.01 BPM median error is not where the remaining error is.

---

## 7. Is the measurement itself stable?

42 track identities in the corpus exist as two or more separate files — usually
an MP3 and an AIFF of the same recording, in different groups.

- Keystone returned **the same key on every copy: 42 of 42 (100%)**.
- Tempo across copies: median spread 0.00 BPM, maximum 0.40 BPM, nothing above
  1 BPM.
- Mixed In Key's own labels also agreed across all 42.

So the engine is deterministic and codec-robust across these pairs, and the
disagreements in §3 and §6 are genuine method differences rather than run noise.

Tuning, measured on 1,056 real files: median **−2.5 cents**, quartiles −6.8 /
+1.2, range −33.4 to +33.1, and **189 files (17.9%) beyond ±20 cents**. S2 is
doing real work on roughly one file in six, which is worth knowing given that
both Faraldo and Sha'ath measured no aggregate gain from tuning handling on
digitally-produced EDM. This corpus is not all digitally-produced EDM.

---

## 8. Caveats

1. **These are agreement figures, not accuracy figures.** The references are
   Mixed In Key's opinion. Where the two disagree neither is known to be right.
   Nothing in this document may be quoted as an accuracy number, and the word
   "accuracy" appears here only inside the names of measures (Accuracy1,
   Accuracy2, mode accuracy) that are defined against these same opinion labels.
2. **A ceiling, not a target.** Perfect agreement with Mixed In Key would mean
   reproducing its errors too. Published work puts Mixed In Key around 75.70
   MIREX weighted against GiantSteps' annotations, so roughly a quarter of these
   reference labels are likely wrong by a human annotator's reckoning, and some
   share of Keystone's "disagreements" are therefore Keystone being right. This
   measurement cannot tell those apart. **Only an independently annotated corpus
   can**, which is what ENGINE.md §7.1 is for and what has still not been run.
3. **Not comparable with the 75.70 or the 76.0 gate.** Different reference,
   different corpus, different quantity. The gate remains untested.
4. **The labels may be stale.** They were written into the filenames at some
   past time by some version of Mixed In Key. No version was recorded, the files
   may have been re-encoded, re-tagged or replaced since, and a few of the
   `_PN` names show they also went through Platinum Notes, which processes the
   audio. Where a label and the current audio disagree, the label is wrong and
   this run scores it as a Keystone error.
5. **Provenance is inferred.** It is inferred from the suffix matching Mixed In
   Key's own recommended rename pattern (`Original - Key - Tempo`, SPEC.md §3)
   across 1,063 files. It was not confirmed by reading tags or asking the
   library's owner. Anything renamed by hand or by another tool using the same
   pattern would be indistinguishable.
6. **The corpus is one DJ's library, in one genre family.** House and pop,
   89.4% minor labels, overwhelmingly four-to-the-floor. There is no classical,
   jazz, live, acoustic or rubato material, and ENGINE.md itself notes key
   detection degrades sharply by genre (blues under 50 MIREX, country over 90).
   These figures do not generalise past material like this.
7. **Groups are not independent samples.** `club` has n = 50 and `misc` n = 30;
   those two rows carry wide intervals and small differences between them mean
   nothing. No confidence intervals are computed anywhere in this document, so
   no difference of a few points should be read as real.
8. **The three vocal and acapella groups have a second labelling problem.**
   Mixed In Key derived those labels from the same thin monophonic audio
   Keystone is struggling with, so the references themselves are least reliable
   exactly where agreement is lowest. The low acapella figure is evidence about
   the material, not about either program.
9. **This is the template voter alone.** There is no bundled S-KEY graph in the
   tree (`SkeySession::from_path` is still `todo!()`), so the ensemble — named in
   ENGINE.md as the cheapest real accuracy gain available — is unmeasured. Read
   §3 as a floor for the engine's eventual configuration, not as its score.
10. **The tempo reference is an integer.** The filenames carry whole-number BPM,
    so Accuracy1's ±4% window is generous at 80 BPM and tight at 190, and
    integer-exact agreement cannot distinguish 128.00 from 128.49.
11. **One file could not be decoded and six were refused by later stages**
    (§2.2). Nothing was dropped silently: §3 is n = 1,062 and §4–§6 are n = 1,056,
    and both are stated wherever they appear.

---

## 9. Reproducing this

```bash
cd src-tauri
cargo build --release --features eval --bin keystone-eval

# Pass A — key, through mirex.rs. <mirror>/<group>/ holds one
# "<name>.<ext>.key" per track (a line such as "D minor") beside
# <mirror>/<group>/audio/<name>.<ext>.
./target/release/keystone-eval <mirror>/<group>

# Pass B — the whole engine on one file, for confidence, tuning and tempo.
./target/release/keystone-eval --track "<file>"

# The scorer's own tests.
cargo test --features eval --lib eval::mirex
```

The corpus is rebuilt from the `find | grep` in §1, parsing
`<original> - <camelot> - <tempo>.<ext>` and inverting `Camelot::to_key`. Run it
again after any change to S2, S3, S4 or `fuse`, and compare against this table
rather than against a remembered headline figure.

**When this is re-run, the first number to check is the refusal rate in §4, not
the weighted total in §3.** A weighted total that improves while 88% of the
library still shows no key badge has not improved anything a user can see.
