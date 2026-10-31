import type { TempoDecimals } from '../types';

/** `02:26` and, past an hour, `1:02:26`. SPEC.md's elapsed / total readout. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '00:00';
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, '0');
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function formatDurationMs(ms: number | null): string {
  return ms == null ? '' : formatTime(ms / 1000);
}

/**
 * Tempo, at the user's decimal setting.
 *
 * Two decimals are *requested* by a setting but *licensed* by the beat-grid
 * residual: at 100 fps the tempogram's own BPM step is 2.66 at 128 BPM, so the
 * second decimal only means anything when it came from the least-squares grid
 * fit. Pass `residualMs` and anything over 15 ms silently drops to one place
 * rather than printing a digit the engine did not measure.
 */
export function formatTempo(
  bpm: number | null,
  decimals: TempoDecimals,
  residualMs?: number | null,
): string {
  if (bpm == null) return '';
  let places: number = decimals;
  if (places === 2 && (residualMs == null || residualMs >= 15)) places = 1;
  return bpm.toFixed(places);
}

/** `+12` / `-7` cents, with the sign always shown — 0 is a result too. */
export function formatCents(cents: number | null): string {
  if (cents == null) return '';
  const rounded = Math.round(cents);
  return `${rounded >= 0 ? '+' : ''}${rounded}`;
}

export function formatEnergy(energy: number | null): string {
  return energy == null ? '' : String(energy);
}

/** `320 kbps`, blank when unknown. */
export function formatBitrate(kbps: number | null): string {
  return kbps == null ? '' : `${kbps} kbps`;
}

export function formatCount(n: number): string {
  return n.toLocaleString();
}

/** Unix seconds -> a short local date for the DATE ADDED column. */
export function formatDate(unixSeconds: number | null): string {
  if (!unixSeconds) return '';
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
  });
}

export function formatDb(db: number | null): string {
  return db == null ? '' : `${db.toFixed(1)} dB`;
}

/**
 * Shorten a long path for a cell, keeping the end.
 *
 * The tail is what identifies a file; the head is the part the user already
 * knows because they added the folder.
 */
export function truncatePath(path: string, max = 48): string {
  if (path.length <= max) return path;
  return `...${path.slice(path.length - (max - 3))}`;
}

/** `ARTIST - TITLE`, falling back to the filename when tags are missing. */
export function trackLabel(t: {
  artist: string | null;
  title: string | null;
  filename: string;
}): string {
  if (t.artist && t.title) return `${t.artist} - ${t.title}`;
  return t.title ?? t.artist ?? t.filename;
}
