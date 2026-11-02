import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { keystoneService } from '../services/keystoneService';
import { useDragStore } from '../stores/dragStore';

/**
 * Drag files or folders from Finder onto the window.
 *
 * This listens to Tauri's window-level drag-drop event, which is the right one
 * here: these drags originate in Finder, in another process. (A drag the app
 * itself starts never reaches this event — that is a different problem with a
 * different answer.)
 *
 * Returns whether something is currently hovering, so the UI can show where it
 * will land. Without that feedback a drop is a guess.
 */
export function useFileDrop() {
  const [hovering, setHovering] = useState(false);
  const qc = useQueryClient();
  // Read inside the listener, which is registered once and must not close
  // over a stale value.
  const hoveringRef = useRef(hovering);
  hoveringRef.current = hovering;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed) return;

        // A drag that started inside the window is a row on its way to a
        // collection, not a file arriving from Finder. Tauri's window-level
        // handler does not always tell them apart, and when it does not, the
        // whole-window "Drop to analyse" sheet threw itself over the app the
        // moment a track was picked up — covering the sidebar the track was
        // being dragged to. Internal drags are pointer drags now and start no
        // native session, so this should no longer fire; it stays because the
        // cost of being wrong is the overlay covering the drop target again.
        if (useDragStore.getState().payload != null) {
          if (hoveringRef.current) setHovering(false);
          return;
        }

        if (event.payload.type === 'over') {
          setHovering(true);
          return;
        }
        if (event.payload.type === 'leave') {
          setHovering(false);
          return;
        }
        if (event.payload.type !== 'drop') return;

        setHovering(false);
        const paths = event.payload.paths;
        if (!paths || paths.length === 0) return;

        keystoneService
          .addDroppedPaths(paths)
          .then(() => {
            // The indexer emits as rows land, but the first of them can be a
            // second away on a large folder; refreshing now means the window
            // reacts to the drop rather than appearing to swallow it.
            void qc.invalidateQueries({ queryKey: ['tracks'] });
            void qc.invalidateQueries({ queryKey: ['library-folders'] });
            void qc.invalidateQueries({ queryKey: ['watched-folders'] });
          })
          .catch(() => {
            /* surfaced by the analysis queue; a failed drop must not throw here */
          });
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {
        /* not running in a Tauri window — nothing to listen to */
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [qc]);

  return hovering;
}
