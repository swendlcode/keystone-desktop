import { KeyBadge } from '../ui/KeyBadge';
import { PendingValue } from './PendingValue';
import { camelotColor, camelotColorAlpha } from '../../utils/camelotColors';
import { displayKey } from '../../utils/camelot';
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
 * A confident key is a filled swatch from the Camelot scale, which is what
 * makes it match the wheel. An uncertain one is the same colour as an outline
 * with a trailing `?` — same hue, so the wheel still reads, but it does not
 * look like an answer.
 */
export function KeyResult({ track, notation, pad }: KeyResultProps) {
  if (!track.camelot) return <PendingValue track={track} />;

  if (!isKeyUncertain(track)) {
    return (
      <KeyBadge camelot={track.camelot} notation={notation} pad={pad} />
    );
  }

  const text = displayKey(track.camelot, {
    notation,
    pad,
    modeAmbiguous: track.keyModeAmbiguous,
  });

  return (
    <span
      className="badge tnum"
      style={{
        color: camelotColor(track.camelot) ?? undefined,
        boxShadow: `inset 0 0 0 1px ${camelotColorAlpha(track.camelot, 0.7) ?? 'transparent'}`,
        background: camelotColorAlpha(track.camelot, 0.1) ?? undefined,
      }}
      title={
        track.keyModeAmbiguous
          ? 'Mode uncertain — relative pair shown'
          : 'Low confidence'
      }
    >
      {text}?
    </span>
  );
}
