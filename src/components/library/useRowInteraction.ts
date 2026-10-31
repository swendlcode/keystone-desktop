import { useCallback, useRef } from 'react';
import type { TrackWindow } from './useTrackWindow';
import { useLibraryStore } from '../../stores/libraryStore';
import { usePlayerStore } from '../../stores/playerStore';
import { useUiStore } from '../../stores/uiStore';
import type { Track } from '../../types';

/**
 * Click, double-click and right-click on a library row.
 *
 * Every handler is built once and reads the current window and the auto-play
 * setting from refs. They have to be stable for `TrackRow`'s memo to mean
 * anything, and the window object is a new identity on every fetch — so a
 * dependency array would rebuild them, and with them every mounted row, at
 * each page boundary.
 */
export function useRowInteraction(window: TrackWindow, autoPlay: boolean) {
  const anchorRef = useRef(0);

  const winRef = useRef(window);
  winRef.current = window;
  const autoPlayRef = useRef(autoPlay);
  autoPlayRef.current = autoPlay;

  const play = useCallback((track: Track) => {
    const player = usePlayerStore.getState();
    player.setPlaylist(winRef.current.loaded);
    player.play(track);
  }, []);

  const onRowMouseDown = useCallback(
    (e: React.MouseEvent, index: number) => {
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
      // SPEC.md's Audio Player setting: auditioning on select is the default,
      // and off is for when a DJ app already owns the audio device.
      if (autoPlayRef.current) play(track);
    },
    [play],
  );

  const onRowDoubleClick = useCallback(
    (index: number) => {
      const track = winRef.current.trackAt(index);
      if (track) play(track);
    },
    [play],
  );

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
