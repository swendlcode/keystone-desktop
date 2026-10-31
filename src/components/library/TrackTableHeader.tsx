import { ArrowDown2, ArrowUp2 } from '../ui/icons';
import { ALIGN_CLASS, COLUMN_SPECS } from './columns';
import { useLibraryStore } from '../../stores/libraryStore';
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
 */
export function TrackTableHeader({ columns, gridTemplate }: TrackTableHeaderProps) {
  const sort = useLibraryStore((s) => s.sort);
  const direction = useLibraryStore((s) => s.direction);
  const toggleSort = useLibraryStore((s) => s.toggleSort);

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
            disabled={spec.sort == null}
            onClick={() => spec.sort && toggleSort(spec.sort)}
            className={`label flex min-w-0 items-center gap-1 truncate ${
              ALIGN_CLASS[spec.align]
            } ${active ? 'text-keystone-white' : ''} ${
              spec.sort == null ? 'cursor-default' : 'hover:text-keystone-white'
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
