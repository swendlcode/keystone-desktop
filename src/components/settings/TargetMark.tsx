import { BRAND_MARKS } from '../../assets/brands';
import { TARGET_LABEL } from './useExportTargets';
import type { ExportTarget } from '../../types';

/**
 * A small mark beside each export target, so the row is scannable.
 *
 * Uses the product's real logo when one has been dropped into
 * `src/assets/brands/` — see the README there — and a lettered badge in the
 * brand's own colour when it has not.
 *
 * The fallback is not a placeholder to be embarrassed about. These are other
 * companies' trademarks: reproducing one means reproducing it accurately,
 * and a logo redrawn from memory is both a worse mark and a worse citation
 * than no logo. The three vector sources that turned up are aggregator sites
 * whose own terms forbid copying, so the assets have to come from the
 * official brand resources and cannot be committed here. A clean checkout
 * therefore has to build and look finished without them, and the colour does
 * most of the recognising anyway — a DJ reads Serato's red before the letter.
 */
const MARKS: Record<ExportTarget, { letter: string; bg: string; fg: string }> = {
  rekordbox: { letter: 'R', bg: '#E3242B', fg: '#ffffff' },
  traktor: { letter: 'T', bg: '#00AEEF', fg: '#06212b' },
  serato: { letter: 'S', bg: '#F0383B', fg: '#ffffff' },
};

export function TargetMark({
  target,
  dimmed = false,
}: {
  target: ExportTarget;
  /** A target that cannot be used reads as unavailable rather than absent. */
  dimmed?: boolean;
}) {
  const mark = MARKS[target];
  const logo = BRAND_MARKS[target];
  const dim = dimmed ? 'opacity-35' : '';

  if (logo) {
    return (
      <img
        src={logo}
        alt={TARGET_LABEL[target]}
        // `object-contain` on a square box: the three marks have different
        // aspect ratios and a row of logos that do not share a baseline
        // reads as broken alignment rather than as three logos.
        className={`h-5 w-5 shrink-0 object-contain transition-opacity ${dim}`}
      />
    );
  }

  return (
    <span
      aria-hidden
      style={{ background: mark.bg, color: mark.fg }}
      className={`flex h-5 w-5 shrink-0 items-center justify-center rounded text-[11px] font-bold leading-none transition-opacity ${dim}`}
    >
      {mark.letter}
    </span>
  );
}
