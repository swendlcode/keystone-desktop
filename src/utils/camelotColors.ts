import { parseCamelot, formatCamelot, type CamelotKey } from './camelot';

/**
 * Accessors for the 24 Camelot swatches.
 *
 * The values themselves live in `src/index.css` as `--camelot-1a` ...
 * `--camelot-12b`. Nothing here holds a hex, and no component should: the wheel
 * segments and the KEY RESULT badges have to be the same colour or the badge
 * stops meaning anything, and the only way to guarantee that is one source.
 */

/** `rgb(var(--camelot-8a))` for a code, or null when the code is unparseable. */
export function camelotColor(code: string | null | undefined): string | null {
  const key = parseCamelot(code);
  if (!key) return null;
  return `rgb(${camelotVar(key)})`;
}

/**
 * The same colour at an alpha, for a wheel segment's unselected fill or a
 * badge's tinted background.
 */
export function camelotColorAlpha(
  code: string | null | undefined,
  alpha: number,
): string | null {
  const key = parseCamelot(code);
  if (!key) return null;
  return `rgb(${camelotVar(key)} / ${alpha})`;
}

/** The bare `var(--camelot-8a)` reference, for composing into a gradient. */
export function camelotVar(key: CamelotKey): string {
  return `var(--camelot-${formatCamelot(key).toLowerCase()})`;
}

/**
 * Readable foreground on a filled swatch.
 *
 * Two tokens, not twenty-four: the whole A ring is dark enough to take the
 * light foreground and the whole B ring is not, which is a property of how the
 * scale is built (see the construction note in index.css) rather than a
 * per-colour judgement.
 */
export function camelotForeground(code: string | null | undefined): string | null {
  const key = parseCamelot(code);
  if (!key) return null;
  return key.letter === 'A' ? 'var(--camelot-fg-a)' : 'var(--camelot-fg-b)';
}

/** Inline style for a filled Camelot swatch — background plus its foreground. */
export function camelotSwatchStyle(
  code: string | null | undefined,
): { background: string; color: string } | null {
  const bg = camelotColor(code);
  const fg = camelotForeground(code);
  if (!bg || !fg) return null;
  return { background: bg, color: fg };
}
