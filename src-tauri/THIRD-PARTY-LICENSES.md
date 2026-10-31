# Third-party licences

Keystone ships under **MIT**. Only MIT, Apache-2.0, BSD and MPL-2.0 dependencies
are permitted. No GPL, no AGPL, no CC-BY-NC, no unlicensed code — in the
analysis path or anywhere else.

## Obligations that are not automatic

### symphonia — MPL-2.0

Not on the MIT/Apache/BSD list, and accepted deliberately. MPL-2.0 is
**file-level** copyleft: the obligation attaches to symphonia's own files if we
modify and redistribute them, and does not reach Keystone's MIT code linking
against it.

Two rules follow, and both are load-bearing:

- **Never patch symphonia's sources in-tree.** Fork-and-patch converts a
  no-obligation dependency into a redistribution obligation.
- Keep it listed here.

### ort / ONNX Runtime — MIT/Apache-2.0

Pinned to an exact version with no caret, because `ort` is a release candidate
whose ABI is tied to one ONNX Runtime build. On Windows the runtime DLLs ship
beside the binary: a stale system `onnxruntime.dll` would otherwise shadow the
bundled one.

### S-KEY model weights

The weights sit inside an MIT repository but carry **no separate licence file**.
Treating the repository licence as covering them is a reasonable reading and an
inference rather than a written weight licence. This is the single legal
dependency the key plan rests on, and the open action is written confirmation
from the authors.

### Beat This! — MIT for code *and* weights

The only permissively licensed path to a neural beat and downbeat tracker, which
is why it is the only one considered. Behind a cargo feature, not in the default
build.

### Evaluation data — GiantSteps+, CC BY-SA 4.0

Evaluating against CC BY-SA data **does not** infect MIT code. **Deriving and
shipping** key profiles or energy constants from it **does** raise ShareAlike,
so anything fitted on it stays in the dev-only harness. Shipped profiles come
from published papers or from a corpus we own.

### M-DJCUE cue annotations — GPL-3.0

Usable as a **local** dev and test set only. Must not be redistributed, and
nothing derived from it — fitted thresholds baked into the binary — may ship.

## Rejected, and what replaced each

| Rejected | Licence | Replaced by |
|---|---|---|
| libKeyFinder, `libkeyfinder-sys` | GPL-3.0(-or-later) | Hand-rolled HPCP on `rustfft`. Also means Sha'ath's numeric profile coefficients are off-limits: they exist in numeric form only inside GPL source |
| Essentia, `essentia-rs` | AGPL-3.0 / GPL-3.0-or-later | Hand-rolled |
| aubio, `aubio-rs`, `aubio-sys` | GPL-3.0(-or-later) | Hand-rolled onset and tempo |
| `bliss-audio` | GPL-3.0-only | n/a — it wraps aubio |
| madmom model files | code BSD-2, **models CC BY-NC-SA 4.0** | Beat This! |
| hendriks73/key-cnn | AGPL-3.0 | S-KEY |
| `chord_detector` | GPL-3.0 | Not needed in v1 |
| angelfaraldo/edmkey | **no licence declared** = all rights reserved | bgate implemented from the papers, not the code |
| Mixxx | GPL-2.0-or-later | Read to understand *what* Serato and Traktor do; **never port a line**. It is the best reference in open source and therefore the most likely way this project acquires a copyleft problem |
| qm-dsp / QM Vamp, BTrack | GPL family | Hand-rolled |
| `rbox`, `prolink-rekordbox` | GPL-3.0-only | rekordbox XML generation |
| crate-digger | EPL-2.0, and Java | Documentation only |
| `triseratops` (crates.io) | **every published version AGPL-3.0-or-later** | Forbidden. The repository was relicensed MPL-2.0 and never republished, so when Serato lands in v1.1 it is a pinned git revision with its LICENSE vendored and the SPDX header relied upon recorded here |
