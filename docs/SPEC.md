# Keystone — product specification

Harmonic-mixing analysis for DJ libraries. Our own take on Mixed In Key.
Sibling to Stack (sample library) and Capstone (audio correction).

Accent `#99F23F` / `#87D936`. Same font and icon system as Stack and Capstone,
dark-first. Copy stays short.

---

## 1. Main window

Three regions: the wheel and collections on the left, the player across the top,
the library table filling the rest.

### Camelot wheel (top-left)

The colour-coded circle of fifths: `1A`–`12A` on the inner ring, `1B`–`12B` on the
outer. Selecting a segment filters the library to that key plus its compatible
neighbours (±1 on the ring, and the relative major/minor across it). The playing
track's key lights up. This is the app's signature object — it has to feel good.

### Sidebar

- **Analysis Queue** — shows `Empty` when idle, otherwise progress.
- **Collections**, user-creatable via a `+` on the header. Observed: `My Collection`,
  `Improve Tracks` (245), `Selected` (172), `Swendl Crate - Miami` (50). Each row
  carries a track count. Rename, delete, reorder.
- `ADD TRACKS` button above.

### Player (top)

- Transport: previous / play / next.
- Full-width waveform with **eight cue markers** (`CUE 1`…`CUE 8`) drawn in place,
  draggable, each clickable to jump. Zoom `−` / `+`. Elapsed / total (`00:00 / 02:26`).
- Under it: `ARTIST - TITLE`, then the readout row —
  `KEY 12A` · `ENERGY 6` · `BPM 130` · `CUE POINTS ◀ ▶` · `ADD CUE` · `VIRTUAL PIANO`.
- Virtual piano opens a keyboard showing the detected key's scale.

### Library table

Search box, track count (`412 TRACKS`), sortable columns. Default visible set:
`COVER ART · ARTIST · TEMPO · KEY RESULT · TITLE · ENERGY · CUE POINTS · DATE ADDED · RATING`.
Key result is a coloured Camelot badge matching the wheel. Rating is five dots.
Available-but-hidden columns listed under Display Options below.

---

## 2. Tag editing

A second view over the same library, for cleaning metadata in bulk.

- Tabs: **Global Replace** / **Remove Extra Spaces**.
- Global replace: `REPLACE [ ] WITH [ ]` across the columns
  Artist, Title, Key, Genre, Album, Grouping, Comment → `REPLACE ALL`.
- Table here shows `COVER ART · ALBUM · ARTIST · TITLE · TEMPO · KEY · ENERGY · GENRE`,
  cells editable in place.
- Deep metadata editing per track: cover art, album, year, genre, grouping, comment,
  plus everything the file's tags carry.

---

## 3. Settings

Left rail: Personalize · Send Feedback · Update Tags · Export Cue Points ·
Rename Files · Key Notation · Tempo · Audio Player · Display Options · About.

### Update Tags
`Update common tags` on/off, then:

**What to write** (one of): key only (`10A`) · energy with the word Energy
(`Energy 7`) · key and energy with the word (`10A - Energy 7`) · energy only (`7`) ·
key and energy (`10A - 7`) · key and tempo (`10A - 126`).

**Where to write it** (one of): in front of the artist · in front of the song name ·
at the end of the song name · in front of the comments · overwrite comments.

Plus: `Update custom "Initial Key" tag` (the dedicated key field Serato and Traktor
read) · `Write energy level in front of the Grouping` · `Update "Tempo" tag` with a
`Keep existing tempo values` sub-option. Action: `RE-TAG EXISTING FILES`.

### Export Cue Points
Checkboxes for **Serato**, **Traktor**, **rekordbox**, each with a detected path and
a `CHANGE` button; disabled with an explanation when the app isn't installed.
`Overwrite cue points already in the collection` toggle.
Action: `EXPORT CUE POINTS FOR EXISTING FILES`.

### Rename Files
Do not rename / rename automatically after processing. Pattern (one of):
`Original - Key` · `Original - Key - Tempo` (recommended) · `Key - Original` ·
`Key - Tempo - Original` · `Tempo - Key - Original`.

### Key Notation
Primary: **Camelot** (`11A`) · Flats (`G♭m`) · Sharps (`F#m`).
Extra key column: None · Flats · Sharps.

### Tempo
`Restrict displayed tempo to this range` with lowest/highest BPM (79 / 192) —
anything outside is doubled or halved rather than reported wrong. Decimal places:
none (`128`) · one (`128.1`) · two (`128.14`).

### Audio Player
Play automatically when loaded, or require a click (the second is for when other DJ
software is running). `Start playing the track 25% of the way in`.
`Cache audio wave image during analysis`.

### Display Options
Column visibility: Cover Art · Path · Artist · Tempo · Energy · Cue Points ·
Date Added · Grouping · Genre · Year · Album · Volume · Clipped Peaks · Rating ·
Bitrate · Comment.
`Add a zero to single-digit Camelot numbers`.
`Display a message if the file can be improved` — this is where Keystone points at
Capstone.
Waveform: `Show detected cue points` · `Show detected energy levels`.

---

## 4. Track context menu (right-click)

```
Play Song
Song Info...
Reveal in Finder
Export File...
---
Remove Key from File Name
Remove Key from Common Tags
Re-tag Song
---
Export Cue Points
Remove Cue Points
---
Re-analyze Song
---
Columns            ▸
Key System         ▸
---
Remove Song from Library...
```

---

## 5. Mashup / DJ Mix — DEFERRED, not in v1

A two-deck surface: deck A and deck B side by side, each with its own cue-marked
waveform and energy labels per section. Centre column toggles **Mashup** / **DJ Mix**,
cycles generated ideas (`Mashup ideas 1 / 52`), offers an idea filter, `Test mashup`
and `Save mashup`, with an A–B crossfader.

Per deck: Camelot badge showing the shift (`10A → 3A`, `-1 semitone`), tempo, energy,
position, a `Change key` dropdown, an `Acapella` / `Stems` source picker, a `Loop 16`
control, `Export`, and `Move to Deck A`.

This is effectively a second product on top of the first, and stem separation alone
is a large dependency. **Confirmed out of v1** — it comes back as its own piece of
work once the analysis engine is proven. Recorded here so the v1 data model does not
paint us into a corner: cue points, per-section energy and key already need to be
addressable per track region, which is what a deck view would later read from.

---

## Engine notes

The analysis core is the whole product, and the thing to get right:

- **Key detection** — chromagram, tuning-aware, with a key-profile correlation
  (Krumhansl–Schmuckler or a trained profile). Mixed In Key measures around 76%
  agreement in published comparisons; that is the bar to beat, and it is beatable
  with a modern approach.
- **Camelot mapping** — key → wheel position, and the compatibility rules the wheel
  filter depends on.
- **Tempo** — beat tracking with the double/half resolution the Tempo setting implies.
- **Energy 1–10** — a perceptual scale, not loudness. Needs a defensible definition
  (spectral flux, onset density, bass weight) and must be stable across genres.
- **Cue points** — detect eight musically useful positions: intro, first beat, drops,
  breakdowns, phrase boundaries on 8/16/32-bar lines.
- **Writing back** — ID3 / MP4 / Vorbis tags, the `Initial Key` field specifically,
  and the Serato / Traktor / rekordbox cue-point formats, which are each their own
  reverse-engineering job.

Reuse from Stack where it fits: symphonia decoding, waveform downsampling, the
virtualized table, the player bar, the design tokens.
