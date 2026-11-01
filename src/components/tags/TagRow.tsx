import { KeyResult } from '../library/KeyResult';
import { setTrackDrag } from '../collections/dragTypes';
import { CoverArtThumb } from './CoverArtThumb';
import { TagCell } from './TagCell';
import { useWriteField } from './useTagEditing';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { useLibraryStore } from '../../stores/libraryStore';
import { useUiStore } from '../../stores/uiStore';
import { formatTempo } from '../../utils/formatters';
import type { Track } from '../../types';

export const ROW_H = 36;
export const COLS = 'grid-cols-[44px_1fr_1fr_1.4fr_76px_76px_60px_1fr]';

/**
 * One row of SPEC.md §2's table.
 *
 * Album, artist, title and genre write straight to the file. Tempo, key and
 * energy are read-only: they are analysis results rather than tags, and
 * overriding one is a different operation with its own source tracking
 * (`keySource: 'user'`) so a later re-analysis cannot quietly undo it.
 */
export function TagRow({
  track,
  index,
  top,
  onOpen,
  onRowMouseDown,
}: {
  track: Track;
  /** Row index, so the table can resolve a shift-click range. */
  index: number;
  top: number;
  onOpen: (trackId: string) => void;
  onRowMouseDown: (e: React.MouseEvent, index: number) => void;
}) {
  const settings = useEffectiveSettings();
  const writeField = useWriteField();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const select = useLibraryStore((s) => s.select);
  const openContextMenu = useUiStore((s) => s.openContextMenu);
  const selected = selectedIds.has(track.id);

  return (
    <div
      style={{ position: 'absolute', top, height: ROW_H, left: 0, right: 0 }}
      className={`grid ${COLS} items-center gap-2 px-3 ${
        selected ? 'bg-keystone-lime/10' : 'hover:bg-gray-800/60'
      }`}
      draggable
      onDragStart={(e) =>
        // Dragging a row outside the selection drags that row alone, which is
        // what every file manager does.
        setTrackDrag(e.dataTransfer, selected ? [...selectedIds] : [track.id])
      }
      onMouseDown={(e) => onRowMouseDown(e, index)}
      onDoubleClick={() => onOpen(track.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        if (!selected) select(track.id);
        openContextMenu({ x: e.clientX, y: e.clientY, trackId: track.id });
      }}
    >
      <CoverArtThumb trackId={track.id} />
      <TagCell value={track.album} onCommit={(v) => writeField(track.id, 'album', v)} />
      <TagCell value={track.artist} onCommit={(v) => writeField(track.id, 'artist', v)} />
      <TagCell value={track.title} onCommit={(v) => writeField(track.id, 'title', v)} />
      <span className="mono tnum px-1.5 text-xs text-gray-400">
        {formatTempo(track.tempoDisplay, settings?.tempoDecimals ?? 0, track.tempoResidualMs)}
      </span>
      <span className="px-1.5">
        <KeyResult
          track={track}
          notation={settings?.keyNotation ?? 'camelot'}
          pad={settings?.padCamelotNumbers ?? false}
        />
      </span>
      <span className="mono tnum px-1.5 text-xs text-gray-400">{track.energy ?? ''}</span>
      <TagCell value={track.genre} onCommit={(v) => writeField(track.id, 'genre', v)} />
    </div>
  );
}
