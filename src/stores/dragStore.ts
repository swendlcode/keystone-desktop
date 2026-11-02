import { create } from 'zustand';
import type { Track } from '../types';

/** Pointer travel, in px, before a press on a row becomes a drag. */
export const DRAG_THRESHOLD = 4;

/** The attribute a drop target carries, so hit-testing is a DOM lookup. */
export const DROP_ZONE_ATTR = 'data-drop-zone';

/** What is in the air. */
export type DragPayload =
  | { kind: 'tracks'; trackIds: string[]; lead: Track }
  | { kind: 'collection'; collectionId: string };

export interface DropZone {
  /** Payload kinds this zone will take; anything else does not light it up. */
  accepts: DragPayload['kind'][];
  onDrop: (payload: DragPayload) => void;
}

/**
 * Keystone drags rows with the pointer, not with HTML5 drag-and-drop.
 *
 * The HTML5 version was unreliable in the one direction that mattered. Three
 * things worked against it at once. `dataTransfer.getData` is unreadable
 * during `dragover`, so the payload had to be smuggled through the *type*
 * list and every target had to guess what was coming. `dragDropEnabled` is on
 * for the window because Finder imports need Tauri's native handler, and that
 * handler sits in front of the webview's own drop — the sheet throwing itself
 * over the app the moment a row was picked up was the visible half of the same
 * problem, and it is documented as outright breaking frontend drag-and-drop on
 * Windows. And the rows were 2px apart with no target in the gap, so a drop
 * aimed between two collections landed on neither.
 *
 * A pointer drag has none of those seams: the payload is a real object rather
 * than a MIME string, the highlight and the drop read the same hit test so
 * what lights up is what receives, and the drag image is a React component —
 * which is what lets the ghost carry the track's energy and key.
 *
 * The zone registry is a plain Map rather than store state on purpose. Zones
 * come and go with mounted rows and are read once per pointer move; putting
 * them in the store would re-render every subscriber on each registration.
 */
interface DragStore {
  /** Null when nothing is being dragged. */
  payload: DragPayload | null;
  /** Viewport position of the pointer, for the ghost. */
  x: number;
  y: number;
  /** Id of the zone under the pointer that will accept this payload. */
  overId: string | null;

  begin: (payload: DragPayload, x: number, y: number) => void;
  move: (x: number, y: number, overId: string | null) => void;
  end: () => void;
}

export const useDragStore = create<DragStore>((set) => ({
  payload: null,
  x: 0,
  y: 0,
  overId: null,

  begin: (payload, x, y) => set({ payload, x, y, overId: null }),
  move: (x, y, overId) => set({ x, y, overId }),
  end: () => set({ payload: null, overId: null }),
}));

/**
 * Arm a press so it becomes a drag only once the pointer travels.
 *
 * The threshold is what keeps a drag source from stealing clicks: a press
 * that never moves `DRAG_THRESHOLD` px starts no session, so selecting a row
 * and picking one up are the same gesture told apart by distance rather than
 * by two different controls. `make` is called at that moment, not at press
 * time, so the payload reflects the selection as it stands when the drag
 * actually begins.
 */
export function armPointerDrag(
  fromX: number,
  fromY: number,
  make: () => DragPayload | null,
): void {
  // Declarations, so the two can refer to each other.
  function onMove(ev: PointerEvent) {
    // The button came up somewhere this listener could not see it — released
    // outside the window is the ordinary way. Without this the arming
    // survives, and the next idle pointer move crosses the threshold and
    // starts a drag the user never pressed for.
    if (ev.buttons === 0) {
      stop();
      return;
    }
    if (
      Math.abs(ev.clientX - fromX) + Math.abs(ev.clientY - fromY) <
      DRAG_THRESHOLD
    ) {
      return;
    }
    stop();
    const payload = make();
    if (payload) useDragStore.getState().begin(payload, ev.clientX, ev.clientY);
  }

  function stop() {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', stop);
  }

  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', stop);
}

/**
 * How many tracks are mid-drag, or null when none are.
 *
 * Derived rather than stored. It used to be its own field on
 * `collectionStore`, set by the row that started the drag and cleared by
 * whichever of `dragend` or `drop` ran first — two writers for one fact, and
 * a drop that never fired left the sidebar lit with nothing in the air.
 */
export function useDraggingTrackCount(): number | null {
  return useDragStore((s) =>
    s.payload?.kind === 'tracks' ? s.payload.trackIds.length : null,
  );
}

const zones = new Map<string, DropZone>();

/** Register while mounted. The returned function must run on unmount. */
export function registerDropZone(id: string, zone: DropZone): () => void {
  zones.set(id, zone);
  return () => {
    // Guarded: a remount can register the same id before the old cleanup
    // runs, and an unguarded delete would drop the live zone.
    if (zones.get(id) === zone) zones.delete(id);
  };
}

export function dropZone(id: string | null): DropZone | undefined {
  return id == null ? undefined : zones.get(id);
}

/**
 * The id of the zone under the pointer, or null.
 *
 * Read off the DOM rather than from cached rectangles so a list that scrolls
 * or reflows mid-drag cannot hand back a stale target. The ghost is
 * `pointer-events-none`, which is what keeps it from being the hit.
 */
export function zoneAtPoint(
  x: number,
  y: number,
  payload: DragPayload,
): string | null {
  const host = document
    .elementFromPoint(x, y)
    ?.closest(`[${DROP_ZONE_ATTR}]`);
  const id = host?.getAttribute(DROP_ZONE_ATTR) ?? null;
  const zone = dropZone(id);
  return zone?.accepts.includes(payload.kind) ? id : null;
}
