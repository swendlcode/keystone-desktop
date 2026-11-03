# Keystone — Project Context

> `AGENTS.md` is a symlink to this file. Agents that look for either name read
> the same thing, so the two cannot drift. On a Windows checkout without symlink
> support it lands as a text file containing `CLAUDE.md`, which still points the
> right way.

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
cargo run --release --features eval --bin keystone-eval -- --track <file>
                                                # S1..S11 over one real file, every
                                                # field printed. Release, not debug:
                                                # the HPSS filters are the per-track
                                                # cost and debug multiplies them.
cargo run --release --features eval --bin keystone-eval -- <giantsteps dir>
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
    library/            TrackTable (virtualized), TrackRow, columns, AnalysisQueue,
                        BpmPicker / EnergyPicker (the overrides, edited in place)
    player/             PlayerBar, WaveformView (canvas), CueMarkers, VirtualPiano
    tags/               The tag-editing view: TagTable, GlobalReplacePanel, editors
    collections/        Sidebar collections, with drag-to-collection; DragGhost
    settings/           One pane per SPEC.md §3 rail item
    menu/               The track context menu from SPEC.md §4
    layout/             Sidebar, TitleBar, MainPanel
  pages/                LibraryPage, TagsPage, SettingsPage
  hooks/                useLibrary, useAnalysis, useCues, useCollections, usePlayer,
                        useSettings, useKeyboard, useDragSession
  services/             keystoneService — the ONLY layer that calls `invoke()`
  stores/               libraryStore, playerStore, analysisStore, collectionStore,
                        dragStore, settingsStore, uiStore (Zustand)
  types/                Shared TS types, re-exported from index.ts; ipc.ts holds the
                        event-name table
  utils/                camelot, camelotColors, formatters
  data/changelog.json   Release notes, read by CI to build the GitHub release body

scripts/
  camelot-scale.mjs     Colour-ramp check, run by `npm run build`

src-tauri/              Rust backend
  Cargo.toml            Feature flags: `eval` (harness), `beat-this` (the gated model)
  tauri.conf.json       assetProtocol enabled, scope ["**"]; updater artifacts on
  src/
    main.rs             Bin stub; calls keystone_lib::run()
    bin/mcp.rs          `keystone-mcp` — the MCP server. See "MCP server" below
    mcp/                The agent tool surface: context, catalog, stdio, tools/
    bridge/             Loopback HTTP the MCP server talks to while the app runs
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
      pipeline.rs       S1..S11 for one track — the ONLY place the order lives
      decode/           S1: open (probe + audio region), stream, rate, levels
      frontend/         Shared DSP: stft, banks, hpss, curves, spectrum
      spectral/         S2 + S3: stft, whitening, tuning, vqt, chroma
      key/              S4: template + skey voters, profiles, chunks, fuse
      camelot.rs        S5: key -> wheel position + the compatibility set
      rhythm/           S6 + S7 + S8: onset, tempogram, tempo, octave, grid, beats,
                        downbeat, phrase, arbiter (the gated Beat This! path)
      structure/        S9: strong-beat features, SSMs, boundaries, regions
      energy/           S10: features, loudness, frozen constants, score
      cues/             S11: filters, roles, assign, coverage
      chroma.rs         S2 and S3 under the names the rest of the engine calls
      tuning.rs         them by. Re-exports of spectral/, never a second
                        implementation — see "One implementation" below
      version.rs        ANALYSIS_VERSION and ENERGY_MODEL_VERSION
    tags/               Read, write, rename, retag, backup, verify
    export/             safety/ first, then rekordbox/, traktor/, serato/ (v1.1)
    jobs/               The analysis queue, its writer, player reads, settings
    eval/               Dev-only accuracy harness (feature = "eval")
    commands/           Thin Tauri handlers, one file per domain
```

## Cue flags are laid out in lanes

`CueMarkers` assigns each flag a lane and offsets it by `LANE_PX`, because two
cues close together printed their labels on top of each other — a flag at the
right edge rendered as `CUE 7 3`, which is `CUE 7` and `CUE 3` overprinted.
Exactly when you need to tell two nearby cues apart, neither was readable.

Greedy first-fit, and two things about it are load-bearing:

- **Sorted by left edge, not by cue position.** First-fit only yields the
  topmost free lane if boxes arrive in the order they *start*, and a flag past
  `frac > 0.94` folds to the other side of its line — so it starts a whole box
  earlier than its position implies. Sorting by position mis-lanes precisely
  the right-edge flags, which is where the collision was found.
- **Overlap is a question about pixels**, so it needs the waveform's measured
  width. That is passed down from `WaveformView`, which already has it from a
  `ResizeObserver`; a second observer here would measure the same element
  twice. `FLAG_WIDTH_PX` is an estimate rather than a measurement on purpose:
  measuring means render, read the box, re-render, twice per frame while
  panning, to decide something that only has to be approximately right.
  Over-estimating spends a lane nothing needed; under-estimating lets two
  labels touch, which is the bug.

## Writing to SQLite: always an IMMEDIATE transaction

**`db::write_tx` for any transaction that writes. Never `conn.transaction()`.**

`Connection::transaction()` is DEFERRED: it takes a read lock on the first
read and asks SQLite to upgrade it on the first write. SQLite refuses that
upgrade outright with `SQLITE_BUSY` when another connection has committed in
between, and the busy handler is **deliberately not consulted** for an
upgrade, because making two readers wait to upgrade would deadlock both. So
`busy_timeout = 10000` does not cover this at all, which is the opposite of
what the pragma list suggests.

`collection_repo::add_tracks` reads `MAX(position)` and then inserts, which is
exactly that shape. Six concurrent writers lost 7 of ~180 calls to
`database error: database is locked`. It is not an MCP-only path either:
filing tracks into a crate while the indexer writes is the same two
connections doing the same thing, and the user would see it as a drag that
sometimes does nothing.

`two_writers_filing_at_once_do_not_collide` is the guard, and it is a real
one — reverted to `conn.transaction()` it fails with 8 errors.

Stack had this right from the start (`TransactionBehavior::Immediate`
everywhere it writes); Keystone was written later and did not inherit it.
Migrations are the one place DEFERRED is fine, because they run before a
second connection exists.

## Adding tracks to a collection tolerates ids that have gone

`add_tracks` inserts through `SELECT ... WHERE EXISTS (SELECT 1 FROM tracks
WHERE id = ?)` rather than a bare `VALUES`. `INSERT OR IGNORE` does **not**
cover a missing track: SQLite's conflict clause does not apply to FOREIGN KEY
constraints, so an unknown `track_id` aborted the statement and rolled the
whole transaction back — ninety-nine good tracks not filed because the
hundredth had been removed since the caller read the list, reported as
`database error: FOREIGN KEY constraint failed`.

An agent working from a list it fetched a minute ago hits that routinely, and
so does a drag that races the reconciler. Callers tell the difference by
comparing the returned count with what they asked for; the MCP tool reports
`added`, `requested` and `skipped`.

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
- **A fitted constant carries its corpus, and none of them is general.**
  `key::fuse`'s four confidence gates and `energy::constants`' fourteen moments plus
  eleven decile knots were fitted on **one DJ's library** — `docs/ACCURACY.md` §1, house
  and pop, 89.4% minor labels, no classical, jazz, live or rubato material — with half of
  it held out. Each one states in its own comment what it was fitted on, on what date,
  and what would invalidate it, and that comment is part of the constant: a bare number
  is a trap for whoever reads it next. **Refit against a broader corpus before anyone
  calls either model calibrated without the qualifier**, and treat a change to the key
  profiles, `template::SOFTMAX_TEMPERATURE`, the chroma front end, the seven energy
  features, the HPSS reductions or the core-region rule as invalidating the fit outright.
  Re-run the passes in `docs/ACCURACY.md` §10; never nudge a knot.
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

One decode feeds every stage, so the only optimisation that matters at this level is
not decoding twice. **Never hold a decoded track longer than the stage that needs it** —
a 10-minute 48 kHz stereo f32 buffer is 230 MB, and eight workers hold eight.

Decode is roughly 90% of per-track cost *on paper*. Measured through the finished
pipeline it is the smaller half: the structure stage's HPSS median filters cost more.
That is a note for whoever sizes the queue, not a licence to decode twice.

The order is **not rearrangeable**. `analysis/pipeline.rs` owns it — `analysis/mod.rs`
is the module tree and the shared `AnalysisContext`, and the two are separate files
because a sequence and a module tree change for different reasons.

| Stage | Does | Lives in |
|---|---|---|
| S0 | Walk, per-path id, pending row | `core/scanner.rs`, `core/indexer.rs` |
| S1 | Decode once → mono 44.1 k, 22.05 k only when a model session exists, true peak, clipped count, streamed peaks | `analysis/decode/`, `core/peaks.rs` |
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

### The decode path, and the single-pass rule

`analysis/decode/` is S1. It has three entry points and they are not
interchangeable:

- **`probe`** is header-only. It is cheap enough to run during the walk, which is why
  the table can show duration and bitrate before anything has been analysed.
- **`audio_region`** returns the byte offset and length of the audio payload. This
  exists so the content hash can skip the tag block: ID3 sits at the *head* of an MP3,
  so hashing the first bytes of the file means Keystone's own re-tag pass changes every
  hash and silently wipes the analysis cache it just filled. It looks like it works
  right up until the first re-tag. `pipeline.rs` hashes through
  `hasher::hash_audio_region`, never over file bytes.
- **`decode`** is the one pass. Arithmetic-mean downmix to mono, rubato for the rate
  conversion, and true peak plus the clipped-sample count accumulated *in the same
  loop*. Waveform peaks accumulate in that loop too when asked, which keeps them
  O(target buckets) rather than O(track).

Two rates, and only one of them is unconditional. `ANALYSIS_SAMPLE_RATE` is 44,100 and
everything reads it; `KEY_SAMPLE_RATE` is 22,050, fixed by the S-KEY graph rather than
chosen, and `DecodeOptions::want_key_rate` is set from `ctx.skey.is_some()`. The second
buffer is the most expensive thing in a decode after the first, only the model voter
reads it, and there is no bundled graph — **no session, no buffer.**

The single-pass rule has a second half that is easy to lose: *one* decode, and the
buffers die as early as possible. `pipeline.rs` scopes the key stage so the
tuning-corrected copy and the 22.05 kHz buffer are both released before S9 materialises
three spectrograms over the same track, and `key_stage` takes `mono_22050` **by value**
precisely because it is the last reader — returning it would hold a second copy of the
track alive through the memory high-water mark. S12 is deliberately *not* in
`analyze_track`: eight workers competing for the single SQLite writer would spend the
per-track budget in busy retries, so persistence is the caller's through `jobs/writer.rs`.

The symphonia pin is **0.5 on purpose.** The proven streaming-decode shape is a 0.5 call
site and 0.6 redesigned the audio primitives; the port is its own task, paid for once.
This module is also meant to be lifted into a crate shared with Capstone, so it must not
reach for anything Keystone-specific beyond the error type.

`benches/decode.rs` wants `KEYSTONE_BENCH_CORPUS` pointed at twenty real tracks spanning
MP3, AIFF, FLAC and M4A. Without it **it reports that it was skipped rather than
inventing a number from synthetic audio**, which would measure the wrong thing. Keep
that behaviour.

### The shared front end: why 4096/512 and a 17x17 HPSS

`analysis/frontend/` holds the STFT, the filterbanks, HPSS and the band splits —
everything onsets, structure and energy have in common, so none of them recomputes a
spectrogram another already has. `structure/spectra.rs` is the one caller that chooses
the transform size, and it chooses `N_FFT = 4096`, `HOP = 512` at 44.1 kHz.

- **4096, not 2048.** The 84-bin structure vector starts at C1 (32.7 Hz). A 2048-point
  window puts only one FFT bin below 43 Hz, which collapses the kick band every
  structure feature depends on.
- **Hop 512** is S9's stated hop: 11.6 ms a frame.
- **Materialised, not streamed**, unlike the key path's front end. A self-similarity
  matrix is not expressible as one pass over frames — S9 and S10 read the same
  spectrogram repeatedly and along both axes. The cost is real and known: a five-minute
  track at 4096/512 is 25,800 frames of 2,049 bins, ~212 MB, and three of those
  matrices exist at once. There is **exactly one** per track for that reason.
- **The window convention is a parameter, never a default.** Symmetric Hann sums to
  `(n-1)/2` and periodic sums to `n/2`; a stage that windows with one and scales as if
  it used the other is wrong by that ratio in *every* magnitude it produces. Periodic
  plus `normalize_by_sqrt_n` is what the Beat This! front end expects and is wrong for
  everything else. Scratch buffers are allocated once per call, not per frame — 25,800
  allocations a track times eight workers is the alternative.
- **Filterbanks are sparse, never a dense matmul.** The same spectrogram measured 45 ms
  through the sparse shape against 1.07 s through a dense multiply. A 24x penalty for a
  two-line difference.

HPSS (Fitzgerald 2010, Driedger's soft masks) runs at **17 frames x 17 bins**, both
forced odd so the median has a true centre:

- **17 frames is 198 ms** at hop 512 — long enough that a kick or snare is a clear
  outlier in the time median, short enough that a chord change two frames later is not
  smeared into the harmonic component. Longer and a filter sweep stops looking
  harmonic; shorter and a 200 ms tom roll starts to.
- **17 bins is 183 Hz** at 4096/44.1 kHz — wider than the few bins a single partial
  occupies, which is the entire mechanism that makes a partial an outlier. It is also
  the method's limit in the bass: below ~180 Hz a bassline's harmonics are spaced
  closer than the filter is wide, so some bass leaks into the percussive component.
  **Widening the filter makes that worse, not better**, since it then spans more
  harmonics still. The kick band is where this separation is weakest, and the
  band-limited flux features exist alongside it for exactly that reason.
- **Masks are built from the squared medians**, soft rather than binary, so the two
  components sum to exactly 1 cell by cell. S10's `perc / (perc + harm)` is only a
  ratio of the mix if `harmonic + percussive == magnitude`.
- **It is the per-track high-water mark in time as well as memory.** 21 million medians
  per minute of audio at 4096/512. Both passes walk the spectrogram row-wise: measured
  per minute of audio on one M-series core, release build, 1.58 s column-wise, 1.04 s
  row-wise, 0.72 s row-wise with the incremental window update. Even so it is ~3.6 s of
  a five-minute track's budget. **The way to close that gap is to hand this stage fewer
  bins, which is the caller's decision, not to micro-optimise the filter.** Measure
  before raising analysis concurrency on a small machine.

### One implementation, two paths to it

`analysis/chroma.rs` and `analysis/tuning.rs` were a second, independent copy of S2 and
S3: the same function names carrying `todo!()`, sitting beside the finished
`spectral::` implementations and reachable from the path the product used, while the
working code was reachable only from `spectral::`. The rhythm trio had the same
pathology earlier. They are now **re-exports plus their own tests**, reached through the
alias on purpose so the assertions fail if anyone ever replaces one with a fresh
implementation.

Finishing a copy is worse than deleting it, and S3 is the worked example: the duplicate
`Pcp` had neither `is_empty` nor `rotate`, so it could not express "no answer" and could
not apply the detune at all, and its `fold_to_12` grouped sub-bins as `i / 3` rather
than the centred `(i + 1) / 3` — a third of a semitone of misalignment, enough to rank
B above G on a C major triad. **Never implement a stage a second time because a
plausible name is empty. Alias it or delete it.**

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

**Two routes out of S2, and the residual is what tells S3 which one was taken.** Past
`CORRECTION_THRESHOLD_CENTS` the buffer is resampled and S3 is handed a residual of
zero; below it the buffer is untouched and S3 is handed the cents, which it applies by
rotating the bin grid (`detune_bin_shift`). Both must land on the same bin, and
`analysis/chroma.rs`'s tests assert exactly that on a note detuned by one whole
sub-bin, plus that the *uncorrected* vector really is on the wrong bin — otherwise
neither path has proved anything.

The `concentration` field is the third thing S2 owes, and it is why the estimate is not
just a number: unpitched material scores below `MIN_CONCENTRATION` and **never asks to
be resampled**. A `cents` value with nothing qualifying it is precisely how percussion
ends up transposing a track.

**Measured, on 1,056 real files** (`docs/ACCURACY.md` §7): median −2.5 cents, quartiles
−6.8 / +1.2, range −33.4 to +33.1, and **189 files (17.9%) beyond ±20 cents.** So S2 is
doing real work on roughly one file in six of a real DJ library, which is worth knowing
given that both Faraldo and Sha'ath measured no aggregate gain on digitally-produced
EDM. That corpus is not all digitally-produced EDM, and neither is the audience's.

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

### The confidence model, and the detection gate that looks like part of it

`analysis/key/fuse.rs`. Confidence is `evidence × √separation × agreement`, with
agreement held above `AGREEMENT_FLOOR` and separation concave on purpose, and four
constants decide what a DJ sees. All four were **fitted 2026-11-01** on the fit half
(532 of 1,063 files) of the corpus in `docs/ACCURACY.md` §1. Before that they came off
synthetic chords, which read nothing like real records: confidence was exactly 0.000 on
64.3% of a real library and below the badge floor on 88.4% of it.

| constant | was | is | what it decides |
|---|---|---|---|
| `SPREAD_FLOOR` | 0.08 | **0.014** | below this the chroma says nothing and evidence is 0 |
| `SPREAD_FULL` | 0.14 | **0.040** | at this the evidence term is 1 |
| `AMBIGUOUS_CONFIDENCE_FLOOR` | 0.25 | **0.12** | below this: no badge at all |
| `MODE_AMBIGUITY_THRESHOLD` | 0.6 | **0.80** | below this: `8A/8B`, not a letter |

Refusals fell from 88.4% of the library to 22.2% (94.0% to 10.0% on full club tracks).
Three things about how that was done are load-bearing:

- **The floor is not where the fit wanted it. It is where the drum-loop promise is.**
  Synthetic percussion reaches 0.0117–0.0132 through this pipeline at every duration
  from 8 s to 120 s, and the thinnest real full track sits at 0.0125 — inside that band.
  0.014 refuses 1.7% of real full tracks on purpose so that percussion keeps being
  refused. The two distributions overlap, so the floor is a price, not a separation.
- **The gate that decides the key is a different gate and was deliberately left where
  it was.** `chunks::CHUNK_WEIGHT_FLOOR` / `_FULL` still carry the synthetic 0.08 /
  0.14, because that gate picks which chunks vote and therefore picks the key, while the
  `fuse` gates only decide how loudly the answer is stated. That is why the calibration
  moved no estimate on any of the 1,063 tracks — key, Camelot, tempo and tuning are
  bit-identical, and the shipped scorer's report byte-identical on all six groups.
  Moving the chunk gate is a detection change and needs its own measurement.
- **`AMBIGUOUS_CONFIDENCE_FLOOR` is a criterion, not a peak in the data.** 0.12 is the
  largest value at which the set it refuses is still below even on the wheel's own test
  — would mixing on that badge work: same code, one step either way, or the relative.
  The curve is flat across the whole range, so the number came from the criterion.

Two consequences are known and are not hidden: a uniform chromatic wash longer than
about 20 s now gets a confident letter (`docs/ACCURACY.md` caveat 13), and 73.9% of
isolated acapellas get a badge of which 41.5% would clash in a mix. Neither is fixed by
moving the floor back — `template::match_chroma` standardises its similarities, so flat
chroma still produces a peaked distribution. The fix is in that front end.

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

**The ladder is energy model 2, fitted 2026-11-01, and version 1 was not fitted at
all.** Its μ and σ came from each feature's *definition* rather than from measurement,
and on real masters two of the seven features saturated (`flux_mean` above Φ = 0.98 on
99.4% of tracks, `dynamic_span` on 40.9%): 160 of the 176 full tracks in the fit half
read 8, 9 or 10, so the scale had no scale in it. Model 2 fits μ, σ and the eleven
decile knots on the full tracks of the fit half of the `docs/ACCURACY.md` corpus — 176
files, the split written to disk before any audio was read — and the **held-out** half
now spreads over all ten values, mean 9.07 → 5.62. Spearman between model 1's `raw` and
model 2's is 0.957: version 1 already ranked the library and threw the ranking away in
the mapping onto 1–10. Fitting a ladder guarantees *spread* and cannot prove *order*;
only `docs/ENGINE.md` §7.3 test 4, the ~300 forced-choice human pairs, can, which is
also why **the seven weights are still not fitted**.

**So a 5 is the median full track in a house and pop DJ library,** and that library is
the whole anchor. Vocal stems and acapellas — two thirds of the corpus — were excluded
from the fit, because letting drum-less stems set the median would push every real
record back to the top of the scale, which is the defect being fixed; the visible
consequence is that an isolated acapella now reads 1 or 2. Two failures ship with it:
the 1–10 reading is within ±1 across separate copies of the same recording on only 34
of 42 pairs, and a 1 dB wide-Q EQ move can shift a step through a downbeat-phase flip
upstream in S8. Both point at the three spectral-balance features, not at the constants.

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
- The hazards are all silent-wrong-behaviour rather than errors. Each one is pinned by
  a test in `core/renamer_apply_tests.rs`, which drives the wired `plan`/`apply` pair
  against a real database and a real directory rather than the rules in isolation —
  that seam, where a name composed from a row has to end up agreeing with a directory
  entry, is where the two halves meet. The six cases, by the hazard they pin:

  | Hazard | Test |
  |---|---|
  | APFS is case-insensitive, so a case-only rename goes via a staging name | `a_case_only_rename_lands_and_the_row_follows_it` |
  | ...and a crash *between* its two steps must still be recoverable | `recovers_a_case_only_rename_interrupted_between_its_two_steps` |
  | macOS hands back NFD; only a normalised compare says it is one file | `an_nfd_spelling_of_the_composed_name_is_not_a_rename` |
  | Idempotence — `Track - 8A - 128 - 8A - 128.mp3` must be impossible | `a_key_and_tempo_suffix_can_never_be_applied_twice` |
  | Windows-illegal characters and reserved device names | `illegal_characters_never_reach_the_disk` |
  | The 255-byte component limit truncates the **original**, never the suffix | `the_component_limit_truncates_the_original_and_keeps_the_suffix` |

- The naming rules and the move itself are implemented **once**, in
  `tags/rename/compose.rs` and `tags/rename/paths.rs`, and re-exported from
  `core/renamer.rs` rather than written a second time. What `core/renamer.rs` owns is
  the half that talks to the database: `plan` is the wired entry point, `apply` is
  journal-first through the repository, and `recover_journal` is what `lib.rs` replays
  at launch. `original_filename` is what makes the idempotence strip exact.
- Two traps in that split. The journal goes through `db/track_repo/journal.rs` because
  all SQL belongs in `db/`, so **`tags/rename/journal.rs` predates it, writes `file_ops`
  with its own inline SQL, and is called from nowhere** — do not wire it back in. And
  `STAGING_PREFIX` is spelled in both `core/renamer.rs` and `tags/rename/paths.rs`
  because `paths` keeps its staging name private; if those two ever drift, an
  interrupted case-only rename becomes unrecoverable in silence.

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

**The gate has not been run.** GiantSteps+ is not on this machine, so no number anywhere
in this repository is an accuracy figure. What *has* been run is `docs/ACCURACY.md`:
1,063 real files from one DJ's library, scored against the Camelot labels Mixed In Key
wrote into their filenames. That measures **agreement with a competitor**, where neither
program is known to be right, and it is not comparable with the 75.70 or with the 76.0
gate — different corpus, different reference, different quantity. Read that document's
§0 before quoting anything out of it, and read §4 first when it is re-run: a weighted
total that improves while most of the library still shows no key badge has not improved
anything a user can see. That was the 2026-10-31 finding — 88.4% of files refused — and
the 2026-11-01 calibration is the answer to it, at 22.2% of the library and 11.1% of
full tracks with the key estimates provably unmoved. **One thing changed about how that
document has to be read:** the binary now carries constants fitted to half of that
corpus, so §4 and §7.1 are no longer out-of-sample. §4.5 and §7.2 are the held-out
versions and the only ones to quote; §3, §5 and §6.1 are unaffected, which §2.3 proves
by measurement rather than by argument.

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

## Dragging rows to a collection

Rows are dragged with the **pointer**, not with HTML5 drag-and-drop. That is a
deliberate replacement, and the reasons are all still live, so it must not go
back:

- `dataTransfer.getData` returns nothing during `dragover`. The payload had to
  be smuggled through the *type* list, and every target guessed at what was
  coming before it could decide whether to light up.
- **`dragDropEnabled` is on** for the window, because Finder imports need
  Tauri's native handler — and that handler sits in front of the webview's own
  drop. The "Drop to analyse" sheet throwing itself over the sidebar the moment
  a row was picked up was the visible half of the same collision, and the flag
  is documented as breaking frontend drag-and-drop outright on Windows.
- The rows sat 2px apart under `gap-0.5` with **no drop target in the gap**, so
  a drop aimed at the boundary between two collections landed on neither.

The mechanism now:

- `stores/dragStore.ts` holds the session — one payload, the pointer position,
  the zone under it — plus `armPointerDrag` (a press becomes a drag only after
  `DRAG_THRESHOLD` px, which is what stops it stealing clicks) and the zone
  registry, which is a plain `Map` rather than store state because zones come
  and go with mounted rows and are read once per pointer move.
- `hooks/useDragSession.ts` owns the window listeners, mounted once in `App`.
  **`pointerup` resolves the drop with the same hit test `pointermove` used for
  the highlight**, so the row that lit up is by construction the row that
  receives. Keep that identity: the old code decided the highlight in
  `dragover` and the destination in `drop`, and two hit tests are two chances
  to disagree.
- Hit-testing is `document.elementFromPoint` against `[data-drop-zone]`, read
  off the DOM rather than from cached rectangles, so a list that scrolls or
  reflows mid-drag cannot hand back a stale target. The ghost is
  `pointer-events-none`, which is what keeps it from being the answer to every
  hit test.
- Each `CollectionRow` carries the gap to the row below it as padding rather
  than letting the list space them, so there are no dead pixels.
- `armPointerDrag` bails when `ev.buttons === 0`. A button released outside the
  window never reaches the listener, and without that check the arming
  survives and the next idle pointer move starts a drag nobody pressed for.
- `collections/DragGhost.tsx` is the drag image, and it shows the carried
  track's **key, energy and tempo**. `setDragImage` could not: it snapshots at
  `dragstart` and never again. Filing a track into a crate is a judgement about
  how it will mix, and the drag used to cover the row whose numbers justified
  it.
- `draggingTracks` is **derived** from the session, not stored. It used to be
  its own field written by the row that started the drag and cleared by
  whichever of `dragend` or `drop` ran first — two writers for one fact, and a
  drop that never fired left the sidebar lit with nothing in the air.

Finder imports are untouched and stay on the native handler. `useFileDrop`
keeps its guard against an internal drag even though a pointer drag starts no
native session: the cost of being wrong is the overlay covering the drop
target again.

## Four backend paths were registered and called by nothing

`set_track_key`, `set_track_tempo` and `set_track_energy` were each written,
registered in the invoke table, and called by nothing — so the three numbers a
DJ is most likely to want the last word on were the three that could not be
corrected from inside the app. All three now have a picker:

- `player/KeyPicker.tsx` — the wheel, as the picker.
- `library/EnergyPicker.tsx` — a 1-10 ramp, in the player bar *and* the column.
- `library/BpmPicker.tsx` — a tempo field in both places, with **Halve** and
  **Double**. Octave errors are the actual tempo mistake, not arbitrary wrong
  readings: drum & bass read at 87 instead of 174, half-time hip-hop at 140
  instead of 70. `set_tempo` writes raw and display together and zeroes the
  octave shift, because a typed tempo is already what the user wants to see.

Each stamps `ValueSource::User`, which is what stops the next analysis pass
reverting it. A correction a re-analysis undoes is worse than no correction, so
if a fourth override is ever added, it carries that stamp or it is not an
override.

`get_cue_counts` was the fourth of the same kind: a command, a repo method and
a `keystoneService.getCueCounts` whose own comment said it was for the table's
CUES column — and no caller. The column meanwhile read `track.cuePointCount`,
a field the read model has never returned, so it showed `--` on every row
while the player bar showed eight markers for the same track. `useCueCounts`
now fetches one query per loaded page, never per row, and the count reaches
`TrackRow` as a plain number so its memo still compares equal.

**Four is a pattern, not a coincidence.** Before adding a command, check
whether the thing it is for already has one — `npm run dead-paths` answers
that in a second.

### `npm run dead-paths`

`scripts/dead-paths.py` reports two different failures:

- **A registered command with no `invoke('name')` anywhere.** Unreachable from
  the UI. There are currently none; the four above were the last of them.
- **A service method that calls `invoke` but has nothing calling it.** The
  shape that hid all four: the IPC existed end to end and only the last link
  was missing. This is the list worth reading.

It is a report, not a gate — it exits 0 and some of what it lists is
deliberate. As of this writing it flags ten service methods, and they are
*features that were never built*, not bugs: `scanFolder`, `cancelScan`,
`getScanProgress`, `runReconciliation`, `cleanCache` and `getLibraryFolders`
are a whole library-maintenance pane that does not exist — tracks go in
through ADD TRACKS and there is no way to re-scan, reconcile or see the
watched folders. `setCollectionColor` and `reorderCollectionTracks` are
collection features with backends and no controls. `getBeatgrid` would draw
beat markers. `getTracks` is superseded by `searchTracks`' paging.

Two were removed rather than listed for ever:

- **`increment_play_count` is now called**, from `usePlayer` at the point a
  source actually starts. `play_count` was a column, a command, a service
  method and a field on `Track` that nothing incremented, so it read 0 for
  every track for ever — including over MCP, where `get_track` hands it to an
  agent as though it meant something.
- **`check_for_update` is gone.** It was hardcoded to answer "no update
  available" and the real updater is `tauri-plugin-updater`. Registered,
  exposed through the service layer, called by nothing — which is the only
  reason it never told someone on a year-old build they were up to date. A
  stub that lies is worse than no stub.

## MCP server (agent access)

`keystone-mcp` is a third binary in the same crate (`src-tauri/src/bin/mcp.rs`)
that exposes the library as MCP tools over stdio. Register it like any server:

```json
{ "mcpServers": { "keystone": { "command": "<path>/keystone-mcp" } } }
```

**Two transports, chosen per call, not at startup** — the user can open or quit
Keystone mid-session and nothing restarts:

1. **The running app**, via the loopback bridge (`src-tauri/src/bridge/`).
   Writes land in the open window, because they go through the same commands
   a click does and so produce the same query invalidations and progress
   events. Writing straight to SQLite under an open window would leave the
   user looking at a stale row with no way to know.
2. **The database directly** (`McpContext::standalone`), when nothing answers
   the probe. WAL plus the existing 10s `busy_timeout` makes this safe even if
   the app opens mid-session.

Both land in `mcp::dispatch` over an `McpContext`. **That is the whole point of
the context**: the tools are written once against the repositories, and the
only difference between transports is which context they get. Two dispatch
tables would be two chances for the paths to disagree about what a tool does.

- `McpContext::live` is `None` standalone. Tools needing the app — only
  `analyze_tracks`, since the workers live in it — go through
  `require_live()` and return a sentence saying to open the app, rather than
  half-working.
- The bridge binds `127.0.0.1` **and** requires a bearer token, written to
  `<data>/mcp_bridge.json` at 0600. Loopback is not a permission boundary on
  a shared machine and these tools move files. The comparison is
  constant-time.
- The handshake file is a **hint, not a promise**: the app can die without
  removing it, so the client probes `/mcp/health` and falls back. `Exit`
  removes it, which is why `run()` is `.build()` + `.run(|handle, event|)`.
- The client side is a hand-written POST over `TcpStream`, not an HTTP crate.
  It is loopback, the server is this same crate, and the response shape is
  known, so TLS/redirects/chunking/pooling are all absent.
- **Nothing but protocol may touch stdout.** `tracing` is pointed at stderr in
  the binary; a stray `println!` in a tool corrupts the stream and the client
  drops the connection with no useful error.
- Tools are **coarser than the commands** they sit over. `rename_files` plans
  *and* applies behind `dryRun`, because an agent has no modal to preview in.
  `dryRun` defaults to **true** on everything that can touch more than one
  file: a tool call is cheap and a model will make one to find out what would
  happen, so the expensive direction has to be asked for.
- A failed tool is a **result with `isError: true`**, not a JSON-RPC error — a
  protocol error never reaches the model, and the sentence is the only thing
  it has to correct itself with.
- `list_tracks` returns a deliberate projection, not the whole `Track`. Sixty
  fields times fifty rows is the agent's context spent on tuning cents.
### Shipping it: nothing to do

`keystone-mcp` rides in the installers because **tauri-cli ships every
`[[bin]]` in the crate** (2.12 onward), including lipo-ing the extra ones for
a universal macOS build. It lands at
`Keystone.app/Contents/MacOS/keystone-mcp`, beside `keystone.exe` on Windows,
which is the path a user registers.

One thing *is* needed, and only for a universal macOS build: Tauri lipos the
**main** binary into `target/universal-apple-darwin/<profile>/` and then copies
every crate bin out of that directory, without lipo-ing the extra ones. So
`scripts/universal-mcp-bin.mjs` merges `keystone-mcp` into that directory
first, from a step in the release workflow. It is a no-op on Windows, where
the bin is copied straight out of `target/<profile>/`.

**Do not add it to `externalBin`.** It was, briefly, with a staging directory
and a `--config` override. That broke the Windows installer, because declaring
a crate bin as a sidecar installs the same file twice and WiX refuses:

```
error LGHT0204 : ICE30: The target file 'keystone-mcp.exe' is installed in
'[ProgramFiles64Folder]\Keystone\' by two different components on an LFN
system: 'keystone_mcp.exe' and 'keystone_mcp'
```

That is tauri-apps/tauri#14681. The fix is to declare nothing and let the
bundler do what it already does.

Verified from a cleaned tree — universal directory deleted, both per-arch
artifacts removed — so the pass came from the script rather than from
leftovers: the bundle carries `keystone-mcp` as an `x86_64 arm64` fat binary
reporting the freshly built version. The first attempt at this looked like it
worked with no script at all, because a file from a previous run was still
sitting in the target directory.

## Known stubs / TODO

Everything below is an explicit `todo!("owes: …")` in the tree, so a stub announces
itself at runtime rather than returning a plausible wrong answer.

**S1 to S11 now run on a real file.** `analysis/mod.rs::analyze_track` decodes once and
carries one track through tuning, chroma, key, Camelot, onsets, tempo, the grid,
downbeats, structure, energy and cues; `jobs/writer.rs` persists the result and
`eval::run` scores an annotated corpus off disk. `tests/pipeline.rs` is the regression
guard on the seam: synthesised audio, written as a real container, opened by symphonia
and asserted on as a whole `AnalysisResult`. Two stubs remain by decision:

- **`analysis/key/skey.rs`** — the ONNX session. The bundled graph is not in
  `src-tauri/resources/` yet, so **the key stage runs the template voter alone**, and
  `analyze_track` skips the 22.05 kHz decode buffer entirely when there is no session to
  feed it. The label order and its two anchors are already tested. v1.1 research track.
- **`export/serato/`** — v1.1 by design.

Not stubs. The calibration hazard, as it stands after 2026-11-01 — the two defects
below are fixed and what replaced them is narrower, not nothing:

- **Every fitted constant in the binary is fitted to one DJ's house-and-pop library.**
  Four gates in `key/fuse.rs`, fourteen moments and eleven knots in
  `energy/constants.rs`. Nothing measures what either does on a techno, drum-and-bass,
  hip-hop, ambient, jazz or classical library, and `energy/constants.rs` states the
  anchor outright: a 5 is the median full track in a library like that one.
  `docs/ACCURACY.md` caveat 12 is the full statement.
- **Nobody has measured confidence AUC on files no constant has seen.** The fit's own
  held-out figure (0.69) and the whole-corpus figure (0.689) agree, and they are not
  independent. One more split is the cheapest honest number left in the engine.
- **The chunk gate is still synthetic, on purpose.** `chunks::CHUNK_WEIGHT_FLOOR` /
  `_FULL` were held at 0.08 / 0.14 so the confidence refit could not move a key
  estimate. The cost is that on 63.7% of the fit half no chunk clears them and the
  whole-track PCP decides alone, so the chunk machinery contributes nothing there.
  Moving them is a detection change: score `docs/ACCURACY.md` §3 before and after.
- **One known false positive, and the test for it passes by accident.** A uniform
  chromatic wash longer than about 20 s is shown a confident letter;
  `refusal_tests::atonal_material_comes_back_ambiguous_too` asserts refusal at exactly
  10.0 s, which is one of the two durations where it still holds, so it should be
  parameterised over duration. Percussion and silence are still hard zeros from 8 s to
  120 s. `docs/ACCURACY.md` caveat 13.
- **The energy ladder spreads a library but is not reproducible to ±1 across copies
  of one record** — 34 of 42 pairs, worst case 7 (`docs/ACCURACY.md` §7.3) — and the
  seven weights are unfitted because the ~300 human pairs in `docs/ENGINE.md` §7.3
  test 4 do not exist. Spread is measured; order is not.

Costs worth knowing before anything here is optimised:

- **`structure/spectra.rs` is the memory and time high-water mark.** 4096/512 over a
  five-minute track is three ~200 MB matrices and the HPSS median filters dominate the
  per-track cost. `frontend/hpss.rs`'s own header says the fix is fewer bins from the
  caller, not a faster filter, and the caller is that file. Measure before raising
  `effective_analysis_concurrency` on a small machine.
- **`cargo test --features eval` takes ~20 minutes** in a debug build; `key_accuracy`
  alone is ~14 of them. Plain `cargo test` is ~20 seconds, which is the one to run on an
  edit.

Still outstanding, unchanged:

- **Builds up to and including v0.1.0 cannot self-update.** The updater
  `endpoints` array was empty when 0.1.0 shipped, so those installs have no URL
  baked in to check and nothing reports an error. It now points at the
  `latest.json` asset on the latest release, the way Stack's and Capstone's
  always did, so releases after 0.1.0 update normally — but 0.1.0 has to be
  replaced by hand. v0.1.1 is the bridge release.
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
  bump, not a tweak; those numbers are already in users' file tags. It has happened
  once, on 2026-11-01 (model 1 → 2), and the stored numbers were invalidated rather than
  migrated: `ANALYSIS_VERSION` went 1 → 2 with it, so every cached row misses and is
  re-analysed. Two consequences a user can see: the first launch after the upgrade
  re-analyses the whole library a row at a time, so the energy column really does hold
  both models until the backlog drains and the queue indicator is the only thing that
  says so; and a re-tag pass is still needed for tags and filenames written under
  model 1 to agree with the column.
- **One table showing model 1 and model 2 energies at once** → an
  `ENERGY_MODEL_VERSION` bump shipped without an `ANALYSIS_VERSION` bump. The analysis
  cache is keyed on `(content_hash, analysis_version)` and nothing else, so old rows
  keep being served with their old energies. `analysis/version.rs` has a test that
  refuses that combination.
- **Frontend asset URLs** must go through `convertFileSrc`; direct `file://` will not
  work. The asset protocol is enabled in `tauri.conf.json` with scope `["**"]`.

## Attribution rules

- Commit messages and PR descriptions carry no co-author or tool attribution lines.
- No tooling is named or credited in source files, comments, or documentation.
