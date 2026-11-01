import type { Waveform, WaveformLevel } from '../../types';

/**
 * Canvas drawing for the player's waveform, kept out of the component.
 *
 * None of this touches React, which is the point: the draw runs on every
 * `timeupdate` while a zoomed-in view follows the playhead, and anything in
 * here that caused a render would cost a reconcile 4 times a second for the
 * length of a track.
 */

export interface WaveWindow {
  /** Seconds. */
  startS: number;
  endS: number;
}

/** How many seconds fit across the canvas at a zoom step. */
export function waveSpan(durationS: number, zoom: number): number {
  if (!(durationS > 0)) return 1;
  return Math.min(durationS, durationS / 2 ** Math.max(0, zoom));
}

/**
 * The visible span at a zoom step, and where it sits.
 *
 * Zoom 0 is the whole track and each step halves the span. Clamping slides the
 * window rather than truncating it, so zooming in near the end of a track
 * still fills the canvas instead of leaving dead space.
 *
 * `centreS` is where the user has scrolled to; null means follow the playhead.
 * The two are separate because placing a cue accurately means looking at a bar
 * that is not the one playing — with the window pinned to the playhead the
 * only way to see another part of the track was to seek to it, which moves the
 * thing you were lining the cue up against.
 */
export function waveWindow(
  durationS: number,
  zoom: number,
  playheadS: number,
  centreS: number | null = null,
): WaveWindow {
  if (!(durationS > 0)) return { startS: 0, endS: 1 };
  const span = waveSpan(durationS, zoom);
  let startS = (centreS ?? playheadS) - span / 2;
  if (startS < 0) startS = 0;
  if (startS + span > durationS) startS = durationS - span;
  return { startS, endS: startS + span };
}

/**
 * The coarsest level that still has a bucket for every pixel.
 *
 * Coarsest-that-suffices rather than finest-available: the finest level of a
 * ten-minute track is millions of buckets, and reducing 400 of them into one
 * pixel column costs the same as reducing 4 and looks identical.
 */
export function pickLevel(
  wave: Waveform,
  window: WaveWindow,
  widthPx: number,
): number {
  const spanS = Math.max(window.endS - window.startS, 1e-6);
  for (let i = 0; i < wave.levels.length; i += 1) {
    const buckets = (spanS * wave.sampleRate) / wave.levels[i].samplesPerBucket;
    if (buckets >= widthPx) return i;
  }
  return wave.levels.length - 1;
}

interface DrawOptions {
  wave: Waveform;
  window: WaveWindow;
  playheadS: number;
  /** CSS pixels; the caller has already applied the device pixel ratio. */
  width: number;
  height: number;
  playedColor: string;
  unplayedColor: string;
  centreColor: string;
}

/** Repaints the whole canvas. There is no partial-invalidation path. */
export function drawWaveform(
  ctx: CanvasRenderingContext2D,
  opts: DrawOptions,
): void {
  const { wave, window, width, height, playheadS } = opts;
  ctx.clearRect(0, 0, width, height);

  const level = wave.levels[pickLevel(wave, window, width)];
  if (!level) return;

  const mid = height / 2;
  const spanS = Math.max(window.endS - window.startS, 1e-6);
  const bucketsPerSecond = wave.sampleRate / level.samplesPerBucket;

  // One pass per pixel column. Columns are drawn as 1px-wide rects rather
  // than a stroked path: a 0.5px-offset stroke on a DPR-scaled context is
  // where waveforms acquire their fuzzy look.
  for (let x = 0; x < width; x += 1) {
    const t0 = window.startS + (x / width) * spanS;
    const t1 = window.startS + ((x + 1) / width) * spanS;
    const peak = bucketPeak(level, t0 * bucketsPerSecond, t1 * bucketsPerSecond);
    if (!peak) continue;

    const top = mid - peak.max * mid;
    const bottom = mid - peak.min * mid;
    ctx.fillStyle = t1 <= playheadS ? opts.playedColor : opts.unplayedColor;
    ctx.fillRect(x, top, 1, Math.max(bottom - top, 1));
  }

  ctx.fillStyle = opts.centreColor;
  ctx.fillRect(0, mid, width, 1);
}

/** min/max across a fractional bucket range, or null when it is out of range. */
function bucketPeak(
  level: WaveformLevel,
  fromBucket: number,
  toBucket: number,
): { min: number; max: number } | null {
  const first = Math.max(0, Math.floor(fromBucket));
  const last = Math.min(level.max.length - 1, Math.max(first, Math.ceil(toBucket) - 1));
  if (first > last) return null;

  let min = 0;
  let max = 0;
  for (let i = first; i <= last; i += 1) {
    if (level.min[i] < min) min = level.min[i];
    if (level.max[i] > max) max = level.max[i];
  }
  return { min, max };
}

/** Theme colours, read from the document rather than hardcoded. */
export function waveColors(): Pick<
  DrawOptions,
  'playedColor' | 'unplayedColor' | 'centreColor'
> {
  const style = getComputedStyle(document.documentElement);
  const rgb = (token: string) => `rgb(${style.getPropertyValue(token).trim()})`;
  return {
    playedColor: rgb('--keystone-lime'),
    unplayedColor: rgb('--gray-600'),
    centreColor: rgb('--gray-700'),
  };
}
