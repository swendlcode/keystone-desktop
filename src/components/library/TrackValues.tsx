import { PendingValue } from './PendingValue';
import { formatTempo } from '../../utils/formatters';
import type { TempoDecimals, Track } from '../../types';

/** Below this, the tempo is shown as approximate rather than as a reading. */
const TEMPO_UNCERTAIN_BELOW = 0.5;

/**
 * The TEMPO cell.
 *
 * Two decimals are asked for by a setting but licensed per track by the beat
 * grid's residual, which `formatTempo` enforces — so the setting is passed
 * through rather than branched on here.
 *
 * A variable-tempo track is prefixed `~` and dimmed. The engine plan is
 * explicit that live, rubato and half-time material gets a low-confidence
 * result rather than a confident wrong one, and this is where that lands.
 */
export function TempoValue({
  track,
  decimals,
}: {
  track: Track;
  decimals: TempoDecimals;
}) {
  if (track.tempoDisplay == null) return <PendingValue track={track} />;

  const text = formatTempo(track.tempoDisplay, decimals, track.tempoResidualMs);
  const uncertain =
    track.tempoVariable ||
    (track.tempoConfidence != null &&
      track.tempoConfidence < TEMPO_UNCERTAIN_BELOW);

  return (
    <span
      className={`mono tnum ${uncertain ? 'text-gray-400' : 'text-keystone-white'}`}
      title={
        track.tempoVariable
          ? 'Variable tempo'
          : uncertain
            ? 'Low confidence'
            : undefined
      }
    >
      {uncertain ? '~' : ''}
      {text}
    </span>
  );
}

/**
 * The ENERGY cell: the integer, with a ten-step gauge behind it.
 *
 * The gauge is there because 1-10 is a scale, and a bare digit in a column of
 * digits does not read as one. It is not a confidence bar — energy has no
 * per-track confidence, only a frozen model version.
 */
export function EnergyValue({ track }: { track: Track }) {
  if (track.energy == null) return <PendingValue track={track} />;

  return (
    <span className="inline-flex items-center gap-1.5" title={`Energy ${track.energy}`}>
      <span className="mono tnum w-3 text-right text-keystone-white">
        {track.energy}
      </span>
      <span className="flex h-2.5 items-end gap-px" aria-hidden>
        {Array.from({ length: 10 }, (_, i) => (
          <span
            key={i}
            style={{ height: `${30 + i * 7}%` }}
            className={`w-0.5 rounded-sm ${
              i < track.energy! ? 'bg-keystone-lime' : 'bg-gray-700'
            }`}
          />
        ))}
      </span>
    </span>
  );
}

/**
 * How many of the eight cue slots are filled.
 *
 * `tracks` does not carry this yet, so the read is defensive: `TrackSort`
 * already offers `cuePointCount`, which means the backend sorts by a number
 * the read model does not return. Until the column lands this cell says
 * nothing rather than guessing — counting per row would be one query per
 * visible row, and inferring it from `analyzedAt` would be an invented number
 * in a column a DJ exports from.
 */
export function cueCountOf(track: Track): number | null {
  const v = (track as Track & { cuePointCount?: number | null }).cuePointCount;
  return typeof v === 'number' ? v : null;
}

export function CueCountValue({ track }: { track: Track }) {
  const count = cueCountOf(track);
  if (count == null) return <PendingValue track={track} />;

  return (
    <span
      className={`mono tnum ${count > 0 ? 'text-keystone-white' : 'text-gray-600'}`}
      title={`${count} of 8 cue points`}
    >
      {count}
    </span>
  );
}
