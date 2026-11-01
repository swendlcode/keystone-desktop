//! Serato — **v1.1, not v1.**
//!
//! Deferred not because it is undoable but because it is the only target that
//! rewrites thousands of irreplaceable audio files, it spans four containers,
//! and its testing tail is longer than its implementation. The two XML targets
//! ship first so the safety harness earns its keep where a mistake costs a
//! restore rather than a re-rip. If the schedule forces one hard target into
//! v1, ship **MP3 only** and defer AIFF, FLAC and M4A.
//!
//! **Licence, and it is specific.** Every published `triseratops` crates.io
//! version is `AGPL-3.0-or-later` and is **forbidden** in an MIT product. The
//! repository was relicensed to MPL-2.0 and never republished, so the usable
//! artifact is a pinned git revision with its LICENSE vendored and the SPDX
//! header relied upon recorded in `THIRD-PARTY-LICENSES`. The crate is 0.0.x and
//! self-describes as under heavy development, so vendoring is protection as well
//! as diligence. The fallback is a clean-room implementation from the CC BY-SA
//! 4.0 `Holzhaus/serato-tags` documentation — read the spec, and never paste its
//! prose or byte tables into source comments, or ShareAlike attaches to them.
//!
//! What it will involve, recorded now so the estimate is not relitigated:
//!
//! - Write `Serato Markers2` **and** `Serato Markers_`, consistently. Markers2
//!   is base64 with a linefeed every 72 characters, minimum ~470 bytes
//!   null-padded, entries as a NUL-terminated type string + big-endian `u32`
//!   length + payload. `Markers_` is **fixed length at 14 slots** (first 5 cues
//!   plus 9 loops) in `serato32` 7-bit packing — **writing a short one destroys
//!   the user's existing loops with no error.** When the two disagree, Serato
//!   prefers `Markers_`.
//! - Also write `Serato BeatGrid` and `Serato Autotags`. **Skip `Serato
//!   Overview`** — it is the waveform, tens of KB per file, and Serato
//!   regenerates it. **Test `Serato Analysis`:** whether omitting or
//!   mis-versioning it triggers a full re-analysis is unknown. Leave
//!   `Serato Offsets_` alone; it is not reverse engineered.
//! - Containers: ID3v2.4 `GEOB` frames for MP3/AIFF; base64 Vorbis comments for
//!   FLAC; `----:com.serato.dj:*` freeform atoms for MP4 (note `autgain`, sic).
//!   **Never Ogg Vorbis** — its layout differs significantly and is undocumented
//!   even by the people who reverse-engineered the rest, so there is nothing to
//!   implement against.
//! - Serato reads only the **first 5 cues** from the MP4 `markers` field, so on
//!   M4A the legacy tag carries a partial set and Markers2 must carry all eight.
//! - FLAC and MP4 **cannot be round-tripped losslessly**: Serato writes junk
//!   bytes even when nothing changed.
//! - **Stored cue colours are not the colours Serato displays** — there is an
//!   undocumented palette remap. Verify against a real install.
//! - **Writing tags does not update Serato's library.** 3.x caches in
//!   `_Serato_/database V2`; Serato DJ Pro 4 moved to `master.sqlite`. The user
//!   must run **Rescan ID3 Tags** in the Files panel. Without that sentence in
//!   the UI, the feature gets reported as broken by people for whom it worked.

pub mod grid;
pub mod markers;
pub mod serato32;
