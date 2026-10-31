import { COLUMN_ORDER, type ColumnVisibility, type TrackColumn, type TrackSort } from '../../types';

/**
 * Per-column layout: width, alignment, and which sort it maps to.
 *
 * The *set* of visible columns is not here — it is the Display Options
 * setting, read at render time. Hardcoding a default set in the table is how a
 * user who turns a column off watches it come back on the next release.
 */
export interface ColumnSpec {
  /** Minimum width in px. Flexible columns grow from it. */
  width: number;
  align: 'left' | 'right' | 'center';
  /** Share of the leftover width. Absent means a fixed-width column. */
  flex?: number;
  /** The backend sort this header requests, or null for an unsortable column. */
  sort: TrackSort | null;
}

export const COLUMN_SPECS: Record<TrackColumn, ColumnSpec> = {
  coverArt: { width: 34, align: 'center', sort: null },
  artist: { width: 150, align: 'left', flex: 3, sort: 'artist' },
  tempo: { width: 68, align: 'right', sort: 'tempo' },
  keyResult: { width: 76, align: 'left', sort: 'keyResult' },
  title: { width: 170, align: 'left', flex: 4, sort: 'title' },
  energy: { width: 84, align: 'left', sort: 'energy' },
  cuePoints: { width: 56, align: 'right', sort: 'cuePointCount' },
  dateAdded: { width: 104, align: 'left', sort: 'dateAdded' },
  rating: { width: 72, align: 'left', sort: 'rating' },
  path: { width: 160, align: 'left', flex: 2, sort: 'path' },
  grouping: { width: 110, align: 'left', flex: 1, sort: null },
  genre: { width: 110, align: 'left', flex: 1, sort: 'genre' },
  year: { width: 56, align: 'right', sort: 'year' },
  album: { width: 140, align: 'left', flex: 2, sort: 'album' },
  volume: { width: 76, align: 'right', sort: null },
  clippedPeaks: { width: 68, align: 'right', sort: null },
  bitrate: { width: 80, align: 'right', sort: 'bitrate' },
  comment: { width: 140, align: 'left', flex: 2, sort: null },
};

/** The visible columns, in `COLUMN_ORDER`. */
export function visibleColumns(columns: ColumnVisibility | undefined): TrackColumn[] {
  if (!columns) return [];
  return COLUMN_ORDER.filter((c) => columns[c]);
}

/**
 * `grid-template-columns` for a row and for the header, built from one list so
 * the two cannot drift by a pixel — which is the whole failure mode of a
 * table whose header is a separate element from its rows.
 */
export function gridTemplate(cols: TrackColumn[]): string {
  return cols
    .map((c) => {
      const spec = COLUMN_SPECS[c];
      return spec.flex
        ? `minmax(${spec.width}px, ${spec.flex}fr)`
        : `${spec.width}px`;
    })
    .join(' ');
}

export const ALIGN_CLASS: Record<ColumnSpec['align'], string> = {
  left: 'justify-start text-left',
  right: 'justify-end text-right',
  center: 'justify-center text-center',
};

/** Row height in px. The virtualizer's estimate and the real height. */
export const ROW_HEIGHT = 32;
