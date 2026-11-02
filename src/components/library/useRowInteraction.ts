import { useCallback, useRef } from 'react';
import { setTrackDrag } from '../collections/dragTypes';
import { useCollectionStore } from '../../stores/collectionStore';
import type { TrackWindow } from './useTrackWindow';
import { useLibraryStore } from '../../stores/libraryStore';
import { usePlayerStore } from '../../stores/playerStore';
import { useUiStore } from '../../stores/uiStore';
import type { Track } from '../../types';

/**
 * Click, double-click and right-click on a library row.
 *
 * A click selects; a DOUBLE click plays. Selecting used to audition too,
 * behind a setting — which made every pass through the list with the arrow
 * keys or a stray click start audio, and meant selecting twenty rows for a
 * bulk edit played twenty opening bars. Starting playback is a deliberate
 * act and gets a deliberate gesture.
 *
 * Every handler is built once and reads the current window from a ref. They
 * have to be stable for `TrackRow`'s memo to mean anything, and the window
 * object is a new identity on every fetch — so a dependency array would
 * rebuild them, and with them every mounted row, at each page boundary.
 */
export function useRowInteraction(window: TrackWindow) {
  const anchorRef = useRef(0);

  const winRef = useRef(window);
  winRef.current = window;

  const play = useCallback((track: Track) => {
    const player = usePlayerStore.getState();
    player.setPlaylist(winRef.current.loaded);
    player.play(track);
  }, []);

  const onRowMouseDown = useCallback(
    (e: React.MouseEvent, index: number) => {
      // Left button only.
      //
      // `mousedown` fires before `contextmenu`, so without this a right-click
      // ran the selection logic first and collapsed a selection of four
      // hundred rows down to the one under the pointer — by the time the menu
      // opened, the row was already the whole selection and the menu's own
      // "leave an existing selection alone" check had nothing left to
      // protect. With auto-play on it also started the track.
      if (e.button !== 0) return;

      const track = winRef.current.trackAt(index);
      if (!track) return;
      const store = useLibraryStore.getState();

      if (e.metaKey || e.ctrlKey) {
        store.toggleSelect(track.id);
        return;
      }
      if (e.shiftKey) {
        store.selectRange(idsBetween(winRef.current, anchorRef.current, index));
        return;
      }
      anchorRef.current = index;
      store.select(track.id);
    },
    [],
  );

  const onRowDoubleClick = useCallback(
    (index: number) => {
      const track = winRef.current.trackAt(index);
      if (track) play(track);
    },
    [play],
  );

  /**
   * Drag rows out to a collection in the sidebar.
   *
   * This is the library table only. The tag table had it too and should not:
   * that view is a grid of fields being typed into, where a press-and-move
   * over a cell is far more likely to be a mis-click than an intent to file
   * something, and the drag it started interrupted the edit.
   *
   * Dragging a row outside the selection drags that row alone, which is what
   * every file manager does.
   */
  const onRowDragStart = useCallback((e: React.DragEvent, index: number) => {
    const track = winRef.current.trackAt(index);
    if (!track) return;
    const selected = useLibraryStore.getState().selectedIds;
    const ids = selected.has(track.id) ? [...selected] : [track.id];
    setTrackDrag(e.dataTransfer, ids);
    // Lights the collections, so the drag has somewhere visibly to go.
    useCollectionStore.getState().setDraggingTracks(ids.length);
  }, []);

  const onRowDragEnd = useCallback(() => {
    useCollectionStore.getState().setDraggingTracks(null);
  }, []);

  const onRowContextMenu = useCallback((e: React.MouseEvent, index: number) => {
    const track = winRef.current.trackAt(index);
    if (!track) return;
    e.preventDefault();
    // Right-clicking outside the selection acts on that row, not on a
    // selection the user has forgotten about three screens up.
    if (!useLibraryStore.getState().selectedIds.has(track.id)) {
      useLibraryStore.getState().select(track.id);
      anchorRef.current = index;
    }
    // The menu itself lives in `App`, anchored on the store, so the library and
    // the tag table cannot each open one of their own.
    useUiStore.getState().openContextMenu({
      x: e.clientX,
      y: e.clientY,
      trackId: track.id,
    });
  }, []);

  return {
    play,
    onRowMouseDown,
    onRowDoubleClick,
    onRowDragStart,
    onRowDragEnd,
    onRowContextMenu,
  };
}

/**
 * Shift-click range, over the rows actually loaded.
 *
 * A range reaching outside the loaded window stops at its edge rather than
 * fetching: the alternative is a shift-click between row 1 and row 19,000
 * pulling ninety-five pages to build a selection the user then applies a bulk
 * tag write to.
 */
function idsBetween(win: TrackWindow, from: number, to: number): string[] {
  const lo = Math.min(from, to);
  const hi = Math.max(from, to);
  const ids: string[] = [];
  for (let i = lo; i <= hi; i += 1) {
    const track = win.trackAt(i);
    if (track) ids.push(track.id);
  }
  return ids;
}
