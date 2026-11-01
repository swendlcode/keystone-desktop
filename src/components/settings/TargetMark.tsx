import type { ExportTarget } from '../../types';

/**
 * A small mark beside each export target, so the row is scannable.
 *
 * Deliberately a lettered badge in each product's own colour rather than a
 * copy of its logo. Reproducing someone else's trademark means shipping it
 * accurately — an approximation drawn from memory is both a worse mark and a
 * worse citation than not drawing one. A coloured initial is unambiguous in a
 * list of three, next to the name in full, and claims nothing it is not.
 *
 * The colours are each brand's own, which is the part that does the work: a
 * DJ recognises Serato's red and Traktor's blue without reading the letter.
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
  return (
    <span
      aria-hidden
      style={{ background: mark.bg, color: mark.fg }}
      className={`flex h-5 w-5 shrink-0 items-center justify-center rounded text-[11px] font-bold leading-none transition-opacity ${
        dimmed ? 'opacity-35' : ''
      }`}
    >
      {mark.letter}
    </span>
  );
}
