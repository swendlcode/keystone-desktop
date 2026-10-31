import { camelotSwatchStyle } from '../../utils/camelotColors';
import { displayKey } from '../../utils/camelot';
import type { KeyNotation } from '../../types';

interface KeyBadgeProps {
  /** Camelot code from the track row, e.g. `8A`. Null renders nothing. */
  camelot: string | null;
  notation?: KeyNotation;
  /** `08A` rather than `8A` — the Display Options setting. */
  pad?: boolean;
  /** Show the relative pair (`8A/8B`) because the engine was unsure of mode. */
  modeAmbiguous?: boolean;
  className?: string;
  title?: string;
}

/**
 * The KEY RESULT cell, and the same object in the player readout.
 *
 * Its colour comes from the Camelot scale in index.css, which is the same
 * source the wheel segments read. That is the point: a DJ matches the badge to
 * the ring by colour, so a second palette here would quietly break the whole
 * mental model.
 *
 * An ambiguous mode is drawn in the A-ring colour with the pair as text. We do
 * not blend the two swatches — a colour between them is a third key as far as
 * the eye is concerned.
 */
export function KeyBadge({
  camelot,
  notation = 'camelot',
  pad = false,
  modeAmbiguous = false,
  className = '',
  title,
}: KeyBadgeProps) {
  if (!camelot) return null;
  const style = camelotSwatchStyle(camelot);
  if (!style) return null;
  const text = displayKey(camelot, { notation, pad, modeAmbiguous });

  return (
    <span
      className={`badge tnum ${modeAmbiguous ? 'italic' : ''} ${className}`}
      style={style}
      title={title ?? (modeAmbiguous ? 'Mode uncertain — relative pair shown' : undefined)}
    >
      {text}
    </span>
  );
}
