#!/usr/bin/env node
/**
 * Generates the 24 Camelot swatches in src/index.css.
 *
 *   node scripts/camelot-scale.mjs            print the CSS block
 *   node scripts/camelot-scale.mjs --verify   check index.css matches, and
 *                                             that every invariant still holds
 *
 * The scale is generated rather than chosen because three properties have to
 * hold simultaneously and cannot be held by eye:
 *
 *   - perceptual hue 30 deg apart, so adjacent Camelot numbers look adjacent
 *   - constant OKLab lightness per ring, so A/B reads as a lightness step and
 *     ONE foreground token per ring is enough
 *   - chroma at the sRGB gamut limit per hue, so the wheel is as vivid as the
 *     display allows
 *
 * The scale this replaced was mixed from three channel values by hand. It
 * missed all three: hue steps measured 10-56 deg, 8A was lighter than 2B, and
 * nine of 24 swatches put their label below 4.5:1 (8A at 1.95:1).
 *
 * --verify is the regression test. There is no CSS test runner here, so run it
 * from the lint step or by hand after touching the scale.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/** 1A sits at 250 deg to keep the wheel's existing orientation. */
const HUE_START = 250;
const HUE_STEP = 30;
/** Minor ring: dark enough that every hue takes the light foreground. */
const L_A = 0.53;
/** Major ring: light enough that every hue takes the dark foreground. */
const L_B = 0.72;
const FG_A = [247, 247, 247];
const FG_B = [11, 13, 10];
/** WCAG AA for the 9.5-10.5px labels the wheel draws. */
const MIN_CONTRAST = 4.5;

const srgb = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
const linear = (c) => {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
};

/** OKLCH -> sRGB, Ottosson's published matrices. May fall outside 0..1. */
function oklchToRgb(L, C, hDeg) {
  const h = (hDeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    srgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ];
}

function oklab(r, g, b) {
  const R = linear(r), G = linear(g), B = linear(b);
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}
const hueOf = (r, g, b) => {
  const { a, b: bb } = oklab(r, g, b);
  return ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360;
};

function contrast(c1, c2) {
  const l1 = 0.2126 * linear(c1[0]) + 0.7152 * linear(c1[1]) + 0.0722 * linear(c1[2]);
  const l2 = 0.2126 * linear(c2[0]) + 0.7152 * linear(c2[1]) + 0.0722 * linear(c2[2]);
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

const inGamut = (v) => v.every((c) => c >= -0.0005 && c <= 1.0005);
const quantise = (v) => v.map((c) => Math.max(0, Math.min(255, Math.round(c * 255))));

/** Largest chroma that keeps this hue inside sRGB at this lightness. */
function gamutChroma(L, h) {
  let lo = 0, hi = 0.45;
  for (let i = 0; i < 60; i++) {
    const mid = (lo + hi) / 2;
    if (inGamut(oklchToRgb(L, mid, h))) lo = mid;
    else hi = mid;
  }
  return lo;
}

function build() {
  const rows = [];
  for (let i = 0; i < 12; i++) {
    const h = (HUE_START + i * HUE_STEP) % 360;
    rows.push({
      n: i + 1,
      h,
      a: quantise(oklchToRgb(L_A, gamutChroma(L_A, h), h)),
      b: quantise(oklchToRgb(L_B, gamutChroma(L_B, h), h)),
    });
  }
  return rows;
}

function cssBlock(rows) {
  return rows
    .map((r) =>
      `  --camelot-${r.n}a: ${r.a.join(' ')};`.padEnd(32) +
      ` --camelot-${r.n}b: ${r.b.join(' ')};`,
    )
    .join('\n');
}

/** Returns a list of human-readable failures; empty means the scale is sound. */
function checkInvariants(rows) {
  const bad = [];

  for (const r of rows) {
    const ca = contrast(r.a, FG_A);
    const cb = contrast(r.b, FG_B);
    if (ca < MIN_CONTRAST) bad.push(`${r.n}A label contrast ${ca.toFixed(2)}:1`);
    if (cb < MIN_CONTRAST) bad.push(`${r.n}B label contrast ${cb.toFixed(2)}:1`);
  }

  // Invariant 2: every B is lighter than every A, not merely its own pair.
  const maxA = Math.max(...rows.map((r) => oklab(...r.a).L));
  const minB = Math.min(...rows.map((r) => oklab(...r.b).L));
  if (minB <= maxA) {
    bad.push(`rings overlap in lightness: darkest B ${minB.toFixed(3)} <= lightest A ${maxA.toFixed(3)}`);
  }

  // Invariant 1: neighbours a real 30 deg apart, allowing for quantisation.
  for (let i = 0; i < 12; i++) {
    const cur = hueOf(...rows[i].a);
    const next = hueOf(...rows[(i + 1) % 12].a);
    const gap = (((next - cur) % 360) + 360) % 360;
    if (Math.abs(gap - HUE_STEP) > 3) {
      bad.push(`hue gap ${rows[i].n}A->${rows[(i + 1) % 12].n}A is ${gap.toFixed(0)} deg`);
    }
  }

  return bad;
}

const rows = build();
const failures = checkInvariants(rows);

if (process.argv.includes('--verify')) {
  const css = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'index.css'),
    'utf8',
  );
  const missing = rows.flatMap((r) => {
    const out = [];
    if (!css.includes(`--camelot-${r.n}a: ${r.a.join(' ')};`)) out.push(`${r.n}a`);
    if (!css.includes(`--camelot-${r.n}b: ${r.b.join(' ')};`)) out.push(`${r.n}b`);
    return out;
  });
  if (missing.length) failures.push(`index.css out of date for: ${missing.join(', ')}`);

  if (failures.length) {
    console.error('camelot scale FAILED:');
    for (const f of failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  const worstA = Math.min(...rows.map((r) => contrast(r.a, FG_A)));
  const worstB = Math.min(...rows.map((r) => contrast(r.b, FG_B)));
  console.log(
    `camelot scale OK - 24 swatches, 30 deg apart, worst label contrast ` +
      `${worstA.toFixed(2)}:1 (A) / ${worstB.toFixed(2)}:1 (B)`,
  );
} else {
  if (failures.length) {
    console.error('refusing to print a scale that fails its own invariants:');
    for (const f of failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log(cssBlock(rows));
}
