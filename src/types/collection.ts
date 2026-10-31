/** Mirrors `src-tauri/src/models/collection.rs`. */

export type CollectionKind = 'user' | 'smart';

export interface Collection {
  id: string;
  name: string;
  kind: CollectionKind;
  /** Smart collections only: a serialized `TrackFilters`. Membership is a
      query, not rows, so `collection_tracks` stays empty for these. */
  rule: string | null;
  color: string | null;
  position: number;
  /** Every sidebar row carries a count, so it is part of the read model
      rather than a second round trip per row. */
  trackCount: number;
  createdAt: number;
}

export interface CollectionTrack {
  collectionId: string;
  trackId: string;
  position: number;
  addedAt: number;
}
