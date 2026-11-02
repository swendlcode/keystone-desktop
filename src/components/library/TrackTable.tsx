import { useEffect, useMemo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { EmptyState } from '../ui/EmptyState';
import { MusicSquare } from '../ui/icons';
import { TrackRow } from './TrackRow';
import { TrackTableHeader } from './TrackTableHeader';
import { ROW_HEIGHT, gridTemplate, visibleColumns } from './columns';
import { useRowInteraction } from './useRowInteraction';
import type { TrackWindow } from './useTrackWindow';
import { useSelectAll } from '../../hooks/useKeyboard';
import { PAGE_SIZE, useSetRating } from '../../hooks/useLibrary';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { useLibraryStore } from '../../stores/libraryStore';
import { usePlayerStore } from '../../stores/playerStore';
import { useUiStore } from '../../stores/uiStore';

interface TrackTableProps {
  window: TrackWindow;
  /** Called when the scroll position moves into another page. */
  onPageChange: (page: number) => void;
}

/**
 * The library table.
 *
 * Virtualized over the backend's total rather than over a loaded array, so
 * the scrollbar is the real length of the result the moment the first page
 * lands and a jump to row 14,000 is one fetch instead of seventy. Rows the
 * window does not hold yet render as a placeholder rather than as a gap.
 */
export function TrackTable({ window: win, onPageChange }: TrackTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const settings = useEffectiveSettings();
  const setRating = useSetRating();

  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const playingId = usePlayerStore((s) => s.currentTrack?.id ?? null);

  const rows = useRowInteraction(win);
  useSelectAll(useMemo(() => win.loaded.map((t) => t.id), [win.loaded]));

  const columnOrder = useUiStore((s) => s.columnOrder);
  const columns = useMemo(
    () => visibleColumns(settings?.columns, columnOrder),
    [settings?.columns, columnOrder],
  );
  const template = useMemo(() => gridTemplate(columns), [columns]);

  const virtualizer = useVirtualizer({
    count: win.total,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  const items = virtualizer.getVirtualItems();
  const firstIndex = items[0]?.index ?? 0;

  useEffect(() => {
    onPageChange(Math.floor(firstIndex / PAGE_SIZE));
  }, [firstIndex, onPageChange]);

  // `setRating.mutate` is stable across renders, so this stays one identity
  // for the life of the table and does not defeat the row memo.
  const onRate = useRef((trackId: string, rating: number) => {
    setRating.mutate({ id: trackId, rating });
  }).current;

  if (win.total === 0 && !win.isLoading) {
    return (
      <EmptyState
        title="No tracks"
        hint="Add a folder, or clear the filters."
        icon={<MusicSquare size={28} color="currentColor" variant="Linear" />}
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
        {/* `w-full`, not `min-w-max`: the `fr` tracks in the column
            template need a definite width to divide, and a shrink-to-fit
            container resolves every flexible column to its minimum instead.
            Columns that do not fit overflow and the parent scrolls them. */}
        <div className="w-full">
          <TrackTableHeader columns={columns} gridTemplate={template} />
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {items.map((item) => {
              const track = win.trackAt(item.index);
              return (
                <div
                  key={item.key}
                  style={{
                    position: 'absolute',
                    top: item.start,
                    left: 0,
                    right: 0,
                    height: item.size,
                  }}
                >
                  <TrackRow
                    track={track}
                    index={item.index}
                    columns={columns}
                    gridTemplate={template}
                    selected={track != null && selectedIds.has(track.id)}
                    playing={track != null && track.id === playingId}
                    notation={settings?.keyNotation ?? 'camelot'}
                    pad={settings?.padCamelotNumbers ?? false}
                    decimals={settings?.tempoDecimals ?? 0}
                    onRowMouseDown={rows.onRowMouseDown}
                    onRowDoubleClick={rows.onRowDoubleClick}
                    onRowPointerDown={rows.onRowPointerDown}
                    onRowContextMenu={rows.onRowContextMenu}
                    onRate={onRate}
                  />
                </div>
              );
            })}
          </div>
        </div>
      </div>

    </div>
  );
}
