import { useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { COLS, ROW_H, TagRow } from './TagRow';
import { useTracks } from '../../hooks/useLibrary';

const HEADERS = ['', 'Album', 'Artist', 'Title', 'Tempo', 'Key', 'Energy', 'Genre'];

/**
 * SPEC.md §2's table.
 *
 * Virtualized for the same reason the library table is: a DJ library runs to
 * tens of thousands of rows, and every visible row here also fetches its own
 * embedded cover art, so the window of mounted rows is what keeps that bounded.
 */
export function TagTable({ onOpen }: { onOpen: (trackId: string) => void }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { data } = useTracks(0);
  const tracks = data?.tracks ?? [];

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
              top={v.start}
              onOpen={onOpen}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
