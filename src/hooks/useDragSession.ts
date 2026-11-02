import { useEffect } from 'react';
import {
  dropZone,
  useDragStore,
  zoneAtPoint,
  type DragPayload,
} from '../stores/dragStore';

/**
 * The window listeners that carry a pointer drag, mounted once in `App`.
 *
 * Everything after the threshold is crossed happens here rather than on the
 * row, because the pointer spends the whole drag somewhere else — over the
 * sidebar, over a collection, outside the window. A row cannot see any of
 * that, which is the structural reason the drag has to be a session rather
 * than a handler.
 *
 * `pointerup` resolves the drop with the same hit test `pointermove` used for
 * the highlight, so the row that lit up is by construction the row that
 * receives. That identity is the whole point: the old code decided the
 * highlight in `dragover` and the destination in `drop`, and two hit tests
 * are two chances to disagree.
 */
export function useDragSession(): void {
  const payload = useDragStore((s) => s.payload);

  useEffect(() => {
    if (!payload) return;

    const settle = (p: DragPayload, x: number, y: number) => {
      const id = zoneAtPoint(x, y, p);
      const zone = dropZone(id);
      // End first: the mutation the handler fires can invalidate queries and
      // unmount the zone, and a session still holding a payload at that point
      // leaves the ghost on screen.
      useDragStore.getState().end();
      zone?.onDrop(p);
    };

    const onMove = (e: PointerEvent) => {
      useDragStore
        .getState()
        .move(e.clientX, e.clientY, zoneAtPoint(e.clientX, e.clientY, payload));
    };
    const onUp = (e: PointerEvent) => settle(payload, e.clientX, e.clientY);
    const onCancel = () => useDragStore.getState().end();
    const onKey = (e: KeyboardEvent) => {
      // Escape abandons the drag, the way it does in every file manager.
      if (e.key === 'Escape') useDragStore.getState().end();
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onCancel);
    // Released outside the window, or the window lost focus mid-drag. Either
    // way no `pointerup` is coming here, and a session left open would hold
    // the ghost on screen over an app the user has moved on to.
    window.addEventListener('blur', onCancel);
    window.addEventListener('keydown', onKey);
    // A drag that selects the text it passes over looks broken even when it
    // works, and the selection survives the drop.
    document.body.classList.add('dragging');

    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onCancel);
      window.removeEventListener('blur', onCancel);
      window.removeEventListener('keydown', onKey);
      document.body.classList.remove('dragging');
    };
  }, [payload]);
}
