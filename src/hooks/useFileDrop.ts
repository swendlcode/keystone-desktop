import { useEffect, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { keystoneService } from '../services/keystoneService';

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

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed) return;

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
