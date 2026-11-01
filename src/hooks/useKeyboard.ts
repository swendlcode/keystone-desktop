import { useEffect, useRef } from 'react';
import { useLibraryStore } from '../stores/libraryStore';
import { usePlayerStore } from '../stores/playerStore';

/** True when a keystroke belongs to a field rather than to the window. */
function inField(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return Boolean(
    el &&
      (el.tagName === 'INPUT' ||
        el.tagName === 'TEXTAREA' ||
        el.isContentEditable),
  );
}

/**
 * Cmd / Ctrl + A: select every row the mounted table is holding.
 *
 * Holding, not matching. The tables are virtualized over the backend's total
 * and fetch two pages around the scroll position, so "all" here is the loaded
 * window — the same boundary shift-click already stops at, and for the same
 * reason: select-all over a 20,000-row filter would pull a hundred pages to
 * build a selection whose next use is a bulk tag write. The toolbar prints
 * the resulting count so what was selected is never a guess.
 */
export function useSelectAll(ids: string[]) {
  const latest = useRef(ids);
  latest.current = ids;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'a' || !(e.metaKey || e.ctrlKey)) return;
      if (inField(e.target)) return;
      e.preventDefault();
      useLibraryStore.getState().selectRange(latest.current);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}

/**
 * Global shortcuts. Mounted once, from App.
 *
 * The spacebar is `pause()`, never `stop()`: a DJ auditioning a breakdown taps
 * it to hold and taps again to carry on. Mapping it to stop sends them back to
 * the intro every time.
 */
export function useKeyboard() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Never steal a key from a field — the tag editor is a grid of them.
      if (inField(e.target)) return;

      const player = usePlayerStore.getState();

      if (e.code === 'Space') {
        e.preventDefault();
        if (!player.currentTrack) return;
        if (player.isPlaying) player.pause();
        else player.resume();
        return;
      }

      if (e.key === 'ArrowRight' && e.metaKey) {
        e.preventDefault();
        player.playNext();
        return;
      }

      if (e.key === 'ArrowLeft' && e.metaKey) {
        e.preventDefault();
        player.playPrev();
        return;
      }

      if (e.key === 'Escape') {
        useLibraryStore.getState().clearSelection();
      }
    };

    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
