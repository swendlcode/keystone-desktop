import { create } from 'zustand';
import type { TagField } from '../types';

/**
 * A rectangular selection of tag cells: one column, a run of rows.
 *
 * One column rather than a true 2-D block. Every bulk edit a tagger actually
 * performs is "make this column say the same thing for these tracks" —
 * stamping an album across a rip, fixing a genre on an EP — and a block
 * spanning Artist and Title has no meaning you could type one value into.
 * Keeping it to a column is what lets a commit be unambiguous.
 */
interface TagSelection {
  /** Which column, or null when nothing is selected. */
  field: TagField | null;
  /** Row the selection started at. May be above or below `focus`. */
  anchor: number;
  /** Row the selection currently reaches. */
  focus: number;
  /** True while the pointer is down and sweeping a range. */
  dragging: boolean;
  /** The cell being typed into, or null. Always inside the selection. */
  editing: number | null;

  begin: (field: TagField, index: number) => void;
  extendTo: (index: number) => void;
  endDrag: () => void;
  edit: (index: number) => void;
  stopEditing: () => void;
  clear: () => void;
}

export const useTagSelectionStore = create<TagSelection>((set) => ({
  field: null,
  anchor: 0,
  focus: 0,
  dragging: false,
  editing: null,

  begin: (field, index) =>
    set({ field, anchor: index, focus: index, dragging: true, editing: null }),

  extendTo: (focus) => set({ focus }),

  endDrag: () => set({ dragging: false }),

  edit: (editing) => set({ editing }),

  stopEditing: () => set({ editing: null }),

  clear: () =>
    set({ field: null, anchor: 0, focus: 0, dragging: false, editing: null }),
}));

/** Inclusive row bounds, lowest first, whichever way the sweep went. */
export function selectionRange(s: TagSelection): [number, number] {
  return s.anchor <= s.focus ? [s.anchor, s.focus] : [s.focus, s.anchor];
}

/** Where a cell sits in the selection, for drawing the marquee. */
export interface CellSelection {
  selected: boolean;
  /** Topmost row of the run — draws the marquee's top edge. */
  first: boolean;
  /** Bottom row of the run — draws the bottom edge. */
  last: boolean;
  /** How many rows the commit will write to. */
  count: number;
}

export function cellSelection(
  s: TagSelection,
  field: TagField,
  index: number,
): CellSelection {
  if (s.field !== field) {
    return { selected: false, first: false, last: false, count: 0 };
  }
  const [from, to] = selectionRange(s);
  return {
    selected: index >= from && index <= to,
    first: index === from,
    last: index === to,
    count: to - from + 1,
  };
}
