import type { KeyMode } from '../types';

/**
 * Key <-> Camelot mapping and the compatibility rules the wheel filter runs on.
 *
 * Everything here is a pure function of its arguments: no store reads, no DOM,
 * no settings. That is deliberate — the wheel, the badges, the table sort and
 * the tag writer all have to agree on these answers, and the only way to be
 * sure they do is to make them testable in isolation.
 *
 * The two mapping formulas are verified against Mixed In Key's own published
 * wheel: 8B = C major, 8A = A minor, +1 on the ring = up a perfect fifth.
 */

export type CamelotLetter = 'A' | 'B';

/** A Camelot code: 1-12 plus a letter, e.g. `8A`. */
export interface CamelotKey {
  /** 1-12. */
  number: number;
  letter: CamelotLetter;
}

/** Pitch class, 0 = C. */
export type PitchClass = number;

const SHARP_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
/**
 * The typographic flat, matching SPEC.md §3's `G♭m` and `analysis::camelot`'s
 * `FLAT_NAMES`.
 *
 * Not the ASCII `b`: these names are rendered, and the backend writes the same
 * sign into the key marker and the renamed filename. Two spellings of one key
 * would put `Gbm` in the table and `G♭m` in the file name of the same track.
 * The ASCII form exists only in the backend, for `TKEY`, which accepts three
 * characters from a fixed set and cannot carry this one.
 */
const FLAT_NAMES = ['C', 'D♭', 'D', 'E♭', 'E', 'F', 'G♭', 'G', 'A♭', 'A', 'B♭', 'B'];

/** All 24 codes, `1A` first then `1B`, ascending. Stable order for the wheel. */
export const CAMELOT_CODES: readonly string[] = Array.from({ length: 12 }, (_, i) =>
  i + 1,
).flatMap((n) => [`${n}A`, `${n}B`]);

/** True 1..12. Wraps are handled by `wrapNumber`, never by the caller. */
function wrapNumber(n: number): number {
  // ((n - 1) mod 12) + 1, written so negative n wraps too.
  return (((n - 1) % 12) + 12) % 12 + 1;
}

// ── Mapping ─────────────────────────────────────────────────────────────────

/**
 * Key -> Camelot. `root` is a pitch class with 0 = C; out-of-range roots are
 * wrapped rather than rejected, because the engine's key index is modular.
 */
export function toCamelot(root: PitchClass, mode: KeyMode): CamelotKey {
  const p = ((root % 12) + 12) % 12;
  const number =
    mode === 'major' ? ((7 * p + 7) % 12) + 1 : ((7 * p + 4) % 12) + 1;
  return { number, letter: mode === 'major' ? 'B' : 'A' };
}

/** Key -> Camelot code string. */
export function toCamelotCode(root: PitchClass, mode: KeyMode): string {
  const key = toCamelot(root, mode);
  return `${key.number}${key.letter}`;
}

/**
 * Camelot -> key. The inverse of `toCamelot`.
 *
 * 7 is its own inverse mod 12 (7*7 = 49 = 1), which is why both branches
 * multiply by 7 rather than by some other constant.
 */
export function fromCamelot(key: CamelotKey): { root: PitchClass; mode: KeyMode } {
  const n = wrapNumber(key.number);
  if (key.letter === 'B') {
    return { root: ((7 * (n - 8)) % 12 + 12) % 12, mode: 'major' };
  }
  return { root: ((7 * (n - 5)) % 12 + 12) % 12, mode: 'minor' };
}

/** Parse `8A`, `08a`, `12B`. Returns null for anything else. */
export function parseCamelot(code: string | null | undefined): CamelotKey | null {
  if (!code) return null;
  const m = /^0*(\d{1,2})\s*([ABab])$/.exec(code.trim());
  if (!m) return null;
  const number = Number(m[1]);
  if (number < 1 || number > 12) return null;
  return { number, letter: m[2].toUpperCase() as CamelotLetter };
}

/** `{ number: 8, letter: 'A' }` -> `8A`, or `08A` when `pad` is set. */
export function formatCamelot(key: CamelotKey, pad = false): string {
  const n = pad ? String(key.number).padStart(2, '0') : String(key.number);
  return `${n}${key.letter}`;
}

// ── Compatibility ───────────────────────────────────────────────────────────

/**
 * The wheel filter: the selected key plus its three harmonic neighbours.
 *
 * Same number / other letter is the relative major-minor switch; number -1 and
 * +1 are a perfect fifth down and up, wrapping 12 to 1. Mixed In Key's
 * documented advanced moves (+2 energy boost, -5 jaws, -3 pay attention) stay
 * out: they are deliberate effects, not "will mix", and putting them in the
 * default set makes the filter useless as a filter.
 *
 * Returned in a fixed order — self, relative, down a fifth, up a fifth — so the
 * wheel can draw them consistently.
 */
export function compatibleKeys(key: CamelotKey): CamelotKey[] {
  const other: CamelotLetter = key.letter === 'A' ? 'B' : 'A';
  return [
    { number: key.number, letter: key.letter },
    { number: key.number, letter: other },
    { number: wrapNumber(key.number - 1), letter: key.letter },
    { number: wrapNumber(key.number + 1), letter: key.letter },
  ];
}

/** `compatibleKeys` as code strings, for a query or a Set. */
export function compatibleCodes(code: string): string[] {
  const key = parseCamelot(code);
  if (!key) return [];
  return compatibleKeys(key).map((k) => formatCamelot(k));
}

/** Symmetric: `isCompatible(a, b) === isCompatible(b, a)` for all 24 x 24. */
export function isCompatible(a: string, b: string): boolean {
  const ka = parseCamelot(a);
  const kb = parseCamelot(b);
  if (!ka || !kb) return false;
  if (ka.letter === kb.letter) {
    const d = Math.abs(ka.number - kb.number);
    return d === 0 || d === 1 || d === 11; // 11 is the 12 <-> 1 wrap
  }
  return ka.number === kb.number;
}

/** The relative major or minor — the A/B switch across the ring. */
export function relativeKey(key: CamelotKey): CamelotKey {
  return { number: key.number, letter: key.letter === 'A' ? 'B' : 'A' };
}

/** Step around the ring, keeping the letter. `step` may be negative. */
export function transposeWheel(key: CamelotKey, step: number): CamelotKey {
  return { number: wrapNumber(key.number + step), letter: key.letter };
}

// ── Geometry ────────────────────────────────────────────────────────────────

/**
 * Centre angle of a wheel segment, in degrees clockwise from 12 o'clock.
 *
 * 1 sits at the top and the numbers run clockwise, which is how every Camelot
 * wheel a DJ has seen is drawn. Returned as a bearing rather than radians
 * because SVG transforms take degrees.
 */
export function camelotAngle(key: CamelotKey): number {
  return ((wrapNumber(key.number) - 1) * 30) % 360;
}

/** Which ring a key is drawn on: minor inside, major outside (SPEC.md §1). */
export function camelotRing(key: CamelotKey): 'inner' | 'outer' {
  return key.letter === 'A' ? 'inner' : 'outer';
}

// ── Naming ──────────────────────────────────────────────────────────────────

/** `Am`, `F#m`, `D♭` — for display. Tag values are composed in the backend. */
export function keyName(
  root: PitchClass,
  mode: KeyMode,
  notation: 'sharps' | 'flats' = 'sharps',
): string {
  const p = ((root % 12) + 12) % 12;
  const names = notation === 'flats' ? FLAT_NAMES : SHARP_NAMES;
  return mode === 'minor' ? `${names[p]}m` : names[p];
}

/** Camelot code -> note name, for the Key Notation setting's other two modes. */
export function camelotToKeyName(
  code: string,
  notation: 'sharps' | 'flats' = 'sharps',
): string | null {
  const key = parseCamelot(code);
  if (!key) return null;
  const { root, mode } = fromCamelot(key);
  return keyName(root, mode, notation);
}

/**
 * What the key cell shows.
 *
 * When the engine flagged the mode ambiguous it returns the relative pair
 * (`8A/8B`) rather than committing to a letter. Most residual key error is
 * mode, which is exactly the A/B letter, and a DJ would rather see
 * acknowledged ambiguity than a confident wrong answer.
 */
export function displayKey(
  code: string | null,
  opts: {
    notation: 'camelot' | 'sharps' | 'flats';
    pad?: boolean;
    modeAmbiguous?: boolean;
  },
): string {
  const key = parseCamelot(code);
  if (!key) return '';
  if (opts.notation !== 'camelot') {
    const name = camelotToKeyName(formatCamelot(key), opts.notation);
    if (!opts.modeAmbiguous) return name ?? '';
    const rel = camelotToKeyName(formatCamelot(relativeKey(key)), opts.notation);
    return rel ? `${name}/${rel}` : (name ?? '');
  }
  const self = formatCamelot(key, opts.pad);
  if (!opts.modeAmbiguous) return self;
  return `${self}/${formatCamelot(relativeKey(key), opts.pad)}`;
}
