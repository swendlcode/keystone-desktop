# Keystone — agreement with Mixed In Key on real audio

**Re-measured 2026-11-01**, after the key-confidence and energy-scale
calibration. Engine: `analysis_version` 2, energy model 2, template voter alone
(no S-KEY graph in the tree). Machine: Apple M-series, 12 logical cores,
macOS 25.6.

The previous run — **2026-10-31**, `analysis_version` 1, energy model 1, same
corpus, same method, same machine — is reported beside every figure. Nothing
from it was discarded or restated from memory: it was re-parsed from its own
per-track output files by the same code that parsed this one.

---

## 0. Read this before the numbers

**THIS MEASURES AGREEMENT WITH A COMPETITOR, NOT ACCURACY.**

Every reference label below was written into a filename by Mixed In Key's own
rename feature. Those labels are Mixed In Key's **opinion**. They are not ground
truth, nobody checked them against a score or an annotator, and where Keystone
and Mixed In Key disagree **neither is known to be right**. A number in this
document says how often two programs say the same thing. It does not say how
often either one is correct, and it must not be quoted, paraphrased or
summarised as an accuracy figure anywhere.

Three further consequences that matter more than they look:

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
- **New on 2026-11-01, and it is the largest change to how this document must be
  read: the engine now contains constants fitted to this corpus.** Four
  confidence gates in `key::fuse` and fourteen moments plus eleven knots in
  `energy::constants` were fitted on half of the 1,063 files measured here. The
  key *estimates* are untouched by that and §3 is therefore still an honest
  out-of-sample measurement of detection. The *confidence* figures in §4 and the
  *energy* figures in §7 are not: they are measured on a corpus that includes
  the files their constants were chosen on. §4.5 and §7.2 give the held-out
  versions, and those are the ones to believe. See caveat 12.

What this run *is* good for: it establishes that key detection did not move
while the calibration was done — which was the one thing that had to be proved,
and it is proved exactly rather than approximately — and it measures what the
calibration bought and what it cost.

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

**Reconciliation with the 2026-10-31 run.** The sweep was re-run before anything
else and returned **1,098 paths, byte-identical to the previous run's list**:
`diff` of the two sorted outputs is empty. Deduplicating to distinct basenames
gives **1,063 distinct files** in the same six groups at the same counts. The
only difference between the two corpus builds is *which copy's path* was kept
for two files that exist twice under the same basename (`find` does not order
duplicate basenames deterministically). Since §8 shows the engine returns the
same key and tempo on every duplicate copy, that is immaterial — but to keep the
comparison exact, **this run used the 2026-10-31 corpus file unchanged**, so both
runs cover the identical 1,063 paths in the identical order.

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

**The calibration splits.** The confidence fit used a 532/531 half-split of these
1,063 files; the energy fit used a different 532/531 split of the same files,
restricted to the 350 full tracks, of which 176 were its fit half. Both splits
were written to disk before any audio was analysed. What matters for reading this
document is in §4.5 and §7.2.

---

## 2. Method

Two passes over the same 1,063 files, run twice — once per engine version.

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
`src-tauri/src/eval/mirex.rs` unmodified.** The *same mirror directory* was
scored in both runs, so the labels and the files are not merely equivalent but
literally the same bytes. Note that the harness prints the hardcoded label
`GiantSteps+` because `bin/eval.rs` has that string in it; the corpus is the one
in §1 and is not GiantSteps.

**Pass B — the whole engine, per file.** `keystone-eval --track <file>`, which
runs S1..S11 through `analysis::analyze_track` — the same path the app uses —
and prints key, key confidence, the mode-ambiguity flag, the runner-up, measured
tuning, tempo, tempo confidence, energy and the raw energy score. This is where
§4 through §8 come from. Six workers; 1,063 files in 656 s wall on 2026-10-31
and 679 s on 2026-11-01 (the 23 s is CPU contention with the test suite, not a
pipeline change).

Both passes run the identical key stage: `key::detect` is
`detect_detailed(..).result`, and `skey` is `None` in both because there is no
bundled graph. Where both passes covered the same files they agreed exactly.

**Tempo** is scored on the engine's *displayed* BPM, folded into the default
79–192 range, against the integer in the filename. Accuracy1 is within 4%;
Accuracy2 also credits the label × {3, 2, ½, ⅓}, so the gap between them is the
octave problem, as ENGINE.md §7.4 asks.

**A refusal is counted as its own outcome, not as an error.** Keystone reports a
result as ambiguous — no key badge — when confidence falls below
`fuse::AMBIGUOUS_CONFIDENCE_FLOOR`, and shows the relative pair rather than a
letter when `mode_ambiguous` is set. **That floor is 0.25 in the 2026-10-31
engine and 0.12 in the 2026-11-01 one, and every refusal figure in this document
is computed against the floor its own engine shipped**, because that is what a
user of that build would have seen. §3 scores every track regardless of
confidence, because a score computed only over the tracks a detector felt good
about is not a score.

### 2.1 The scorer was cross-checked, and one bug was caught

MIREX was also implemented in Python for the per-track slicing in §4–§8. It is
only trustworthy if it agrees with the Rust original, so:

- All 8 tests in `eval/mirex_tests.rs` were ported assertion by assertion and
  pass against the Python port, as they do against the Rust (`cargo test
  --features eval --lib eval::mirex`: 8 passed).
- On the four groups where pass B reached every file that pass A scored —
  `club` (50), `capstone` (42), `readyfile` (228), `misc` (30), **350 tracks** —
  the Rust harness and the Python port agree to the printed decimal on every
  reported figure. Recombining the Rust per-group counts into the `FULL TRACKS`
  aggregate also reproduces the Python figure exactly (71.00 vs 71.00,
  n = 350 vs 350). That recombination was redone on 2026-11-01 and reproduces
  all three aggregates in §3 to the last decimal.
- A first cut of the repeatability check in §8 collapsed every Cyrillic filename
  to an empty identity and invented a 14-file "duplicate group" out of unrelated
  tracks. It is fixed and the corrected figure is in §8. Recorded because it is
  the exact failure mode this document is supposed to guard against.
- The Python port is unchanged between the two runs, and both runs' per-track
  reports were parsed by one script written for this re-measurement, so a
  difference between the two columns cannot be a difference in the parser.

Where the two passes cover different files, the Rust figure is the one reported,
because it covers more of the corpus.

### 2.2 What failed to read, counted rather than dropped

Identical in both runs — the same seven files, with the same seven reasons:

| outcome | n | detail |
|---|---|---|
| decoded and keyed | 1,062 | — |
| decode failure | 1 | `Bloody Boy - Dance And Fun Original Mix…- 9A - 128.mp3`: `unsupported or damaged container (end of stream)`. A truncated file, not an engine defect. |
| keyed, but the full pipeline refused the file | 6 | 5 `acapella` + 1 `vocals`. `no usable beat grid (1–7 beats, tempo confidence 0.00–0.41)` on five, `energy needs at least one bar inside the span` on one. |

So §3 is over **1,062** tracks and §4–§8, which need pass B, are over **1,056**,
in both runs. The six are worth noting in their own right: the key stage produced
an answer on all of them, and a later stage then refused the file outright, which
means an acapella with no detectable beat still yields no key in the app. The
calibration did not touch that and was not meant to.

### 2.3 What changed between the two runs, and what provably did not

| constant | 2026-10-31 | 2026-11-01 |
|---|---|---|
| `fuse::SPREAD_FLOOR` | 0.08 | 0.014 |
| `fuse::SPREAD_FULL` | 0.14 | 0.040 |
| `fuse::AMBIGUOUS_CONFIDENCE_FLOOR` | 0.25 | 0.12 |
| `fuse::MODE_AMBIGUITY_THRESHOLD` | 0.6 | 0.80 |
| the evidence term's input | the chunk aggregate's spread | the whole-track PCP's spread |
| `chunks::CHUNK_WEIGHT_FLOOR` / `_FULL` | *(were `fuse`'s)* 0.08 / 0.14 | 0.08 / 0.14, now named separately |
| `energy::MU`, `SIGMA`, `QUANTILES` | set from each feature's definition | fitted on 176 full tracks |
| `energy::WEIGHTS` | not fitted | not fitted |
| `ANALYSIS_VERSION` | 1 | 2 |
| `ENERGY_MODEL_VERSION` | 1 | 2 |

The split of the old `fuse` gates into a detection gate (`chunks`) and a
confidence gate (`fuse`) is the load-bearing part. `chunks::aggregate` decides
**which chunks vote and therefore which key wins**; it kept the synthetic
constants to the last digit. `fuse::evidence_from_spread` only decides **how
loudly the answer is stated**; that is what was refit. `fuse::to_result` picks
the winner from the fused distribution before `evidence` is applied anywhere, so
the key cannot move when only the confidence gates move.

That is a prediction, and it was checked rather than asserted. Across the 1,056
tracks both runs completed:

| field | rows differing |
|---|---|
| key (`root`, `mode`) | **0 of 1,056** |
| Camelot label | **0 of 1,056** |
| displayed BPM | **0 of 1,056** |
| raw BPM | **0 of 1,056** |
| measured tuning (cents) | **0 of 1,056** |
| chunk count | **0 of 1,056** |
| tempo confidence | **0 of 1,056** |
| key confidence | 1,041 of 1,056 — expected |
| `mode_ambiguous` | 604 of 1,056 — expected |
| runner-up | 630 of 1,056 — expected |
| energy | 964 of 1,056 — expected |

And pass A, which is the shipped scorer over the shipped mirror, is
**byte-identical between the two runs on all six groups**: a `diff` of the two
harness reports, excluding the wall-clock line, is empty for `club`, `capstone`,
`readyfile`, `misc`, `vocals` and `acapella`.

---

## 3. Key agreement with Mixed In Key — scored by `mirex.rs`

**The whole of this section is unchanged, to the last decimal, and that is the
result.** A confidence recalibration must not change which key is detected; it
did not. Nothing below is a before-and-after column, because there is nothing to
put in the second column.

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
   59.43% exact; isolated acapellas at 40.35 / 29.29%. A thin monophonic vocal
   gives the chroma stage almost nothing to correlate against. Quoting the
   all-corpus 50.67 as if it described club performance would be meaningless,
   which is why the three difficulties are never pooled into a headline.
2. **There is no minor bias.** Keystone's estimates are consistently *less*
   minor than Mixed In Key's labels (0.788 vs 0.894 overall, −0.106). The `edmm`
   failure mode — post a good weighted score by always answering minor — is not
   what is happening here. If anything Keystone over-calls major. §4.4 adds a
   wrinkle: the set the new floor *refuses* is 97.0% minor-labelled and answered
   93.6% minor, so inside that sub-population the bias does exist.
3. **Mode is in better shape than the weighted total suggests.** Mode accuracy
   is 85.43% on full tracks against 59.43% exact, so most of the disagreement is
   *which* key, not major-versus-minor. §6 says why.

---

## 4. How often Keystone refused to answer — the number this work exists to fix

A refusal is a different outcome from a wrong answer, and the product treats it
as such: below `AMBIGUOUS_CONFIDENCE_FLOOR` there is no key badge at all; above
it, `mode_ambiguous` decides whether the badge is a single Camelot code or the
relative pair. Three states, so all three are counted. n = 1,056 (pass B).

### 4.1 The three badge states, before and after

`refused` = no badge. `pair` = a badge, shown as `8A/8B` rather than a letter.
`letter` = a single Camelot code. The p95 column is linear-interpolated and so
runs up to 0.02 below the nearest-rank p95 quoted in the 2026-10-31 edition of
this document; both tables below use the same definition.

**2026-10-31 — `AMBIGUOUS_CONFIDENCE_FLOOR` 0.25, `MODE_AMBIGUITY_THRESHOLD` 0.6**

| group | n | refused | pair | letter | mean conf | median | p95 |
|---|---|---|---|---|---|---|---|
| `club` | 50 | 47 (94.0%) | 0 (0.0%) | 3 (6.0%) | 0.062 | 0.000 | 0.262 |
| `capstone` | 42 | 37 (88.1%) | 1 (2.4%) | 4 (9.5%) | 0.063 | 0.000 | 0.269 |
| `readyfile` | 228 | 200 (87.7%) | 1 (0.4%) | 27 (11.8%) | 0.078 | 0.000 | 0.320 |
| `misc` | 30 | 24 (80.0%) | 1 (3.3%) | 5 (16.7%) | 0.134 | 0.085 | 0.456 |
| `vocals` | 315 | 282 (89.5%) | 1 (0.3%) | 32 (10.2%) | 0.065 | 0.000 | 0.339 |
| `acapella` | 391 | 343 (87.7%) | 3 (0.8%) | 45 (11.5%) | 0.077 | 0.000 | 0.435 |
| all full tracks | 350 | 308 (88.0%) | 3 (0.9%) | 39 (11.1%) | 0.079 | 0.000 | 0.320 |
| the three named | 756 | 672 (88.9%) | 4 (0.5%) | 80 (10.6%) | 0.071 | 0.000 | 0.393 |
| **everything** | **1,056** | **933 (88.4%)** | **7 (0.7%)** | **116 (11.0%)** | **0.074** | **0.000** | **0.390** |

**2026-11-01 — `AMBIGUOUS_CONFIDENCE_FLOOR` 0.12, `MODE_AMBIGUITY_THRESHOLD` 0.80**

| group | n | refused | pair | letter | mean conf | median | p95 |
|---|---|---|---|---|---|---|---|
| `club` | 50 | 5 (10.0%) | 6 (12.0%) | 39 (78.0%) | 0.431 | 0.435 | 0.806 |
| `capstone` | 42 | 4 (9.5%) | 5 (11.9%) | 33 (78.6%) | 0.435 | 0.440 | 0.809 |
| `readyfile` | 228 | 26 (11.4%) | 40 (17.5%) | 162 (71.1%) | 0.497 | 0.525 | 0.860 |
| `misc` | 30 | 4 (13.3%) | 15 (50.0%) | 11 (36.7%) | 0.408 | 0.395 | 0.803 |
| `vocals` | 315 | 93 (29.5%) | 52 (16.5%) | 170 (54.0%) | 0.309 | 0.270 | 0.769 |
| `acapella` | 391 | 102 (26.1%) | 63 (16.1%) | 226 (57.8%) | 0.339 | 0.300 | 0.815 |
| all full tracks | 350 | 39 (11.1%) | 66 (18.9%) | 245 (70.0%) | 0.472 | 0.480 | 0.860 |
| the three named | 756 | 200 (26.5%) | 121 (16.0%) | 435 (57.5%) | 0.333 | 0.295 | 0.810 |
| **everything** | **1,056** | **234 (22.2%)** | **181 (17.1%)** | **641 (60.7%)** | **0.374** | **0.350** | **0.830** |

**The defect is fixed.** Refusals on a real library fall from 88.4% to 22.2%,
and on the full tracks a DJ actually mixes from **88.0% to 11.1%**. On the
`club` folder — 50 current house records, the material this product is for — from
94.0% to 10.0%, and the share showing a single Camelot code rises from 6.0% to
78.0%.

The exact zeros, which were the symptom:

| set | n | conf = 0.000 before | after |
|---|---|---|---|
| `club` | 50 | 34 (68.0%) | 0 (0.0%) |
| `capstone` | 42 | 29 (69.0%) | 0 (0.0%) |
| `readyfile` | 228 | 127 (55.7%) | 5 (2.2%) |
| `misc` | 30 | 9 (30.0%) | 0 (0.0%) |
| `vocals` | 315 | 213 (67.6%) | 28 (8.9%) |
| `acapella` | 391 | 267 (68.3%) | 28 (7.2%) |
| all full tracks | 350 | 199 (56.9%) | 5 (1.4%) |
| **everything** | **1,056** | **679 (64.3%)** | **61 (5.8%)** |

### 4.2 Agreement inside each confidence band

The question §4.1 cannot answer: a lower floor trivially produces fewer
refusals. Does the confidence under it mean anything? The bands are the same in
both tables so the two engines can be read against each other; the `badge`
column says what that engine's own floor did with the band.

**Full tracks (n = 350).**

| band | before: n | weighted | exact | badge | after: n | weighted | exact | badge |
|---|---|---|---|---|---|---|---|---|
| [0.00, 0.05) | 224 | 67.41 | 54.91% | refused | 14 | 42.86 | 14.29% | refused |
| [0.05, 0.10) | 27 | 68.89 | 55.56% | refused | 17 | 48.82 | 35.29% | refused |
| [0.10, 0.12) | 13 | 50.00 | 38.46% | refused | 8 | 43.75 | 37.50% | refused |
| [0.12, 0.15) | 19 | 92.11 | 89.47% | refused | 13 | 64.62 | 46.15% | shown |
| [0.15, 0.25) | 25 | 70.80 | 52.00% | refused | 31 | 68.39 | 58.06% | shown |
| [0.25, 0.40) | 29 | 83.45 | 75.86% | shown | 49 | 59.59 | 46.94% | shown |
| [0.40, 1.00] | 13 | 100.00 | 100.00% | shown | 218 | **78.85** | **68.81%** | shown |

**Everything (n = 1,056).**

| band | before: n | weighted | exact | after: n | weighted | exact |
|---|---|---|---|---|---|---|
| [0.00, 0.05) | 746 | 47.69 | 35.52% | 118 | 25.59 | 13.56% |
| [0.05, 0.10) | 72 | 54.44 | 41.67% | 75 | 35.20 | 26.67% |
| [0.10, 0.12) | 18 | 41.67 | 27.78% | 41 | 34.15 | 24.39% |
| [0.12, 0.15) | 35 | 71.71 | 62.86% | 41 | 33.17 | 21.95% |
| [0.15, 0.25) | 62 | 53.06 | 38.71% | 115 | 46.00 | 33.04% |
| [0.25, 0.40) | 73 | 63.97 | 54.79% | 188 | 47.50 | 35.64% |
| [0.40, 1.00] | 50 | 59.80 | 52.00% | 478 | **65.00** | **52.72%** |

Before, the bands were not ordered: the all-corpus top band (59.80) scored
*below* the [0.12, 0.15) band (71.71), and on full tracks the lowest band of all
(67.41) beat three of the bands above it. After, both tables rise with
confidence from bottom to top with one dip each, and the top band is the best
band in both. The 2026-10-31 reading that "the ordering carries real information
and only the mapping onto 0–1 is wrong" was too kind to the old model; the
ordering was weak as well, and §4.3 puts a number on how weak.

### 4.3 One number for it: AUC

AUC is the probability that a randomly chosen track Keystone got right carries a
higher confidence than a randomly chosen track it got wrong. **0.50 means the
confidence carries no information at all.** `mixable` counts `correct`, `fifth`
and `relative` as hits, which is the wheel's own rule for whether mixing on the
badge works. The bracketed pair is (hits / misses).

| set | target | before | after |
|---|---|---|---|
| `club` | exact | **0.459** (28/22) | 0.654 |
| `club` | mixable | 0.517 (37/13) | 0.690 |
| `capstone` | exact | 0.492 (23/19) | 0.606 |
| `capstone` | mixable | 0.591 (32/10) | 0.686 |
| `readyfile` | exact | 0.600 (143/85) | 0.699 |
| `readyfile` | mixable | 0.589 (193/35) | 0.707 |
| `misc` | exact | 0.824 (14/16) | 0.708 |
| `misc` | mixable | 0.658 (23/7) | **0.435** |
| `vocals` | exact | 0.495 (89/226) | 0.633 |
| `acapella` | exact | 0.560 (115/276) | 0.656 |
| all full tracks | exact | 0.574 (208/142) | **0.689** |
| all full tracks | mixable | 0.582 (285/65) | **0.680** |
| vocals + acapella | exact | 0.532 (204/502) | 0.646 |
| everything | exact | 0.556 (412/644) | **0.689** |
| everything | mixable | 0.544 (649/407) | **0.691** |

On the `club` folder the old confidence was **worse than a coin flip** at
predicting exact agreement (0.459). It is now 0.654. Every group improves except
`misc`, where n = 30 and the mixable split is 23/7 — that row's interval is wide
enough to contain almost anything, and it is reported because it is the one row
that went the wrong way, not because it means something.

0.689 is a real signal and a modest one. It is not the 0.85 a confidence that
could be trusted per-track would show.

### 4.4 What a user actually meets

The product question is not about bands. It is: of the badges the UI shows, how
many are usable, and how many will clash in a mix? A `fifth` or a `relative` is
a badge you can mix on; `parallel` and `other` are not, so they are counted here
as **harmful**.

| run | set | state | n | weighted | exact | mixable | harmful |
|---|---|---|---|---|---|---|---|
| before | full tracks | shown | 42 | 88.57 | 83.33% | 92.9% | 3 (7.1%) |
| before | full tracks | refused | 308 | 68.60 | 56.17% | 79.9% | — |
| before | everything | shown | 123 | 62.28 | 53.66% | 69.9% | 37 (30.1%) |
| before | everything | refused | 933 | 49.36 | 37.08% | 60.3% | — |
| after | full tracks | shown | 311 | 74.18 | 63.34% | **83.6%** | 51 (16.4%) |
| after | full tracks | — of which letter | 245 | 77.06 | 67.35% | 83.7% | 40 (16.3%) |
| after | full tracks | — of which pair | 66 | 63.48 | 48.48% | 83.3% | 11 (16.7%) |
| after | full tracks | refused | 39 | 45.64 | 28.21% | **64.1%** | — |
| after | everything | shown | 822 | 56.75 | 44.53% | 67.5% | 267 (32.5%) |
| after | everything | refused | 234 | 30.17 | 19.66% | **40.2%** | — |

Two readings, and both have to be stated.

**The shown set is now clearly better than the refused set, which is the thing
that was broken.** On full tracks: 83.6% mixable shown against 64.1% refused,
63.3% exact against 28.2%. Over the whole corpus: 67.5% against 40.2%. Before,
on the `club` folder specifically, the three shown badges were *worse* than the
47 refused tracks (56.67 weighted against 67.23) — the marker was pointing the
wrong way. It is now 70.67 against 30.00.

**And the badge is less reliable per badge than it was.** Harmful errors among
shown full-track badges rise from 7.1% to 16.4%. That is not a regression in
detection — nothing in detection moved — it is what happens when a self-selected
set of 42 tracks is replaced by 311. In absolute terms: across the 350 full
tracks there are 65 harmful answers available. The old engine showed 3 of them,
and hid the other 62 behind a refusal that also hid 173 correct answers. The new
engine shows 51 harmful badges and 197 exactly-correct ones, and 260 badges you
can mix on against the old engine's 39. **The trade is 221 more usable badges
for 48 more harmful ones, about four and a half to one.**

One more honest wrinkle. The refused set's *mode* accuracy is high — 93.2% over
everything, against 78.8% for the shown set — which looks like the floor
refusing the wrong tracks. It is not skill. That set is **97.0% minor-labelled**
and Keystone answers minor on **93.6%** of it, so its mode accuracy is the
minor-bias artefact §1 warns about, measured inside a sub-population rather than
over the corpus. Exact agreement in the same set is 19.7%.

### 4.5 The held-out version, which is the one to believe

Everything above is measured on a corpus that contains the 532 files the four
`fuse` constants were chosen on. The confidence fit reported its own held half
separately, at AUC 0.69 pooled against 0.51 for the gated-spread input it
replaced. This re-measurement does not re-split and so cannot independently
confirm that figure. What it can and does confirm is the part that needs no
split: the key estimates are bit-identical (§2.3), pass A is byte-identical
(§3), and the refusal rates and band ordering moved as described over the whole
corpus.

**So: treat §4.1's refusal rates as solid.** A refusal rate is a property of the
floor and the confidence distribution, and a floor chosen to halve the refusals
on half a corpus will halve them on the other half too; there is very little room
for a split to flatter that. **Treat §4.3's AUC as optimistic by an unmeasured
amount.** The honest statement is that AUC on the fit-containing corpus is 0.689
and the fit's own held-out figure was 0.69; those agree, but they are not
independent, and nobody has yet measured AUC on files no constant has seen. That
is the one thing this re-measurement could not do and the next run should.

---

## 5. Tempo agreement with the Mixed In Key tempo in the filenames

**Unchanged in every cell**, as it must be: the displayed BPM differs on 0 of
1,056 tracks between the two runs. Against the engine's displayed BPM, folded
into the default 79–192 range. n = 1,056.

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

## 6. The disagreements, and what the refit made visible

### 6.1 The structure of the error is unchanged

Over full-track answers that got the letter right (n = 299), **identical in both
runs**:

| wheel offset | count | share |
|---|---|---|
| **+0 (agrees)** | 208 | 69.6% |
| **+1 (dominant, +7 st)** | 65 | 21.7% |
| −1 (subdominant, +5 st) | 15 | 5.0% |
| everything else | 11 | 3.7% |

The dominant is **4.3× more common than the subdominant**, so this is a
directional bias and not symmetric confusion. The three measurements that pin it
down are unchanged from 2026-10-31: rotating every estimate by −7 semitones
drops full tracks from 71.00 weighted to 19.90, so it is a per-track tie-break
failure and not a constant rotation in the profile table or the Camelot mapping;
the runner-up usually holds the right answer; and top-2 therefore beats top-1 by
a wide margin.

### 6.2 The runner-up is now almost always offered, and top-2 jumps

`to_result` withholds the runner-up when `evidence` is 0, because offering a
second-best reading for a drum loop implies one existed. Under the old synthetic
gates that fired on most of a real library, so the ambiguity UI had nothing to
display on two thirds of it.

| group | has a runner-up | top-1 exact | top-2 exact |
|---|---|---|---|
| `club` | 32.0% → **100.0%** | 56.00% → 56.00% | 66.00% → **82.00%** |
| `capstone` | 31.0% → **100.0%** | 54.76% → 54.76% | 66.67% → **85.71%** |
| `readyfile` | 44.7% → 97.8% | 62.72% → 62.72% | 71.05% → **84.21%** |
| `misc` | 70.0% → 100.0% | 46.67% → 46.67% | 60.00% → 66.67% |
| `vocals` | 32.7% → 92.1% | 28.25% → 28.25% | 37.14% → 49.84% |
| `acapella` | 32.0% → 94.1% | 29.41% → 29.41% | 36.06% → 47.31% |
| all full tracks | 43.4% → **98.6%** | 59.43% → 59.43% | 68.86% → **82.57%** |
| everything | 36.0% → 95.0% | 39.02% → 39.02% | 47.25% → 59.75% |

And the 2026-10-31 claim that the runner-up rescues the dominant error three
times in four was made on 24 tracks. It now has 61 and it holds: among
full-track +7 errors that carry a runner-up, the runner-up is Mixed In Key's
answer in **44 of 61 (72.1%)**, against 18 of 24 (75.0%) before. On acapellas
the rescue rate rises from 30.4% (n = 23) to 38.8% (n = 67), so the earlier low
figure there was partly a small sample.

This is the first evidence that the relative-pair and runner-up display earns
its place on real material rather than merely sounding honest.

### 6.3 The ten most confident disagreements — and the cost of the refit

Full tracks only, ranked by the engine's own confidence, one row per distinct
track. `amb` is `mode_ambiguous`; `0` means the UI shows a single Camelot letter.

| # | Mixed In Key | Keystone | category | offset | conf (was) | runner-up | amb | track |
|---|---|---|---|---|---|---|---|---|
| 1 | 2A (D# minor) | 1A (G# minor) | other | +5 st | **0.93** (0.00) | **2A = MIK** | 0 | MONTERO (Call Me By Your Name) |
| 2 | 1A (G# minor) | 2A (D# minor) | fifth | +7 st | **0.83** (0.01) | **1A = MIK** | 0 | BOP |
| 3 | 12A (C# minor) | 1A (G# minor) | fifth | +7 st | **0.83** (0.00) | **12A = MIK** | 0 | God Is A Dancer (with Mabel) |
| 4 | 12A (C# minor) | 1A (G# minor) | fifth | +7 st | 0.82 (0.00) | 2A | 0 | Whoopty |
| 5 | 4A (F minor) | 4B (G# major) | relative | +3 st | 0.80 (0.08) | **4A = MIK** | 0 | Body |
| 6 | 4A (F minor) | 5A (C minor) | fifth | +7 st | 0.79 (0.00) | **4A = MIK** | 0 | Clap Your Hands |
| 7 | 11A (F# minor) | 12A (C# minor) | fifth | +7 st | 0.77 (0.00) | **11A = MIK** | 0 | Ed Sheeran — Shape Of You (transition edit) |
| 8 | 11A (F# minor) | 12A (C# minor) | fifth | +7 st | 0.77 (0.00) | **11A = MIK** | 0 | kizaru — dezhavyu |
| 9 | 2A (D# minor) | 3A (A# minor) | fifth | +7 st | 0.73 (0.06) | 6B | 0 | Write This Down |
| 10 | 8A (A minor) | 11B (A major) | parallel | +0 st | 0.72 (0.33) | 12B | 0 | Spiller — Groovejet |

**Seven of the ten are the dominant, and the runner-up is Mixed In Key's own
answer in seven of the ten** — the same pattern as 2026-10-31 and the same
pattern as the whole-corpus table in §6.1. The change is the confidence column.
**Nine of these ten read 0.00–0.08 on 2026-10-31 and were refused; they now read
0.72–0.93 and are displayed as a single confident Camelot letter.**

That is the real cost of this refit, stated plainly: it converts the dominant
tie-break bug from a silent failure into a loud one. Row 5 is the sharpest case,
a relative error (F minor read as G# major) at confidence 0.80 with
`mode_ambiguous` clear — precisely the case `MODE_AMBIGUITY_THRESHOLD` exists to
catch, and it did not.

§4.4 says the trade is worth it at about four and a half to one, and §6.4 says the gate does
work in aggregate. Both of those are true at the same time as this table.

### 6.4 The mode-ambiguity gate at 0.80 does discriminate

`MODE_AMBIGUITY_THRESHOLD` went from 0.6 to 0.80, which makes *more* of the
newly visible tracks show a pair instead of a letter: 0.9% of full tracks before,
18.9% after. The gate's job is to catch relative confusions, so:

| run | set | letter n | mode acc | relative errors | pair n | mode acc | relative errors |
|---|---|---|---|---|---|---|---|
| before | full tracks | 39 | 92.3% | 0.0% | 3 | 100.0% | 0.0% |
| before | everything | 116 | 79.3% | 2.6% | 7 | 71.4% | 0.0% |
| after | full tracks | 245 | 89.4% | **0.4%** | 66 | 69.7% | **13.6%** |
| after | everything | 641 | 82.8% | **1.4%** | 181 | 64.6% | **11.0%** |

On full tracks the letter-shown set has a 0.4% relative-error rate and the
pair-shown set 13.6% — a 34-fold concentration. The gate is sorting the relative
confusions into the display state built for them. Before, it had three tracks to
work with and said nothing.

### 6.5 Where the work is

In order of how much it is worth against how hard it looks. Item 1 of the
2026-10-31 list is done and is struck from this one.

1. **Break the dominant tie in S4.** Now the top item, and now user-visible: a
   fifth of full-track answers are one clockwise step out, the right answer is
   already in second place three times out of four, and §6.3 shows those errors
   arriving at confidence 0.93 with a single letter on them. The template
   profiles correlate nearly as well with the dominant as with the tonic — which
   is what `fuse::separation`'s own comment describes — so the fix lives in
   profile shape or in a tonic-weight prior, not in the fusion. This is the
   largest single accuracy gain visible in the data and it needs no model.
2. **Teach the front end to recognise a flat chroma.** The refit's one known
   false positive: a uniform twelve-note wash now gets a confident single letter.
   Measured through the engine at six durations — whole-track spread
   0.0254–0.0261, confidence **0.054 at 10 s, 0.039 at 16 s, then 0.192 / 0.213 /
   0.217 / 0.253 at 20 / 30 / 60 / 120 s, shown with `mode_ambiguous` clear from
   20 s up.** Synthetic percussion is still a hard 0.000 at every duration from
   8 s to 120 s (spread 0.0117–0.0130, below `SPREAD_FLOOR`), so the drum-loop
   promise holds; it is the *full* chroma that is now indistinguishable from
   music, because `template::match_chroma` standardises its similarities and a
   near-flat input still yields a peaked distribution. `SPREAD_FLOOR` cannot fix
   it: the wash sits at the real corpus's own 20th percentile. See caveat 13.
3. **Ship the ambiguity UI; the data now supports it twice over.** Top-2 beats
   top-1 by 23 points on full tracks (82.57% against 59.43%), the second answer
   is the competitor's in 72% of dominant errors, a runner-up is available on
   98.6% of full tracks rather than 43.4%, and §6.4 shows the pair state
   concentrating relative errors 34-fold.
4. **Redefine `high_ratio`, and look at S8's downbeat phase.** §7.3 is the
   evidence: the energy ladder now disagrees by 2 or more on 8 of 42 pairs of
   separately sourced copies of the same recording, worst case 7. The energy
   calibration's own perturbation sweep found `high_ratio`, `centroid` and
   `low_ratio` to be the three least repeatable features, and a downbeat-phase
   flip upstream in S8 explains one failing case. ENGINE.md risk 6.
5. **Decide what an acapella should do.** 40.35 weighted, five files with no beat
   grid at all, and the refit has made this *more* pressing rather than less:
   289 of 391 acapellas (73.9%) now get a badge and 120 of those (41.5%) are
   harmful. A
   thin monophonic vocal may simply not have a key the 24-class taxonomy can
   carry, and the honest output is a refusal — but it has to be a refusal the UI
   explains. ENGINE.md risk 10.
6. **Leave tempo alone.** 88.29% integer-exact on full tracks with a 0.01 BPM
   median error is not where the remaining error is.

---

## 7. Energy

ACCURACY.md did not measure energy before this run. The histograms below were
extracted from the same 2026-10-31 per-track reports as everything else in the
`before` column, so this is a genuine before-and-after on identical files.

### 7.1 The histogram, 1 through 10

| set | model | n | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | mean | median | distinct |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `club` | 1 | 50 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 5 | 12 | 32 | 9.50 | 10 | 4 |
| `club` | 2 | 50 | 1 | 2 | 2 | 6 | 3 | 2 | 7 | 9 | 5 | 13 | 7.16 | 8 | 10 |
| `capstone` | 1 | 42 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 7 | 33 | 9.74 | 10 | 3 |
| `capstone` | 2 | 42 | 0 | 0 | 0 | 5 | 4 | 2 | 6 | 8 | 7 | 10 | 7.64 | 8 | 7 |
| `readyfile` | 1 | 228 | 0 | 0 | 0 | 0 | 4 | 4 | 12 | 38 | 97 | 73 | 8.93 | 9 | 6 |
| `readyfile` | 2 | 228 | 26 | 25 | 15 | 32 | 22 | 32 | 27 | 20 | 18 | 11 | 5.14 | 5 | 10 |
| `misc` | 1 | 30 | 0 | 1 | 0 | 1 | 0 | 1 | 5 | 7 | 13 | 2 | 8.00 | 9 | 7 |
| `misc` | 2 | 30 | 8 | 6 | 5 | 6 | 1 | 2 | 0 | 1 | 0 | 1 | 3.13 | 3 | 8 |
| `vocals` | 1 | 315 | 1 | 5 | 11 | 13 | 25 | 58 | 81 | 76 | 42 | 3 | 6.83 | 7 | 10 |
| `vocals` | 2 | 315 | 93 | 44 | 23 | 38 | 18 | 25 | 26 | 28 | 16 | 4 | 3.91 | 3 | 10 |
| `acapella` | 1 | 391 | 0 | 15 | 37 | 58 | 64 | 76 | 74 | 47 | 19 | 1 | 5.69 | 6 | 9 |
| `acapella` | 2 | 391 | 193 | 49 | 39 | 43 | 16 | 24 | 9 | 13 | 4 | 1 | 2.60 | 2 | 10 |
| **all full tracks** | 1 | 350 | 0 | 1 | 0 | 1 | 4 | 5 | 18 | 52 | 129 | 140 | 9.03 | 9 | 8 |
| **all full tracks** | 2 | 350 | 35 | 33 | 22 | 49 | 30 | 38 | 40 | 38 | 30 | 35 | 5.56 | 6 | 10 |
| everything | 1 | 1,056 | 1 | 21 | 48 | 72 | 93 | 139 | 173 | 175 | 190 | 144 | 7.14 | 7 | 10 |
| everything | 2 | 1,056 | 321 | 126 | 84 | 130 | 64 | 87 | 75 | 79 | 50 | 40 | 3.97 | 3 | 10 |

Model 1 put **321 of 350 full tracks (91.7%) on 8, 9 or 10**, and the `club` and
`capstone` folders used four and three of the ten values. Model 2 spreads full
tracks across all ten. `club` keeps a top-heavy shape (13 at 10, mean 7.16),
which is what a curated peak-time crate should look like against a library-wide
median of 5.

Two consequences in the open. `misc` collapses to the bottom (mean 3.13, 8 of 30
at energy 1): roughly a quarter of its files are stems or vocal parts by name.
And **49.4% of isolated acapellas now read 1**. That follows directly from the
fit excluding stems, which `energy::constants` states and argues for; it is
consistent with the stated anchor — a 5 is the median full track in a house and
pop DJ library — and it means the number is uninformative on a sample pack.

### 7.2 The held-out result, reproduced independently

The energy fit used 176 of the 350 full tracks and reported its held half. That
claim is checkable from this re-measurement without trusting the fitting
harness: take the pre-registered split file, intersect it with the full-track
groups, and read the energies out of the before and after runs.

| half and model | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | mean | median | distinct |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| held, model 1 | 0 | 1 | 0 | 1 | 0 | 1 | 10 | 25 | 64 | 72 | 9.07 | 9 | 7 |
| held, model 2 | 17 | 15 | 5 | 32 | 12 | 20 | 23 | 20 | 13 | 17 | 5.62 | 6 | 10 |
| fit, model 1 | 0 | 0 | 0 | 0 | 4 | 4 | 8 | 27 | 65 | 68 | 8.98 | 9 | 6 |
| fit, model 2 | 18 | 18 | 17 | 17 | 18 | 18 | 17 | 18 | 17 | 18 | 5.49 | 6 | 10 |

**Every cell of the held-out row reproduces the reported table exactly**,
including the per-group breakdown, and so does the fit half's near-perfect
uniformity — which is what fitting deciles on a set and then scoring that same
set must produce, and is the clearest available illustration of why the held row
is the only one worth quoting. 161 of the 174 held-out full tracks read 8, 9 or
10 under model 1; 50 do under model 2.

Spearman between model 1's `raw` and model 2's, measured over this whole run
rather than the held half: **0.958 on full tracks** (the fit reported 0.957 on
its held half), 0.887 on stems, 0.876 overall. **This is a restretch, not a
reordering.** Model 1 already ranked the library and threw the ranking away in
the mapping onto 1–10. Fitting a ladder guarantees *spread*; it cannot prove the
*order* is right, and no measurement in this document or anywhere else in the
repository does. That still needs ENGINE.md §7.3 test 4, the 300 forced-choice
human pairs, which do not exist.

### 7.3 The duplicate-copy test, which model 2 fails worse than model 1

42 recordings in this corpus exist as two separate files — an AIFF in `capstone`
and an MP3 in `club`, or the same download filed twice. Their durations are
identical to 0.0 s in all 42 cases. That makes them a free, real-audio version of
ENGINE.md §7.3 **test 3**, the edit-pair test, and it was not run by either
calibration.

| energy spread across copies | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| model 1 | 30 | 10 | 1 | 1 | 0 | 0 | 0 | 0 |
| model 2 | 18 | 16 | 4 | 2 | 1 | 0 | 0 | 1 |

Within ±1 on **40 of 42** under model 1 and **34 of 42** under model 2. The worst
case is a 7: David Guetta / Marten Hørger / Men Machine, "The Past, The Present,
The Future", which reads **8** as the AIFF and **1** as the MP3 (`raw` 0.590
against 0.189). The AIFF copy reads higher than the MP3 copy in all twelve of
the worst-spread pairs.

Three things have to be said about this, and none of them cancels the others.

- **It is partly the same artefact that flatters model 1 everywhere else.** The
  raw difference on that worst pair was already 0.195 under model 1 (0.741
  against 0.546) and was invisible only because both ended up near the top of a
  compressed ladder. The energy calibration made exactly this argument about its
  own invariance pass — an invariance test passes trivially on a constant
  function — and the argument applies here too.
- **And the gap really did widen.** 0.195 became 0.400. Model 2's σ is tighter
  than model 1's on five of the seven features, so a given feature difference now
  moves `raw` further. The refit amplified a sensitivity that was already there.
- **These are not transcodes, so this is not strictly an invariance failure.**
  Two separately sourced files of one recording can be genuinely different
  masters with different EQ and loudness treatment, and the corpus cannot tell
  that apart from a feature defect. What it does establish is that **on real
  library material, model 2's 1–10 reading is not reproducible to ±1 across
  copies of the same record**, which is the resolution the scale implicitly
  claims. Combined with the perturbation result — ±6 dB gain and 44.1↔48 kHz
  resample drift exactly 0, but a 1 dB wide-Q EQ move can shift a step — the
  diagnosis points at the three spectral-balance features and not at the
  calibration.

Key, tempo and tuning are unaffected: 42 of 42 identities return the same key in
both runs, with a median tempo spread of 0.00 BPM and a maximum of 0.40.

### 7.4 What the version bump costs an existing library

`ENERGY_MODEL_VERSION` 1 → 2 and `ANALYSIS_VERSION` 1 → 2. Every stored energy
is incomparable with a new one, including numbers already written into users'
Grouping tags and filenames by `tags/retag.rs`: re-analysis plus a re-tag pass is
needed for the column and the tags to agree. The 964 of 1,056 energies that
changed in this run are the measure of it. A cheaper path exists and is unused:
`version::energy_current` plus the cached features in the payload would allow
re-scoring energy without re-running the DSP; nothing calls it.

---

## 8. Is the measurement itself stable?

42 track identities in the corpus exist as two or more separate files — usually
an MP3 and an AIFF of the same recording, in different groups.

- Keystone returned **the same key on every copy: 42 of 42 (100%)**, in both
  runs.
- Tempo across copies: median spread 0.00 BPM, maximum 0.40 BPM, nothing above
  1 BPM, in both runs.
- Mixed In Key's own labels also agreed across all 42.
- Key confidence across copies: median spread 0.000 (max 0.140) before, 0.010
  (max 0.290) after. The scale is wider, so its spread is wider.
- Energy across copies: §7.3, which is the one place the engine got less stable.

So key detection and tempo are deterministic and codec-robust across these
pairs, and the disagreements in §3 and §6 are genuine method differences rather
than run noise.

Tuning, measured on 1,056 real files and **bit-identical between the two runs**:
median **−2.5 cents**, quartiles −6.8 / +1.2, range −33.4 to +33.1, and **189
files (17.9%) beyond ±20 cents**. S2 is doing real work on roughly one file in
six, which is worth knowing given that both Faraldo and Sha'ath measured no
aggregate gain from tuning handling on digitally-produced EDM. This corpus is
not all digitally-produced EDM.

The build this was measured on: `cargo check --features eval --all-targets`
clean, `cargo test --features eval` **451 passed, 0 failed, 1 ignored**, `npm run
build` clean. The 2026-10-31 run was made at 450 tests; the extra one is
`fuse_tests::the_fitted_gates_still_bracket_what_they_were_fitted_to`. No test
was deleted by either calibration: four cases moved out of
`analysis/key/signal_tests.rs`, which was at the 300-line module cap, into a new
`analysis/key/refusal_tests.rs`, and two assertions inside them were weakened
with their reasons stated in the file. Caveat 13 is about one of those.

---

## 9. Caveats

The eleven from the 2026-10-31 edition, plus the two consequences stated in §0,
are all still true and are kept below with their numbering. Caveats 12 and 13
are new, and both exist because of this calibration.

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
   These figures do not generalise past material like this. **As of 2026-11-01
   this is no longer only a caveat about the figures — see caveat 12.**
7. **Groups are not independent samples.** `club` has n = 50 and `misc` n = 30;
   those two rows carry wide intervals and small differences between them mean
   nothing. No confidence intervals are computed anywhere in this document, so
   no difference of a few points should be read as real. The `misc` AUC in §4.3,
   the only figure that moved the wrong way, is an n = 30 row with a 23/7 split
   and is the clearest example.
8. **The three vocal and acapella groups have a second labelling problem.**
   Mixed In Key derived those labels from the same thin monophonic audio
   Keystone is struggling with, so the references themselves are least reliable
   exactly where agreement is lowest. The low acapella figure is evidence about
   the material, not about either program.
9. **This is the template voter alone.** There is no bundled S-KEY graph in the
   tree (`SkeySession::from_path` is still `todo!()`), so the ensemble — named in
   ENGINE.md as the cheapest real accuracy gain available — is unmeasured. Read
   §3 as a floor for the engine's eventual configuration, not as its score. It
   also means the confidence model was fitted without it: `fuse`'s own comment
   says the evidence term has to be refitted over both voters once the model
   voter lands, and that will invalidate the constants in §2.3.
10. **The tempo reference is an integer.** The filenames carry whole-number BPM,
    so Accuracy1's ±4% window is generous at 80 BPM and tight at 190, and
    integer-exact agreement cannot distinguish 128.00 from 128.49.
11. **One file could not be decoded and six were refused by later stages**
    (§2.2). Nothing was dropped silently: §3 is n = 1,062 and §4–§8 are
    n = 1,056, and both are stated wherever they appear. Those seven are the same
    seven files in both runs.
12. **NEW, and the most important one on this page: the engine now contains
    constants fitted to this corpus, so this document is no longer fully
    out-of-sample.** Four gates in `key::fuse` were fitted on 532 of these 1,063
    files; `energy::constants`' fourteen moments and eleven knots were fitted on
    176 of the 350 full tracks. The consequences, in order of how much they
    matter:
    - **§3, §5 and §6.1 are unaffected and remain out-of-sample.** No fitted
      constant can reach the key estimate or the tempo, and §2.3 proves it by
      measurement rather than by argument.
    - **§4's refusal rates and §7.1's histogram are measured partly on the files
      their constants were chosen on.** The held-out versions are §4.5 and §7.2.
      §7.2's fit-half row — a near-perfect 18/18/17/17/18/18/17/18/17/18 — is
      what reporting on the set you fitted on looks like, and is why the held
      row is the only one quoted elsewhere.
    - **Nobody has yet measured confidence AUC on files no constant has seen.**
      §4.3's 0.689 is on the fit-containing corpus. The fit's own held-out figure
      was 0.69 and they agree, but they are not independent.
    - **The constants are fitted to one person's library.** `energy::constants`
      states the anchor outright: a 5 is the median full track in a house and pop
      DJ library. A user whose library is techno, drum and bass, hip-hop,
      ambient, jazz or classical has no reason to expect either the energy
      ladder or the confidence floor to sit where it should, and no measurement
      exists either way. Caveat 6 used to be a limitation on the figures in this
      document; it is now a limitation on the shipped product.
    - Any change to the key profiles, to `template::SOFTMAX_TEMPERATURE`, to the
      chroma front end, to the seven energy features, to the HPSS reductions or
      to the core-region rule invalidates the fit. Re-run both passes; do not
      nudge a constant.
13. **NEW: the confidence refit created one known false positive, and the test
    that is supposed to cover it passes by accident of its fixture length.** A
    uniform twelve-note chromatic wash is shown a confident single-letter key
    from about 20 s of duration upward (§6.5 item 2). `refusal_tests::`
    `atonal_material_comes_back_ambiguous_too` asserts it is refused at exactly
    10.0 s, which is one of the two durations where it still is. The assertion it
    used to make — `evidence < 0.3` — was deliberately weakened to
    `evidence > 0.0` with the reason written into the file, and the reason is
    sound: the wash's whole-track spread (0.0254) sits at the real corpus's 20th
    percentile, so the gate genuinely cannot separate them and pretending
    otherwise is what broke the product. But the test as it stands does not
    measure the regression it documents, and it should be parameterised over
    duration so that the failure is visible in CI rather than only in prose.
    Pure percussion and silence are still hard zeros at every duration measured,
    8 s to 120 s, so the drum-loop promise is intact.

---

## 10. Reproducing this

```bash
cd src-tauri
cargo build --release --features eval --bin keystone-eval

# Pass A — key, through mirex.rs. <mirror>/<group>/ holds one
# "<name>.<ext>.key" per track (a line such as "D minor") beside
# <mirror>/<group>/audio/<name>.<ext>.
./target/release/keystone-eval <mirror>/<group>

# Pass B — the whole engine on one file, for confidence, tuning, tempo, energy.
./target/release/keystone-eval --track "<file>"

# The raw gate inputs, one TSV row per file, which is what the confidence
# constants were fitted from.
./target/release/keystone-eval --key-table 6 < paths.txt

# The seven energy features per file, which is what the energy ladder was
# fitted from.
./target/release/keystone-eval --energy-features paths.txt 6

# The scorer's own tests.
cargo test --features eval --lib eval::mirex
```

The corpus is rebuilt from the `find | grep` in §1, parsing
`<original> - <camelot> - <tempo>.<ext>` and inverting `Camelot::to_key`. Run it
again after any change to S2, S3, S4, `fuse`, `chunks`, the energy features or
the energy constants, and compare against these tables rather than against a
remembered headline figure.

**When this is re-run, check in this order.**

1. **Is §3 still identical?** A change to `fuse` or to the energy constants that
   moves a single cell of §3 has changed detection, and whoever made it did not
   mean to. §2.3's identity table is the cheap version of this check and it runs
   off pass B alone.
2. **The refusal rate in §4.1.** A weighted total that improves while the
   library shows no key badge has not improved anything a user can see. The
   converse now also applies: a refusal rate that falls while §4.4's
   shown-versus-refused gap closes has only made the engine louder, not better.
3. **§4.4's shown-versus-refused gap and §4.3's AUC.** These are the numbers
   that say whether the badge means anything. The bar is not fewer refusals.
4. **§7.3's duplicate-copy spread.** It is free, it is real audio, and it is
   currently failing.
