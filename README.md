# Keystone

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Tauri](https://img.shields.io/badge/Desktop-Tauri_2.0-24C8DB)
![Rust](https://img.shields.io/badge/Backend-Rust-000000)
![React](https://img.shields.io/badge/Frontend-React_18-61DAFB)
![Offline](https://img.shields.io/badge/Runs-Offline-99F23F)

Harmonic mixing analysis for your DJ library. Keystone reads your tracks, works out
key, tempo and energy, and lays them out on a Camelot wheel so you can see what mixes
with what.

Free, open source, and entirely offline. No account, nothing uploaded.

<!-- Screenshot placeholder — drop a capture of the main window (wheel, library, player)
     at docs/screenshot.png before publishing. -->
![Keystone](docs/screenshot.png)

## What it does

- **Camelot wheel.** Click a key and the library narrows to it plus everything
  compatible with it.
- **Key, tempo, energy** on every row, with eight cue markers you can drag on the
  waveform.
- **Honest ambiguity.** When the key is genuinely borderline, Keystone shows the pair
  (`8A/8B`) rather than committing to a letter it cannot stand behind.
- **Write it back.** Key, energy and tempo into your tags, your filenames, or both —
  with a preview before and an undo after.
- **Cue export** to rekordbox and Traktor. Your collection file is backed up first, and
  Keystone refuses to write while the app is open.
- **Tag cleanup** in bulk: global replace, stray spaces, cover art, per-track editing.
- **Collections, ratings and search** across the whole library.

## What it does not do

Stated plainly, because the alternative is finding out later.

- **No Serato export yet.** rekordbox and Traktor only. Serato writes into the audio
  files themselves across four container formats, so it comes when it can be tested
  properly rather than when it compiles.
- **No mashup or two-deck view.** No stem separation, no acapella extraction.
- **No published accuracy number.** Keystone makes no accuracy claim, and specifically
  does not claim to beat Mixed In Key. Any figure would have to be measured on a public
  annotated set of real audio with the scoring method named, and that has not been done
  yet. When it is, the number and the method get published together or not at all.
  There is one measurement in the repository — [docs/ACCURACY.md](docs/ACCURACY.md),
  which reports how often Keystone's key agrees with Mixed In Key's own labels on one
  private 1,063-track library — and it is agreement between two programs on one DJ's
  collection, where neither is known to be right, so nothing in it is an accuracy figure
  or comparable with any published one.
- **The confidence scale is not calibrated yet, and you will see that.** The thresholds
  behind the key badge were fitted on synthetic audio and read far too low on real
  records, so on a real library most tracks currently show the relative pair rather than
  a single key. The answer underneath is usually sound; the number grading it is not.
  This is a known defect, not the honest-ambiguity feature working as intended.
- **No writing into rekordbox's own database.** XML only.
- **Nothing for material that is not broadly 4/4 and tempo-stable.** Live, acoustic,
  classical, swung and half-time tracks get a low-confidence result or an honest "could
  not analyse" — never a confident wrong answer.
- **No cloud, in any part of the app.** Including models: if something cannot be
  bundled, it does not ship.

## Install

**macOS**

1. Download the `.dmg` from [keystone.swendl.com](https://keystone.swendl.com) or the
   releases page.
2. Open it and drag **Keystone.app** to Applications.
3. If macOS says _"Keystone is damaged and can't be opened"_, run this once:

```bash
xattr -rd com.apple.quarantine /Applications/Keystone.app
```

4. Launch it from Applications.

> That warning appears because the app is not yet notarized with Apple. The command
> clears the quarantine flag macOS puts on downloaded files.

**Windows**

Download and run the installer from the releases page.

## First run

1. Click **ADD TRACKS** and pick a folder.
2. Let the analysis queue work through it — the sidebar shows progress and survives a
   restart.
3. Click a segment on the wheel to see what mixes with it.

## Build from source

You need Node 20+, Rust stable via [rustup](https://rustup.rs), and on macOS the Xcode
command line tools (`xcode-select --install`).

```bash
npm install
npm run tauri dev      # development, with hot reload
npm run tauri build    # production bundle
```

Keystone keeps its database in the platform app data directory:

- macOS: `~/Library/Application Support/app.keystone.desktop/keystone.db`
- Windows: `%APPDATA%\app.keystone.desktop\keystone.db`
- Linux: `~/.local/share/app.keystone.desktop/keystone.db`

## Releasing

Push a version tag and CI builds macOS and Windows and publishes the release:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## License

MIT. See [LICENSE](LICENSE).

Keystone is part of a set: [Stack](https://stack.swendl.com) for sample libraries,
Keystone for harmonic mixing, Capstone for audio correction.
