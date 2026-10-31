# Keystone — Project Context

Desktop harmonic-mixing analysis for DJ libraries. Offline, local, MIT. A DJ points
Keystone at their tracks and gets key, tempo, energy, eight phrase-aligned cue points
and a Camelot wheel that filters the library to what will actually mix — then writes
that back into their tags, filenames, and rekordbox / Traktor cue points.

Sibling to **Stack** (sample library manager) and **Capstone** (audio correction). The
three share the design language, the Tauri/React/Rust/SQLite shape, and a rotated
accent palette.

Two documents define the product and are the authority over this file:

- [`docs/SPEC.md`](docs/SPEC.md) — what the product is: every window, control and
  setting, and what is deliberately out of v1.
- [`docs/ENGINE.md`](docs/ENGINE.md) — how the analysis engine works, stage by stage,
  with the measurements and the licence boundaries behind each decision.

This file is the orientation layer. Where it summarises, those two win; where a number
or a threshold matters, it lives in them and in the code, not here.

## Stack

| Layer | Tech |
|---|---|
| Desktop shell | **Tauri 2** (`src-tauri/`) |
| UI | **React 18 + Vite + TypeScript (strict)** (`src/`) |
| Styling | **Tailwind CSS** + CSS vars for the design system |
| Icons | **iconsax-reactjs** 0.0.8 — *only* re-exported through `src/components/ui/icons.ts` |
| State | **Zustand** (client UI state) + **@tanstack/react-query** v5 (server state) |
| Backend lang | **Rust 2021** |
| DB | **SQLite** via rusqlite + r2d2 pool + WAL mode; migrations in `src-tauri/src/db/migrations/` |
| Decode | **symphonia** 0.5 (pinned — see below) + **rubato** for resampling |
| DSP | **rustfft** / **realfft** / **ndarray**, hand-rolled. No GPL audio library, ever |
| Loudness | **ebur128** for EBU R128 short-term loudness |
| Key model | **ort** (ONNX Runtime), pinned exactly, for the S-KEY voter |
| Tags | **lofty** 0.25 — concrete tag types only, never the generic `Tag` |
| Export XML | **quick-xml** streaming |
| File watching | **notify** + **notify-debouncer-full** (`FileIdMap`) |
| Hashing | **xxhash-rust** xxh3 |
| CPU stage | **rayon** pool behind our own bounded queue |
| Async runtime | **Tauri's managed Tokio** — `tauri::async_runtime::spawn`, NOT `tokio::spawn`, inside `setup` |

**symphonia is pinned to 0.5, not 0.6.** The proven streaming-decode call site is a 0.5
API and 0.6 is a declared breaking change (audio primitives redesigned,
`MediaSourceStream` lifetime-generic, track timing moved out of `codec_params`). The
port is its own scheduled task, in a decode module shared with Capstone, so it is paid
for once. symphonia is MPL-2.0: file-level copyleft, listed in `THIRD-PARTY-LICENSES.md`,
and **never patched in-tree**.

**`ort` is pinned with `=`, no caret.** It is a release candidate whose ABI is tied to
one ONNX Runtime build. On Windows the DLLs ship beside the binary, because a stale
system `onnxruntime.dll` shadows the bundled one.

## Dev commands

```bash
. "$HOME/.cargo/env"        # one-time per shell if cargo isn't on PATH
npm install                 # first time only
npm run tauri dev           # dev with HMR + Rust hot rebuild
npm run typecheck           # tsc --noEmit

cd src-tauri
cargo test                                      # the unit suite
cargo test --features eval                      # + the accuracy harness
cargo run --features eval --bin keystone-eval    # the harness report, in full
cargo bench --bench decode                      # the decode budget
```

The `eval` feature is dev-only and gated in `Cargo.toml` with `required-features`. It
reads a corpus from disk and must never reach a shipped bundle.

## Repository layout

```
src/                    React frontend
  App.tsx               Root layout; the active view comes from uiStore
  main.tsx              QueryClient + StrictMode entry
  index.css             Design tokens (dark + light), Tailwind layers
  components/
    ui/                 Primitives — Button, Badge, KeyBadge, Select, Toggle, icons.ts
    wheel/              CamelotWheel + geometry + segments — the app's signature object
    library/            TrackTable (virtualized), TrackRow, columns, AnalysisQueue
    player/             PlayerBar, WaveformView (canvas), CueMarkers, VirtualPiano
    tags/               The tag-editing view: TagTable, GlobalReplacePanel, editors
    collections/        Sidebar collections, with drag-to-collection
    settings/           One pane per SPEC.md §3 rail item
    menu/               The track context menu from SPEC.md §4
    layout/             Sidebar, TitleBar, MainPanel
  pages/                LibraryPage, TagsPage, SettingsPage
  hooks/                useLibrary, useAnalysis, useCues, useCollections, usePlayer,
                        useSettings, useKeyboard
  services/             keystoneService — the ONLY layer that calls `invoke()`
  stores/               libraryStore, playerStore, analysisStore, collectionStore,
                        settingsStore, uiStore (Zustand)
  types/                Shared TS types, re-exported from index.ts; ipc.ts holds the
                        event-name table
  utils/                camelot, camelotColors, formatters
  data/changelog.json   Release notes, read by CI to build the GitHub release body

src-tauri/              Rust backend
  Cargo.toml            Feature flags: `eval` (harness), `beat-this` (the gated model)
  tauri.conf.json       assetProtocol enabled, scope ["**"]; updater artifacts on
  src/
    main.rs             Bin stub; calls keystone_lib::run()
    lib.rs              Builder chain. Startup order: recover renames, then resume work
    handlers.rs         `keystone_invoke_handler!` — the one invoke table
    state.rs            AppState: pool, repos, indexer, queue, watcher, settings
    events.rs           Every backend -> frontend event name, declared once
    error.rs            KeystoneError + serde-serializable Result alias
    models/             IPC structs. ALL carry serde(rename_all = "camelCase")
    db/                 connection, migrations, track_repo, cue_repo, collection_repo,
                        analysis_repo, library_repo, query_builder, mapping
                        — ALL SQL lives here, nowhere else
    core/               Filesystem domain, no Tauri deps
      scanner.rs        Recursive walk, audio extensions only
      watcher.rs        Debounced notify wrapper
      hasher.rs         Two hashes with two jobs — see "Identity" below
      indexer.rs        The IO stage; exclude()/unexclude() choke points
      reconciler.rs     Startup diff: touch last_seen_at, mark missing
      renamer.rs        Journal-first renames
      peaks.rs          Streaming waveform peaks, cached on disk by content hash
    analysis/           The engine. S0..S12, see below
      spectral/         S2 + S3: stft, whitening, tuning, vqt, chroma
      key/              S4: template + skey voters, profiles, chunks, fuse
      camelot.rs        S5: key -> wheel position + the compatibility set
      rhythm/           S6 + S7 + S8: onset, tempogram, tempo, octave, grid, beats,
                        downbeat, phrase, arbiter (the gated Beat This! path)
      structure/        S9: strong-beat features, SSMs, boundaries, regions
      energy/           S10: features, loudness, frozen constants, score
      cues/             S11: filters, roles, assign, coverage
      version.rs        ANALYSIS_VERSION and ENERGY_MODEL_VERSION
    tags/               Read, write, rename, retag, backup, verify
    export/             safety/ first, then rekordbox/, traktor/, serato/ (v1.1)
    jobs/               The analysis queue, its writer, player reads, settings
    eval/               Dev-only accuracy harness (feature = "eval")
    commands/           Thin Tauri handlers, one file per domain
```

## Architectural rules (enforce in reviews)

- **Commands are thin.** No SQL, no DSP, no business logic inside `#[tauri::command]`.
  They validate input, delegate to `core/` / `analysis/` / `jobs/` / a repo, and return
  `Result`.
- **All SQL in `db/`.** No `conn.prepare(...)` outside it. New query, new repo method.
- **React components never call `invoke()` directly.** component -> hook -> service ->
  `invoke`. `services/keystoneService.ts` is the only file that imports it.
- **Zustand = client state, react-query = server state.** The one deliberate exception
  is `analysisStore`: queue progress arrives by push on a timer, so react-query has
  nothing to key it on. It is documented in the store itself.
- **Every IPC struct carries `#[serde(rename_all = "camelCase")]`,** and TypeScript is
  camelCase. Non-negotiable across the boundary.
- **Icons only from `components/ui/icons.ts`,** and every name is verified against the
  installed `iconsax-reactjs@0.0.8` before use. A missing name imports as `undefined`,
  React renders `<undefined />`, and the app white-screens — TypeScript will not catch
  it, because the package's types are permissive. The file's header lists the names
  that have already burned time (`Key2`, `Pen`, `Rotate`, `Undo`, `Note2`, `Music2`).
- **Long work in `setup` uses `tauri::async_runtime::spawn`.** `tokio::spawn` panics
  with "no reactor running": the runtime is not live yet. Inside a running async task,
  either is fine.
- **File size caps.** React components ≤ 200 lines, Rust modules ≤ 300 lines, functions
  ≤ 50 lines. Extract a sub-module rather than letting a file grow.
- **Comments say WHY.** Never restate the line below. Everything non-obvious in this
  codebase was decided against an alternative, and the comment's job is to name the
  alternative and why it lost.
- **Licence rule, and it drives half the engine design.** Keystone ships MIT, so only
  MIT / Apache-2.0 / BSD, plus MPL-2.0 (file-level). **No GPL, no AGPL, no
  unlicensed code.** libKeyFinder, Essentia, aubio, madmom's model files, `edmkey` and
  published `triseratops` releases are all excluded, and Mixxx is for reading, never
  porting. `docs/ENGINE.md` §4 has the full table. Nothing fitted on a CC BY-SA
  evaluation set may be compiled into the binary.
- **Nothing in the analysis path touches a network.** Including model downloads: if a
  model cannot be bundled, it does not ship.

## Identity: two hashes, two jobs

`core/hasher.rs`, and conflating them is a bug that looks like it works.

- **`hash_path`** → the track `id`. Per-path on purpose. Content-hash identity makes
  byte-identical files at different paths collapse into one row and ping-pong on every
  reconcile, and a DJ library is *more* duplicate-heavy than a sample library: the same
  track as 320k MP3 and as AIFF, promo copies, a folder per gig.
- **`hash_audio_region`** → the `content_hash` column, which keys the analysis cache.
  It hashes the **audio data region**, never the file bytes. ID3 sits at the head of an
  MP3, so hashing the first 256 KB of the *file* means Keystone's own re-tag pass
  changes every hash and silently wipes its own cache. It looks correct right up until
  the first re-tag.

That cache is the highest-value piece of the data model: a moved file, a file Keystone
renamed, and a duplicate all hit it with zero DSP. It is what makes re-scan nearly free
and renaming free instead of a re-analysis storm.

**Invalidation differs for tags and analysis.** Tag reads invalidate on **mtime**;
analysis invalidates on **`content_hash` + `ANALYSIS_VERSION`**. A tag edit changes
mtime but not audio and must not cost a re-analysis.

## The analysis pipeline, in order

One decode feeds every stage. Decode is roughly 90% of per-track cost, so the only
optimisation that matters at this level is not decoding twice. **Never hold a decoded
track longer than the stage that needs it** — a 10-minute 48 kHz stereo f32 buffer is
230 MB, and eight workers hold eight.

The order is **not rearrangeable**. `analysis/mod.rs` owns it.

| Stage | Does | Lives in |
|---|---|---|
| S0 | Walk, per-path id, pending row | `core/scanner.rs`, `core/indexer.rs` |
| S1 | Decode once → mono 44.1 k + 22.05 k, true peak, clipped count, streamed peaks | `analysis/decode.rs`, `core/peaks.rs` |
| S2 | **Tuning estimate** in cents, and the corrective resample | `analysis/spectral/tuning.rs` |
| S3 | 36-bin HPCP + per-strong-beat 12-bin chroma | `analysis/spectral/chroma.rs` |
| S4 | **Key** — two voters fused, confidence, runner-up, ambiguity flag | `analysis/key/` |
| S5 | Camelot number/letter + the wheel's compatibility set | `analysis/camelot.rs` |
| S6 | Onset novelty at 100 fps, plus kick / mid / hat bands | `analysis/rhythm/onset.rs` |
| S7 | Tempo, the octave decision, folding, the least-squares BPM refit | `analysis/rhythm/tempo.rs` |
| S8 | Beat grid, strong-beat grid, downbeat phase, 4-bar phrase period | `analysis/rhythm/beats.rs` |
| S9 | Seven strong-beat features, SSMs, boundaries, `track_regions` | `analysis/structure/` |
| S10 | **Energy 1–10** over the core region | `analysis/energy/` |
| S11 | Eight cue points, assigned by role | `analysis/cues/` |
| S12 | Persist through the single writer task, batched | `jobs/writer.rs` |

### Why tuning is estimated before chroma and key

S2 runs first and is **not optional**. The S-KEY model's front end is a VQT at 12
bins per octave — one bin per semitone, no sub-semitone resolution — and it was
measured collapsing to the wrong tonic *and* the wrong mode at ±50 cents (E♭ major
read as G minor), while staying correct at +20 c. Correcting the detune restored the
original prediction to three decimals. The template voter has the same exposure: a
detuned track smears energy across adjacent HPCP bins, and the profile match is then
decided by the smear.

Be honest about the size of the win. Both Faraldo and Sha'ath measured **no aggregate
gain** on digitally-produced EDM at A440 — Sha'ath disabled tuning handling in
KeyFinder by default for that reason. Tuning correction is cheap insurance against
catastrophic tail failures: vinyl rips, pitched edits, live recordings,
turntable-pitched hip-hop. For this audience, that tail *is* the audience.

Two fields, not one: `tuning_cents` is the measurement for the *file*, which the UI
shows, and the residual still present in the buffers is separate. Collapsing them
breaks one of the two — correcting against a residual of zero on an uncorrected buffer
leaves the detune in the chroma, and reporting the residual instead of the measurement
tells a user with a vinyl rip that their track is at A440.

### Why key is inferred per chunk

Whole-file inference was measurably worse than chunking at 15 s and averaging the
probability vectors: top-1 0.268 against 0.480 on the test track, with per-chunk
accuracy rising with window length. That accuracy figure is **n = 1** and is a
hypothesis for the eval harness, but the confidence lift is real, and three things fall
out of chunking for free:

- **Per-region key**, which is what the deferred deck view would read.
- **A spread across chunks**, which is a second, independent confidence signal: a track
  whose chunks all agree is not the same answer as one whose chunks are split.
- **Robustness to intros and outros**, which are a large fraction of a club edit and
  often harmonically unrepresentative of it.

Two voters are fused (`DEFAULT_WEIGHTS`, 0.6 model / 0.4 template) precisely because
they are unalike: a CNN over a VQT and a cosine match over a whitened HPCP fail on
*different* tracks, and uncorrelated error profiles are the cheapest real accuracy gain
available. The template-only path is a **supported configuration**, not a degraded one
— the harness has to be able to score each voter alone.

### Why confidence exists, and must be surfaced

Most residual key error is **mode**, which is exactly Camelot's A/B letter and the one
thing a DJ actually reads off the badge. So when mode confidence falls below
`MODE_AMBIGUITY_THRESHOLD`, Keystone sets `key_mode_ambiguous` and the UI shows the
relative pair (`8A/8B`) instead of committing to a letter.

**This is the product's differentiator and it does not depend on winning a benchmark.**
A DJ would rather see acknowledged ambiguity than a confident wrong letter, and the
incumbent does not offer it. Voter disagreement is also the signal behind SPEC.md's
`Improve Tracks` collection and `Display a message if the file can be improved`.

The same rule holds elsewhere. Material that is not broadly 4/4 and tempo-stable gets a
low-confidence result or an explicit "could not analyse" — never a confident wrong
answer. Two-decimal BPM is gated on a grid residual sd under 15 ms, because integer lag
quantisation at 100 fps cannot resolve it; it comes from a least-squares fit of time
against beat index, or it is not reported. **Never remove a confidence field to make a
column look tidier.**

### Why energy uses fixed published coefficients and no reference database

`analysis/energy/constants.rs`. The score is
`energy = round(1 + 9·Q(Σ wᶠ · Φ((ln xᶠ − μᶠ)/σᶠ)))` over seven features, and the
constants in that file are the whole model. There is **no reference store and no
correlation step**. Three reasons, all load-bearing:

1. **Freezing is what makes the number mean something.** Energy is written into the
   user's file tags, so the constants are a permanent compatibility surface. Changing
   one is a migration with an `ENERGY_MODEL_VERSION` bump, not a tweak. It is also what
   lets a library be re-tagged without re-analysing a file.
2. **Patent.** US 8,865,993 (Mixed In Key LLC, Active, anticipated expiry ~2033) has,
   in all three independent claims, the step of estimating energy by correlating a
   profile against **a stored database of reference compositions with known energy
   values**. A direct feature→score formula with fixed published coefficients, no
   reference store and no correlation step reads outside the claims as written. That is
   an engineering opinion, not legal advice — get a real opinion before the energy
   feature ships, and keep the definition documented and reference-free meanwhile.
3. **It is not loudness.** Loudness alone explains arousal at R² ≈ 0.43–0.55 while
   timbre feature sets reach 0.69–0.75, and every modern master sits near −8 LUFS so
   integrated loudness is nearly constant across a commercial library.

Genre stability comes from the feature design, not from a genre classifier: six of the
seven features are ratios or per-beat rates, and the only absolute quantity
(`dynamic_span`) carries the lowest weight. **Do not normalise per genre** — that needs
a genre detector, and the user's library is not a sample of the world. Test the
invariance instead (`docs/ENGINE.md` §7.3).

Two things people will file as bugs and should not: track energy is the trimmed mean
over the **core region** (deliberately drop-weighted), not the mean of section
energies; and the core region is the span within 6 LU of the 95th-percentile short-term
loudness, without which a 7-minute club edit reads lower than its own radio edit.

### Cue points are starting points, not answers

Published F1 against real DJ cues is 0.22 for Mixed In Key and 0.39 for the best model
in the literature. Exact agreement with a particular DJ is not achievable by anyone,
including the incumbent. So the markers are **labelled, draggable, phrase-aligned
starting points**, the eight slots are assigned by *role* (intro, first beat, two
breakdowns, build, two drops, outro) rather than by top-eight salience, and coverage is
enforced across the track. Taking the top eight by salience clusters them in one section
and makes the feature useless even when every detection is correct.

Everything snaps to a downbeat, or to a strong beat when downbeat-phase confidence is
low. **Never to a raw novelty peak.** The pipeline is built on the **strong-beat grid**
(beats 1 and 3), not the bar grid, because the downbeat cannot be reliably distinguished
from beat 3 in EDM — designing on the reliable grid is what stops a systematic half-bar
offset leaking into every cue.

### Beat This! is behind a feature gate, not in the build

`cargo` feature `beat-this`, arbitrated in `analysis/rhythm/arbiter.rs`. It is MIT for
code *and* weights, which is rare, but the fp32 export is 190 MB and it saturates at
60–68× realtime however threads are arranged. The gate is a 200-track agreement test
against the classical S8 phase: if classical agrees at >90% within ±70 ms, the model is
dropped and 190 MB is never bundled. A 190 MB bundled model is not distributable, and a
download-on-first-run contradicts the offline premise.

## Database schema

One migration, `db/migrations/m001_initial.rs`. FTS5 is in from the start rather than
retrofitted — Stack retrofitted its search index, the rebuild left it empty, and that
is the entire reason Stack carries a multi-second repair pass on every launch.

- **`tracks`** — one row per file. `id` = xxh3 of the path, `path` UNIQUE,
  `content_hash` indexed. Carries file facts, tags as read from the file, the three
  tempo columns (`tempo_raw` / `tempo_display` / `tempo_octave_shift` plus
  `tempo_residual_ms`, `tempo_variable`), the key columns (`key_root`, `key_mode`,
  `camelot`, `key_confidence`, `key_alt_*`, `key_mode_ambiguous`, `tuning_cents`),
  `energy` + `energy_raw` + levels, user data, and the provenance trio
  `key_source` / `tempo_source` / `energy_source` (`detected` | `user` | `tag`).
- **`analysis_cache`** — keyed by `(content_hash, analysis_version)`, payload JSON.
- **`cue_points`** — eight slots per track, `UNIQUE (track_id, slot)`, `position_ms`
  **REAL** because rekordbox counts 1/150 s frames and Traktor uses ms floats.
  `source` is `detected` or `user`, and a user cue is never overwritten by analysis.
- **`beatgrid`** — one row per tempo segment, mirroring rekordbox's `TEMPO` element.
- **`track_regions`** — per-section start/end, kind, energy and key. Costs nothing now;
  it is the read model a deck view would need, and a migration later.
- **`collections`** + **`collection_tracks`** — ordered membership, `kind` user|smart.
- **`watched_folders`**, **`tag_backup`** (replaced tag bytes), **`export_log`**
  (per-file, resumable by `run_id`), **`file_ops`** (the rename journal).
- **`tracks_fts`** — external-content FTS5 over artist/title/album/genre/grouping/
  comment/filename, plus `tracks_vocab` (fts5vocab). **The three triggers are what keep
  it from going stale**; an external-content table does not follow its content table on
  its own.

Two index rules, both the same rule twice:

- **Every partial index repeats `WHERE index_status != 'missing'` verbatim,** and so
  does every query branch. A predicate spelled differently in the query than in the
  index makes SQLite fetch every row back to re-check it.
- **Never filter a path with `LIKE 'prefix%'`.** SQLite's `LIKE` is case-insensitive by
  default, which disables the prefix optimisation and forces a full scan. Use the range
  form via `query_builder::path_upper_bound`.

Composites lead with `camelot` because the wheel selection and the collection are the
always-present filters. `idx_tracks_artist_nocase` carries `COLLATE NOCASE` because it
is the default sort and without the collation in the index every page load sorts the
whole table through a temp B-tree.

`camelot` is text, so sorting it needs the number cast — a plain sort puts `10A` before
`1A`. `query_builder::camelot_clause` builds the compatibility set as an `IN (...)`.

## Tempo folding happens at read time

The three tempo columns are stored separately so that changing the Tempo range setting
re-folds **from the database, with no re-analysis** (`refold_tempo_display`). And fold
the *chosen* tempo — never choose the candidate by what folds into range, or a correct
170 BPM DnB track and a wrong 85 BPM estimate become indistinguishable in the report.

## Renaming is journal-first

Renaming is the **only irreversible operation in the app**, and one setting runs it
automatically after processing. So:

- The `file_ops` row is written **before** `fs::rename` and cleared after the `UPDATE`.
  `recover_renames` replays the journal at startup, before anything reads a path — a
  crash between the rename and the row update otherwise leaves the table pointing at
  files that have already moved.
- Every run is a **dry-run preview first**, with an undo after.
- The hazards are all silent-wrong-behaviour rather than errors, and each has a test:
  case-only renames via a staging name (APFS is case-insensitive), NFD vs NFC
  comparison (macOS hands you NFD), idempotence (`Track - 8A - 8A` must be impossible),
  Windows-illegal characters and reserved names, and the 255-byte component limit
  truncating the **original** segment, never the key/tempo suffix.

The same journal-first, backup-first, verify-after discipline governs tag writes and
cue export. `tags/write.rs` is copy-modify-verify-rename with mtime restoration, and
every replaced tag's bytes go into `tag_backup` first.

## Tags: the one rule that matters

**Never round-trip a DJ library file through lofty's generic `Tag`.** Its own docs say
conversion discards "anything without a mapping", so one bulk REPLACE ALL over the
generic type would silently delete every Serato GEOB blob, Traktor blob, ReplayGain and
iTunNORM frame in the library — invisibly, until the user opened Serato. Use
`Id3v2Tag`, `Ilst`, `VorbisComments` and `ApeTag` only.

That rule is about **writing**. `tags/read.rs` deliberately uses the generic type: a
read cannot lose what is still on disk.

`TKEY` has a spec-legal value set and **Camelot is not in it** — at most 3 characters
from `A`–`G`, `b`, `#`, `m`, plus `o`. `Am` and `F#m` are legal; `11A` is not. Others
write Camelot anyway and the DJ apps accept it, but third-party taggers may mangle it,
so the tag write follows the existing Key Notation setting and defaults to
sharps/flats, with Camelot an explicit opt-in flagged non-standard. Do not add a second
control for it.

## Cue-point export

Three targets, three different problems, ranked by difficulty: **rekordbox XML <
Traktor NML < Serato**. `export/safety/` was built first and is not optional — every
rule in it exists because of a specific way to destroy an irreplaceable library. The
order is enforced by `export::run`, not by documentation:

1. **Refuse while the DJ app is running.** For Traktor this is not politeness: it holds
   the collection in memory and rewrites `collection.nml` on exit, so an external edit
   made while it runs is silently discarded — and a half-written file can be read in and
   re-saved, laundering corruption into the backup rotation. Process-name match (by
   basename, case-insensitive, prefix counts) **plus** a lock probe.
2. **Dated backup**, `KEEP_BACKUPS` deep, per target.
3. **Atomic write** through `tempfile::persist` — plain rename cannot replace on
   Windows.
4. **Verify after**, then write the `export_log` row.

- **rekordbox** — generates a `DJ_PLAYLISTS` document with a `COLLECTION` and one named
  playlist node. **XML only. Never rekordbox's own database or the ANLZ files**, and the
  UI says so rather than offering a checkbox that implies more.
- **Traktor** — an event-stream rewrite of `collection.nml` that splices only `CUE_V2`
  and the key and **preserves every byte it does not model**. `MUSICAL_KEY` numbering
  and `CUE_V2/@START` units are the two things to verify against a real install before
  trusting the writer; getting either wrong writes a silently broken library and no unit
  test catches it. `MUSICAL_KEY` is advanced opt-in only.
- **Serato** — **v1.1, stubbed on purpose.** See `export/serato/mod.rs`, whose header
  records the whole shape of the job, including the `Markers_` trap (fixed 14 slots;
  writing a short one destroys the user's loops with no error), the AGPL/MPL
  `triseratops` licence split, and the fact that **writing tags does not update
  Serato's library** — the user has to run Rescan ID3 Tags, and without that sentence in
  the UI the feature gets reported as broken by people for whom it worked. **Never Ogg
  Vorbis.**

## Accuracy claims

**Keystone publishes no accuracy number until the gate in `docs/ENGINE.md` §7.1 is
met,** and it does not claim to beat Mixed In Key. The bar is 75.70 MIREX weighted /
69.37% exact on GiantSteps, and nothing openly licensed beats it today.

The harness reports all five MIREX categories plus exact match, mode accuracy and
key-signature accuracy **separately** — never just the weighted total. GiantSteps is
84.8% minor, so a minor-biased model posts a good weighted score while being useless.
The synthetic corpus the unit suite uses prints its own caveat at the top of the report,
because a figure copied out of a test log without its caveat is exactly the failure this
product is an argument against. **Keep that caveat attached to every number.**

## Tauri commands

Registered in `handlers.rs` via `keystone_invoke_handler!`. Adding one is a single line
there and a `#[tauri::command]` in `commands/`. Nothing else.

- **library**: `add_watched_folder`, `remove_watched_folder`, `get_watched_folders`,
  `scan_folder`, `get_scan_progress`, `cancel_scan`, `run_reconciliation`,
  `clean_cache`, `get_library_folders`
- **track**: `search_tracks`, `get_track`, `get_tracks`, `get_wheel_counts`,
  `get_genres`, `get_collection_page`, `set_rating`, `set_track_key`,
  `set_track_tempo`, `set_track_energy`, `remove_tracks_from_library`,
  `reveal_in_finder`, `export_file`, `preview_rename`, `rename_tracks`,
  `remove_key_from_filename`, `increment_play_count`
- **analysis**: `enqueue_analysis`, `enqueue_all_unanalyzed`, `reanalyze_tracks`,
  `get_analysis_progress`, `pause_analysis`, `resume_analysis`, `cancel_analysis`,
  `get_waveform`, `get_track_regions`, `get_analysis`
- **cue**: `get_cue_points`, `get_cue_counts`, `add_cue_point`, `move_cue_point`,
  `delete_cue_point`, `remove_cue_points`, `get_beatgrid`
- **collection**: `list_collections`, `create_collection`, `rename_collection`,
  `set_collection_color`, `delete_collection`, `reorder_collections`,
  `add_tracks_to_collection`, `remove_tracks_from_collection`,
  `reorder_collection_tracks`
- **tag**: `get_track_tags`, `update_track_tags`, `get_cover_art`, `set_cover_art`,
  `global_replace`, `remove_extra_spaces`, `retag_tracks`, `retag_existing_files`,
  `remove_key_from_tags`, `list_tag_backups`, `restore_tag_backup`
- **export**: `detect_export_targets`, `set_export_path`, `dry_run_export`,
  `export_cue_points`, `list_export_backups`, `restore_export_backup`
- **player**: `get_playback_path`, `get_start_offset_ms` — returns a path for
  `convertFileSrc`; no decoding on this path
- **settings**: `get_settings`, `update_settings`, `refold_tempo_display`
- **update**: `check_for_update`

## Tauri events (backend → frontend)

Names live in `events.rs` and mirror into `src/types/ipc.ts`. Never inline a string
literal at an `emit` call site. Listened in `useAnalysis` and `useLibrary`.

- `keystone://analysis-progress` → `analysisStore`. **Emitted on a timer, not per
  track**: at 8 workers a per-track emit floods the webview.
- `keystone://track-updated` → carries the whole row so the table patches in place
  without a refetch.
- `keystone://queue-finished` → the sidebar's `Empty` is a state, not a percentage.
- `keystone://scan-progress`, `keystone://reconcile-complete`,
  `keystone://waveform-ready`
- `keystone://export-progress`, `keystone://tag-progress` → keyed by `run_id` so an
  interrupted 5,000-file run is reportable and resumable.

## Design system

CSS custom properties in `src/index.css`; Tailwind extends `keystone.*`, `gray.*` and a
generated `camelot.*` scale. Tokens are stored as space-separated RGB triplets with no
`rgb()` wrapper, which is what lets Tailwind's `<alpha-value>` compose opacity
modifiers (`bg-keystone-lime/10`).

- `--keystone-black` `#0B0D0A` (bg) · `--keystone-lime` `#99F23F` (accent/CTA) ·
  `--keystone-moss` `#87D936` (shade, pressed, gradients) · `--keystone-white` `#F7F7F7`
- Keystone's accent is one rotation of a shared palette: **Stack `#F2613F`, Keystone
  `#99F23F`, Capstone `#3FC5F2`.** Same hue geometry, different position on the wheel.
  Keep it that way when adding surfaces.
- **Fonts: Syne** for headings and labels, **DM Mono** for every number — BPM, key
  badges, cents, energy, timecodes, file paths.
- Light theme is a **semantic flip**: same token names, inverted values. The lime is
  pulled well down in lightness there (`#5FA81B`), because `#99F23F` on white fails
  contrast at any text size and the accent carries buttons and the playing-key
  highlight. `--color-on-accent` is near-black in **both** themes — never
  `--color-text`, which inverts.
- The naming caveat inherited from Stack: `--keystone-black` / `--keystone-white` are
  the brand background / foreground tokens, so in the light theme the names read
  backwards while rendering correctly. Prefer the semantic `--color-*` tokens in new
  code.
- **Keep UI copy short.** The owner's standing feedback, more than once: too much text.
  Labels, not sentences.

Brand source artwork lives in `../keystone-assets` and its README documents how the
Tauri icon set was generated. The app's own inline marks are
`src/assets/logo/*.svg` + `components/ui/LogoMark.tsx`.

## Known stubs / TODO

Everything below is an explicit `todo!("owes: …")` in the tree, so a stub announces
itself at runtime rather than returning a plausible wrong answer.

- **`analysis/decode.rs`** — the one shared decode pass, the probe, and the audio-region
  offset. Nothing downstream can run on a real file until this lands, and the
  `criterion` decode benchmark is what sizes the whole queue.
- **`analysis/mod.rs::analyze_track`** — the S1..S11 orchestration itself.
- **`analysis/frontend.rs`** — the shared STFT / filterbank / HPSS materialisation.
  `structure/spectra.rs` calls into it, so S9, S10 and S11 are implemented and unit
  tested against synthesised features but cannot yet be driven from audio.
- **`analysis/key/skey.rs`** — the ONNX session. The bundled graph is not in
  `src-tauri/resources/` yet, so **the key stage currently runs the template voter
  alone**. The label order and its two anchors are already tested.
- **`analysis/chroma.rs` and `analysis/tuning.rs` are dead shells.** The live
  implementations are `analysis/spectral/chroma.rs` and
  `analysis/spectral/tuning.rs`. This is the same hazard `analysis/mod.rs` describes for
  the rhythm modules — a second, unimplemented copy that compiles and does nothing, with
  every `todo!()` reachable from a plausible name. **Delete them, or make them
  re-exports; do not implement them a second time.**
- **`core/renamer.rs`** — the plan/apply pair and the five SPEC.md patterns. The journal
  and the path hazards are done and tested in `tags/rename/`.
- **`export/serato/`** — v1.1 by design.
- **`tauri.conf.json` has an empty updater `endpoints` array.** Point it at the
  `latest.json` asset on the latest release before the first tagged build, or installs
  will never see an update and nothing will report an error.
- **`src-tauri/resources/` is empty** and `docs/screenshot.png` does not exist yet.
- No Windows measurement of anything. Windows is roughly two thirds of installs;
  re-measure before any "time remaining" is shown to a user.

## Common gotchas

- **White screen after adding an icon** → the name does not exist in
  `iconsax-reactjs@0.0.8`. Check the package's `index.d.ts` before importing; the header
  of `components/ui/icons.ts` lists the names that have already cost time.
- **"no reactor running" panic** → `tokio::spawn` inside `setup`. Use
  `tauri::async_runtime::spawn`.
- **The analysis cache emptied itself after a re-tag** → the content hash is being taken
  over the file head instead of the audio region. See "Identity".
- **A bulk tag write lost Serato's cues** → something went through lofty's generic
  `Tag`. Concrete tag types only.
- **A Traktor export "did nothing"** → Traktor was open and rewrote `collection.nml`
  from memory on exit. The safety layer should have refused; check the process match.
- **Serato shows no cues after a write** → the user has not run **Rescan ID3 Tags**.
  That sentence belongs in the UI.
- **The facet or wheel query got slow** → a `WHERE index_status != 'missing'` predicate
  is spelled differently from the one in the partial index, or a path filter is using
  `LIKE`.
- **A new BPM/key rule did not apply to analysed tracks** → the cache is keyed by
  `content_hash` + `ANALYSIS_VERSION`. Bump `ANALYSIS_VERSION` in `analysis/version.rs`,
  and understand that it re-analyses every cached track.
- **Energy numbers all moved at once** → someone edited
  `analysis/energy/constants.rs`. That is a migration with an `ENERGY_MODEL_VERSION`
  bump, not a tweak; those numbers are already in users' file tags.
- **Frontend asset URLs** must go through `convertFileSrc`; direct `file://` will not
  work. The asset protocol is enabled in `tauri.conf.json` with scope `["**"]`.

## Attribution rules

- Commit messages and PR descriptions carry no co-author or tool attribution lines.
- No tooling is named or credited in source files, comments, or documentation.
