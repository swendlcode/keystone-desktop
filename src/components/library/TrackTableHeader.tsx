import { useRef, useState } from 'react';
import { ArrowDown2, ArrowUp2 } from '../ui/icons';
import { ALIGN_CLASS, COLUMN_SPECS } from './columns';
import { useLibraryStore } from '../../stores/libraryStore';
import { useUiStore } from '../../stores/uiStore';
import { COLUMN_LABELS, type TrackColumn } from '../../types';

interface TrackTableHeaderProps {
  columns: TrackColumn[];
  gridTemplate: string;
}

/**
 * The sortable header.
 *
 * Sticky rather than a separate element above the scroll container, so it
 * shares the row's `grid-template-columns` string and cannot drift out of
 * alignment with the cells — the one thing a hand-built table always gets
 * wrong.
 *
 * Headers are draggable to reorder. The drop writes the full order — every
 * column, not just the visible ones — so a hidden column keeps its place and
 * comes back where it was rather than at the end.
 */
export function TrackTableHeader({ columns, gridTemplate }: TrackTableHeaderProps) {
  const sort = useLibraryStore((s) => s.sort);
  const direction = useLibraryStore((s) => s.direction);
  const toggleSort = useLibraryStore((s) => s.toggleSort);
  const columnOrder = useUiStore((s) => s.columnOrder);
  const setColumnOrder = useUiStore((s) => s.setColumnOrder);

  const dragged = useRef<TrackColumn | null>(null);
  const [over, setOver] = useState<TrackColumn | null>(null);

  const drop = (target: TrackColumn) => {
    const from = dragged.current;
    dragged.current = null;
    setOver(null);
    if (!from || from === target) return;

    const next = columnOrder.filter((c) => c !== from);
    const at = next.indexOf(target);
    if (at === -1) return;
    // Dropping on a column to the right of the dragged one puts it after
    // that column, which is where the pointer is; to the left, before it.
    const movingRight = columnOrder.indexOf(from) < columnOrder.indexOf(target);
    next.splice(movingRight ? at + 1 : at, 0, from);
    setColumnOrder(next);
  };

  return (
    <div
      role="row"
      style={{ gridTemplateColumns: gridTemplate }}
      className="sticky top-0 z-20 grid h-8 items-center gap-3 border-b border-gray-700 bg-gray-900 px-3"
    >
      {columns.map((column) => {
        const spec = COLUMN_SPECS[column];
        const active = spec.sort != null && spec.sort === sort;

        return (
          <button
            key={column}
            type="button"
            role="columnheader"
            aria-sort={
              active ? (direction === 'asc' ? 'ascending' : 'descending') : 'none'
            }
            // Draggable even when unsortable: moving a column and sorting by
            // it are different gestures, and `coverArt` has to be movable
            // despite having nothing to sort by.
            draggable
            onDragStart={(e) => {
              dragged.current = column;
              e.dataTransfer.effectAllowed = 'move';
              // Firefox refuses to start a drag with an empty payload.
              e.dataTransfer.setData('text/plain', column);
            }}
            onDragOver={(e) => {
              if (!dragged.current) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              setOver(column);
            }}
            onDragLeave={() => setOver((c) => (c === column ? null : c))}
            onDrop={(e) => {
              e.preventDefault();
              drop(column);
            }}
            onDragEnd={() => {
              dragged.current = null;
              setOver(null);
            }}
            title={`${COLUMN_LABELS[column]} — drag to move`}
            onClick={() => spec.sort && toggleSort(spec.sort)}
            className={`label flex min-w-0 cursor-grab items-center gap-1 truncate active:cursor-grabbing ${
              ALIGN_CLASS[spec.align]
            } ${active ? 'text-keystone-white' : ''} ${
              spec.sort == null ? '' : 'hover:text-keystone-white'
            } ${
              over === column
                ? 'bg-keystone-lime/15 text-keystone-lime'
                : ''
            }`}
          >
            <span className="truncate">{COLUMN_LABELS[column]}</span>
            {active &&
              (direction === 'asc' ? (
                <ArrowUp2 size={10} color="currentColor" variant="Bold" />
              ) : (
                <ArrowDown2 size={10} color="currentColor" variant="Bold" />
              ))}
          </button>
        );
      })}
    </div>
  );
}
