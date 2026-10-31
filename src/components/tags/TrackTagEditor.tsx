import { CloseCircle } from '../ui/icons';
import { KeyBadge } from '../ui/KeyBadge';
import { ScrollArea } from '../ui/ScrollArea';
import { CoverArtPicker } from './CoverArtPicker';
import { TagCell } from './TagCell';
import { useTagSnapshot, useWriteField } from './useTagEditing';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { useTrack } from '../../hooks/useLibrary';
import { formatBitrate, formatTempo, truncatePath } from '../../utils/formatters';
import type { TagField, Track } from '../../types';

const TEXT_FIELDS: Array<{ field: TagField; label: string }> = [
  { field: 'artist', label: 'Artist' },
  { field: 'title', label: 'Title' },
  { field: 'album', label: 'Album' },
  { field: 'year', label: 'Year' },
  { field: 'genre', label: 'Genre' },
  { field: 'grouping', label: 'Grouping' },
  { field: 'comment', label: 'Comment' },
  { field: 'initialKey', label: 'Initial key' },
];

/**
 * Per-track deep editing: artwork and every text tag the file carries.
 *
 * The values come from `useTagSnapshot` — the file, read now — not from the
 * `tracks` row. The row is what the indexer last saw, and an editor that shows
 * a remembered value while the file says something else is worse than showing
 * nothing. The analysis facts underneath are the opposite case: they are
 * Keystone's own results and are shown read-only.
 */
export function TrackTagEditor({
  trackId,
  onClose,
}: {
  trackId: string;
  onClose: () => void;
}) {
  const { data: snapshot } = useTagSnapshot(trackId);
  const { data: track } = useTrack(trackId);
  const writeField = useWriteField();

  return (
    <aside className="flex w-80 shrink-0 flex-col border-l border-gray-700 bg-gray-900">
      <div className="flex items-center justify-between border-b border-gray-700 px-3 py-2">
        <span className="label">Song Info</span>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="flex h-6 w-6 items-center justify-center rounded text-gray-400 hover:bg-gray-800 hover:text-keystone-white"
        >
          <CloseCircle size={14} color="currentColor" variant="Linear" />
        </button>
      </div>

      <ScrollArea className="min-h-0 flex-1 p-3">
        <CoverArtPicker trackId={trackId} />

        <dl className="mt-4 flex flex-col gap-2">
          {TEXT_FIELDS.map(({ field, label }) => (
            <div key={field}>
              <dt className="label">{label}</dt>
              <dd>
                <TagCell
                  value={fieldValue(snapshot, field)}
                  numeric={field === 'year' || field === 'initialKey'}
                  onCommit={(v) => writeField(trackId, field, v)}
                />
              </dd>
            </div>
          ))}
        </dl>

        {track && <AnalysisFacts track={track} />}
      </ScrollArea>
    </aside>
  );
}

/** Keystone's own results, read-only: a key override is a different operation
    with its own source tracking, not a tag edit. */
function AnalysisFacts({ track }: { track: Track }) {
  const settings = useEffectiveSettings();
  return (
    <div className="mt-4 flex flex-col gap-1.5 border-t border-gray-700 pt-3">
      <span className="label">Analysis</span>
      <div className="flex items-center gap-2">
        <KeyBadge
          camelot={track.camelot}
          notation={settings?.keyNotation}
          pad={settings?.padCamelotNumbers}
          modeAmbiguous={track.keyModeAmbiguous}
        />
        <span className="mono tnum text-[11px] text-gray-400">
          {formatTempo(track.tempoDisplay, settings?.tempoDecimals ?? 0, track.tempoResidualMs)}{' '}
          BPM
        </span>
        <span className="mono tnum text-[11px] text-gray-400">
          Energy {track.energy ?? '\u2014'}
        </span>
      </div>
      <span className="mono text-[11px] text-gray-500">
        {formatBitrate(track.bitrateKbps)}
      </span>
      <span className="mono text-[11px] text-gray-500" title={track.path}>
        {truncatePath(track.path, 40)}
      </span>
    </div>
  );
}

/** The snapshot's numbers are numbers; the cells are text. */
function fieldValue(
  snapshot: ReturnType<typeof useTagSnapshot>['data'],
  field: TagField,
): string | null {
  if (!snapshot) return null;
  const value = snapshot[field as keyof typeof snapshot];
  if (value == null || typeof value === 'boolean') return null;
  return String(value);
}
