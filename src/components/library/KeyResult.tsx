import { KeyBadge } from '../ui/KeyBadge';
import { PendingValue } from './PendingValue';
import { camelotSwatchStyle } from '../../utils/camelotColors';
import { displayKey, formatCamelot, parseCamelot, relativeKey } from '../../utils/camelot';
import type { KeyNotation, Track } from '../../types';

/**
 * Below this top-probability, the key is shown as a proposal rather than a
 * result.
 *
 * The engine returns a confidence for exactly this reason and the UI is where
 * that promise is kept: most residual key error is mode, which is the A/B
 * letter a DJ reads off the badge, so a confident-looking wrong letter costs
 * more than an acknowledged doubtful one. The threshold is a single named
 * constant so it can be moved once the eval harness has a distribution to
 * look at, instead of being guessed at four call sites.
 */
export const KEY_UNCERTAIN_BELOW = 0.55;

export function isKeyUncertain(track: Track): boolean {
  if (track.keyModeAmbiguous) return true;
  return track.keyConfidence != null && track.keyConfidence < KEY_UNCERTAIN_BELOW;
}

interface KeyResultProps {
  track: Track;
  notation: KeyNotation;
  pad: boolean;
}

/**
 * The KEY RESULT cell, and the same object in the player readout.
 *
 * Every state is a filled swatch, never an outline. The colour is the whole
 * point of this cell — it is what the eye matches against the wheel — and an
 * outlined chip shows a thread of it at 1px while the confident rows beside it
 * show a solid block, so the uncertain rows read as unfinished rather than as
 * uncertain.
 *
 * Uncertainty is carried by the shape instead:
 *
 * - Mode ambiguous, which is most of the residual error, draws *both*
 *   candidates as two filled halves of one chip. Two swatches is a plainer
 *   statement than one swatch and a slash.
 * - Low confidence on a single candidate keeps its swatch and adds a dimmed
 *   `?` inside the chip, where it reads as a hedge on that key rather than as
 *   a question about the notation.
 */
export function KeyResult({ track, notation, pad }: KeyResultProps) {
  if (!track.camelot) return <PendingValue track={track} />;

  if (track.keyModeAmbiguous) {
    const key = parseCamelot(track.camelot);
    if (key) {
      const relative = formatCamelot(relativeKey(key));
      return (
        <span
          className="badge-split tnum"
          title="Mode uncertain — both candidates shown"
        >
          <Half code={track.camelot} notation={notation} pad={pad} />
          <Half code={relative} notation={notation} pad={pad} />
        </span>
      );
    }
  }

  if (!isKeyUncertain(track)) {
    return <KeyBadge camelot={track.camelot} notation={notation} pad={pad} />;
  }

  const style = camelotSwatchStyle(track.camelot);

  return (
    <span className="badge tnum !gap-0.5" style={style ?? undefined} title="Low confidence">
      {displayKey(track.camelot, { notation, pad })}
      <span className="opacity-60">?</span>
    </span>
  );
}

/** One candidate: its own swatch, its own text, half of one chip. */
function Half({
  code,
  notation,
  pad,
}: {
  code: string;
  notation: KeyNotation;
  pad: boolean;
}) {
  const style = camelotSwatchStyle(code);
  return (
    <span style={style ?? undefined}>{displayKey(code, { notation, pad })}</span>
  );
}
