import { useEffect } from 'react';
import { usePlayerStore } from '../stores/playerStore';

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
      const target = e.target as HTMLElement | null;
      // Never steal a key from a field — the tag editor is a grid of them.
      if (
        target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.isContentEditable)
      ) {
        return;
      }

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
      }
    };

    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
