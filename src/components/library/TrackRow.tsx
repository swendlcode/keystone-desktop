import { memo } from 'react';
import { TrackCell, type CellFormat } from './TrackCell';
import { ALIGN_CLASS, COLUMN_SPECS } from './columns';
import type { Track, TrackColumn } from '../../types';

interface TrackRowProps extends CellFormat {
  /** Null while the page holding this index is still in flight. */
  track: Track | null;
  /** Library-wide row index, handed back to the table's handlers. */
  index: number;
  columns: TrackColumn[];
  gridTemplate: string;
  selected: boolean;
  playing: boolean;
  onRowMouseDown: (e: React.MouseEvent, index: number) => void;
  onRowDoubleClick: (index: number) => void;
  onRowDragStart: (e: React.DragEvent, index: number) => void;
  onRowDragEnd: () => void;
  onRowContextMenu: (e: React.MouseEvent, index: number) => void;
  onRate: (trackId: string, rating: number) => void;
}

/**
 * One library row.
 *
 * Memoised on purpose: scrolling a 20,000-row table re-renders the table on
 * every frame, and without this every mounted row re-renders with it even
 * though only the window moved. That makes the props a contract — the table
 * passes stable handlers that take the row index, and three formatting
 * primitives rather than the `Settings` object, because either an inline
 * closure or a freshly spread object would make the memo a no-op.
 */
export const TrackRow = memo(function TrackRow({
  track,
  index,
  columns,
  gridTemplate,
  selected,
  playing,
  notation,
  pad,
  decimals,
  onRowMouseDown,
  onRowDoubleClick,
  onRowDragStart,
  onRowDragEnd,
  onRowContextMenu,
  onRate,
}: TrackRowProps) {
  if (!track) {
    return (
      <div className="flex h-full items-center px-3" aria-hidden>
        <span className="h-2.5 w-1/3 animate-pulse rounded bg-gray-800" />
      </div>
    );
  }

  return (
    <div
      role="row"
      aria-selected={selected}
      draggable
      onMouseDown={(e) => onRowMouseDown(e, index)}
      onDoubleClick={() => onRowDoubleClick(index)}
      onDragStart={(e) => onRowDragStart(e, index)}
      onDragEnd={onRowDragEnd}
      onContextMenu={(e) => onRowContextMenu(e, index)}
      style={{ gridTemplateColumns: gridTemplate }}
      // `cursor-grab`: the row is a drag source and said nothing about it.
      // A default arrow over something draggable is the whole reason the
      // gesture goes undiscovered.
      className={`grid h-full cursor-grab items-center gap-3 border-b border-gray-800/60 px-3 text-xs active:cursor-grabbing ${
        selected ? 'bg-keystone-lime/10' : 'hover:bg-gray-800/60'
      } ${playing ? 'shadow-[inset_2px_0_0_0_var(--color-accent)]' : ''}`}
    >
      {columns.map((column) => (
        <span
          key={column}
          role="cell"
          className={`flex min-w-0 items-center ${ALIGN_CLASS[COLUMN_SPECS[column].align]}`}
        >
          <TrackCell
            column={column}
            track={track}
            notation={notation}
            pad={pad}
            decimals={decimals}
            onRate={onRate}
          />
        </span>
      ))}
    </div>
  );
});
