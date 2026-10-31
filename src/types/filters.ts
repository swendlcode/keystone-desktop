import type { IndexStatus, Track } from './track';

/** Mirrors `TrackSort` in models/track.rs. */
export type TrackSort =
  | 'artist'
  | 'title'
  | 'album'
  | 'genre'
  | 'tempo'
  | 'keyResult'
  | 'energy'
  | 'dateAdded'
  | 'rating'
  | 'cuePointCount'
  | 'bitrate'
  | 'year'
  | 'filename'
  | 'path';

export type SortDirection = 'asc' | 'desc';

/**
 * Mirrors `TrackFilters`. One object, so the react-query key is the whole
 * filter state and a changed filter is a changed key — no manual invalidation.
 */
export interface TrackFilters {
  /** Free text, routed through the FTS5 MATCH path. */
  query: string;
  /** The wheel selection, e.g. `8A`. */
  camelot: string | null;
  /** Expand the selection to its four-element compatibility set. */
  camelotIncludeCompatible: boolean;
  collectionId: string | null;
  energyMin: number | null;
  energyMax: number | null;
  tempoMin: number | null;
  tempoMax: number | null;
  genres: string[];
  ratingMin: number | null;
  /** null = either, true = analysed only, false = unanalysed only. */
  analyzed: boolean | null;
  /** Range-scanned, never `LIKE 'prefix%'`. */
  pathPrefix: string | null;
  indexStatuses: IndexStatus[];
  /** The spec's `Improve Tracks` set: voter disagreement, low confidence,
      clipping, or no analysis at all. */
  improvableOnly: boolean;
}

export interface TrackQuery {
  filters: TrackFilters;
  sort: TrackSort;
  direction: SortDirection;
  limit: number;
  offset: number;
}

export interface TrackPage {
  tracks: Track[];
  /** Total matching rows, for the `412 TRACKS` readout. */
  total: number;
}

/**
 * One count per wheel segment, so a segment with nothing behind it reads as
 * empty instead of filtering to zero rows and looking broken.
 */
export interface WheelCounts {
  /** 24 entries, `['1A', n]` .. `['12B', n]`. */
  counts: Array<[string, number]>;
  unkeyed: number;
}

export const DEFAULT_FILTERS: TrackFilters = {
  query: '',
  camelot: null,
  camelotIncludeCompatible: true,
  collectionId: null,
  energyMin: null,
  energyMax: null,
  tempoMin: null,
  tempoMax: null,
  genres: [],
  ratingMin: null,
  analyzed: null,
  pathPrefix: null,
  indexStatuses: ['pending', 'indexed'],
  improvableOnly: false,
};

export const DEFAULT_SORT: TrackSort = 'artist';
export const DEFAULT_DIRECTION: SortDirection = 'asc';
