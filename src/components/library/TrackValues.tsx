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
 * A variable-tempo or low-confidence reading is dimmed. It used to also be
 * prefixed `~`, which put a character in a monospaced numeric column that is
 * not part of the number — it broke the column's alignment and read as part
 * of the value when copied or glanced at. The dimming carries the same
 * meaning and the tooltip says which of the two it is.
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
 * The count arrives as a prop, fetched once per loaded page by
 * `useCueCounts`, rather than being read off the track. `tracks` does not
 * carry it — `TrackSort` offers `cuePointCount` and the read model does not
 * return it — and the earlier version looked for a field that was never
 * there, so every row showed `--` while the player bar showed eight markers
 * for the same track. A column that says "none" everywhere is worse than one
 * that says nothing.
 *
 * `null` is still "not known yet", which is what the first frame of a new
 * page shows. Zero is a real zero and is dimmed rather than hidden, because
 * "this track has no cues" is the thing a DJ is scanning the column for.
 */
export function CueCountValue({
  track,
  count,
}: {
  track: Track;
  count: number | null;
}) {
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
