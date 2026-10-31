import type { Track } from '../../types';

/**
 * What an analysis-derived cell shows before there is an answer.
 *
 * Deliberately not a spinner and not a zero. A `0` in the ENERGY column and a
 * `0` from the detector are indistinguishable once they are both in a table,
 * and a spinner per row in a 20,000-row virtualized table is a thousand
 * animations the user cannot read anyway.
 */
export function PendingValue({ track }: { track: Track }) {
  if (track.indexStatus === 'error') {
    return (
      <span
        className="mono text-amber-400/70"
        title={track.errorMessage ?? 'Could not analyse'}
      >
        !
      </span>
    );
  }

  const analysing = track.indexStatus === 'pending' || track.analyzedAt == null;
  return (
    <span
      className={`mono text-gray-600 ${analysing ? 'animate-pulse' : ''}`}
      title={analysing ? 'Not analysed yet' : undefined}
    >
      &ndash;&ndash;
    </span>
  );
}

/** True when the track has no analysis to show in any derived column. */
export function isUnanalysed(track: Track): boolean {
  return track.analyzedAt == null;
}
