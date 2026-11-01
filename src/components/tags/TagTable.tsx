import { useCallback, useMemo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { COLS, ROW_H, TagRow } from './TagRow';
import { useSelectAll } from '../../hooks/useKeyboard';
import { useTracks } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';

const HEADERS = ['', 'Album', 'Artist', 'Title', 'Tempo', 'Key', 'Energy', 'Genre'];

/**
 * SPEC.md §2's table.
 *
 * Virtualized for the same reason the library table is: a DJ library runs to
 * tens of thousands of rows, and every visible row here also fetches its own
 * embedded cover art, so the window of mounted rows is what keeps that bounded.
 *
 * Selection is the table's job rather than the row's, because a shift-click
 * range needs to know about rows other than the one clicked. The row used to
 * own it and could therefore only ever do one at a time or toggle one more —
 * which made every bulk action in this view a sequence of Cmd-clicks.
 */
export function TagTable({ onOpen }: { onOpen: (trackId: string) => void }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const anchor = useRef(0);
  const { data } = useTracks(0);
  const tracks = data?.tracks ?? [];

  useSelectAll(useMemo(() => tracks.map((t) => t.id), [tracks]));

  // Read through a ref so the handler stays one identity for the life of the
  // table: `tracks` is a new array on every fetch, and a fresh handler would
  // re-render every mounted row with it.
  const tracksRef = useRef(tracks);
  tracksRef.current = tracks;

  const onRowMouseDown = useCallback(
    (e: React.MouseEvent, index: number) => {
      // Left button only — `mousedown` precedes `contextmenu`, and without
      // this a right-click collapsed the selection before the menu could see
      // it. Same bug as the library table.
      if (e.button !== 0) return;

      const track = tracksRef.current[index];
      if (!track) return;
      const store = useLibraryStore.getState();

      if (e.metaKey || e.ctrlKey) {
        store.toggleSelect(track.id);
        return;
      }
      if (e.shiftKey) {
        const lo = Math.min(anchor.current, index);
        const hi = Math.max(anchor.current, index);
        store.selectRange(
          tracksRef.current.slice(lo, hi + 1).map((t) => t.id),
        );
        return;
      }
      anchor.current = index;
      store.select(track.id);
    },
    [],
  );

  const rows = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: 12,
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className={`grid ${COLS} gap-2 border-b border-gray-700 px-3 py-2`}>
        {HEADERS.map((h, i) => (
          <span key={i} className="label truncate">
            {h}
          </span>
        ))}
      </div>

      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
        <div style={{ height: rows.getTotalSize(), position: 'relative' }}>
          {rows.getVirtualItems().map((v) => (
            <TagRow
              key={tracks[v.index].id}
              track={tracks[v.index]}
              index={v.index}
              top={v.start}
              onOpen={onOpen}
              onRowMouseDown={onRowMouseDown}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
