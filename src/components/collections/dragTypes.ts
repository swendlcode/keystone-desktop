/**
 * The two drag payloads the collections sidebar understands.
 *
 * Custom MIME types rather than `text/plain`: a row has to decide, on
 * `dragover`, whether the thing being dragged is tracks to add or a collection
 * being reordered, and that decision has to happen before the drop — the drop
 * highlight is the only feedback the user gets. `dataTransfer.getData` returns
 * nothing during `dragover` for security reasons, so the *type* has to carry
 * the meaning, which is exactly what these two constants are for.
 *
 * The library table is the drag source for TRACK_IDS and sets it as a JSON
 * array of track ids.
 */
export const TRACK_IDS_MIME = 'application/x-keystone-track-ids';
export const COLLECTION_ID_MIME = 'application/x-keystone-collection-id';

export function setTrackDrag(dt: DataTransfer, trackIds: string[]): void {
  dt.setData(TRACK_IDS_MIME, JSON.stringify(trackIds));
  dt.effectAllowed = 'copy';
}

export function readTrackDrag(dt: DataTransfer): string[] {
  try {
    const raw = dt.getData(TRACK_IDS_MIME);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : [];
  } catch {
    // A drag from another application, or a malformed payload. Dropping it is
    // a no-op rather than an error dialog: the user already sees nothing moved.
    return [];
  }
}

export function hasType(dt: DataTransfer | null, mime: string): boolean {
  return Boolean(dt) && Array.from(dt!.types).includes(mime);
}
