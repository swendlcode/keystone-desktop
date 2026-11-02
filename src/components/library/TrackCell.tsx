import { MusicSquare } from '../ui/icons';
import { Rating } from '../ui/Rating';
import { KeyResult } from './KeyResult';
import { BpmPicker } from './BpmPicker';
import { EnergyPicker } from './EnergyPicker';
import { CueCountValue } from './TrackValues';
import { useCoverArt } from './useCoverArt';
import type { KeyNotation, TempoDecimals, Track, TrackColumn } from '../../types';
import {
  formatBitrate,
  formatCount,
  formatDate,
  formatDb,
  truncatePath,
} from '../../utils/formatters';

export interface CellFormat {
  notation: KeyNotation;
  /** `08A` rather than `8A` — the Display Options setting. */
  pad: boolean;
  decimals: TempoDecimals;
}

interface TrackCellProps extends CellFormat {
  column: TrackColumn;
  track: Track;
  onRate: (trackId: string, rating: number) => void;
}

/**
 * The formatting settings arrive as three primitives rather than as the
 * `Settings` object they came from. `useEffectiveSettings` spreads the saved
 * values over the unsaved draft on every call, so the object is a new identity
 * each render and passing it down would defeat `TrackRow`'s memo for every
 * mounted row on every scroll frame.
 */

/**
 * One cell. A switch rather than a per-column component map, because most
 * branches are a formatter call and the few that are not are already their own
 * component.
 */
export function TrackCell({
  column,
  track,
  notation,
  pad,
  decimals,
  onRate,
}: TrackCellProps) {
  switch (column) {
    case 'coverArt':
      return <CoverArt track={track} />;

    case 'keyResult':
      return <KeyResult track={track} notation={notation} pad={pad} />;

    case 'tempo':
      // Editable in place, for the same reason energy is: an octave error is
      // visible in the column and was only correctable from the player bar,
      // which meant playing a track to fix a number already on screen.
      return <BpmPicker track={track} decimals={decimals} />;

    case 'energy':
      // Editable in place, like the rating beside it. Energy is the reading
      // most likely to be argued with — the scale's ordering was never
      // validated against a human ear — and making it correctable only for
      // the track currently playing meant auditioning a track to fix a
      // number you could already see was wrong.
      return <EnergyPicker track={track} />;

    case 'cuePoints':
      return <CueCountValue track={track} />;

    case 'rating':
      return (
        <Rating
          value={track.rating}
          onChange={(rating) => onRate(track.id, rating)}
        />
      );

    case 'dateAdded':
      return <Mono>{formatDate(track.dateAdded)}</Mono>;

    case 'path':
      return (
        <Mono title={track.path}>{truncatePath(track.path, 40)}</Mono>
      );

    case 'year':
      return <Mono>{track.year ?? ''}</Mono>;

    case 'bitrate':
      return <Mono>{formatBitrate(track.bitrateKbps)}</Mono>;

    // SPEC.md's VOLUME column. The true peak, not the integrated loudness:
    // every modern master sits near the same LUFS, so the column would be the
    // same number on every row and tell the user nothing.
    case 'volume':
      return <Mono>{formatDb(track.truePeakDb ?? track.peakDb)}</Mono>;

    case 'clippedPeaks':
      return track.clippedPeaks == null ? null : (
        <span
          className={`mono tnum ${track.clippedPeaks > 0 ? 'text-amber-400' : 'text-gray-500'}`}
          title={
            track.clippedPeaks > 0
              ? 'Clipped samples — Capstone can repair this'
              : undefined
          }
        >
          {formatCount(track.clippedPeaks)}
        </span>
      );

    case 'artist':
      return <Text value={track.artist} />;
    case 'title':
      return <Text value={track.title ?? track.filename} />;
    case 'album':
      return <Text value={track.album} />;
    case 'genre':
      return <Text value={track.genre} />;
    case 'grouping':
      return <Text value={track.grouping} />;
    case 'comment':
      return <Text value={track.comment} />;
  }
}

function Text({ value }: { value: string | null }) {
  if (!value) return <span className="text-gray-600">&mdash;</span>;
  return (
    <span className="truncate text-keystone-white" title={value}>
      {value}
    </span>
  );
}

function Mono({
  children,
  title,
}: {
  children: React.ReactNode;
  title?: string;
}) {
  return (
    <span className="mono tnum truncate text-gray-300" title={title}>
      {children}
    </span>
  );
}

/**
 * The thumbnail.
 *
 * `hasCoverArt` is on the row, so a track with no art never starts a fetch and
 * draws the glyph immediately — which is most of a DJ library and the whole
 * reason this column is cheap enough to have on by default.
 */
function CoverArt({ track }: { track: Track }) {
  const url = useCoverArt(track);

  if (!url) {
    return (
      <span className="flex h-6 w-6 items-center justify-center rounded bg-gray-800 text-gray-600">
        <MusicSquare size={12} color="currentColor" variant="Linear" />
      </span>
    );
  }

  return (
    <img
      src={url}
      alt=""
      loading="lazy"
      className="h-6 w-6 rounded object-cover"
    />
  );
}
