import { useCallback, useEffect, useRef } from 'react';
import { usePlayerStore } from '../stores/playerStore';
import { keystoneService, mediaUrl } from '../services/keystoneService';
import { useEffectiveSettings } from './useSettings';
import type { Track } from '../types';

/**
 * Owns the single <audio> element.
 *
 * Mounted once, from PlayerBar. An HTMLAudioElement rather than a decoded
 * AudioBuffer because Keystone's files are whole tracks: a 10-minute 48 k
 * stereo f32 buffer is 230 MB, and the engine plan is explicit that nothing
 * holds a decoded track in memory.
 *
 * `stopSeq` is how this hook tells `stop()` from `pause()`. They both set
 * `isPlaying: false`, so without it every stop records the playhead as a
 * resume point and repeated presses on one track walk that point forward until
 * the play button appears dead.
 */
export function usePlayer() {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const stopSeqRef = useRef(0);
  const loadedIdRef = useRef<string | null>(null);
  /** The track whose `src` is actually assigned, as opposed to in flight. */
  const readyIdRef = useRef<string | null>(null);

  const currentTrack = usePlayerStore((s) => s.currentTrack);
  const isPlaying = usePlayerStore((s) => s.isPlaying);
  const volume = usePlayerStore((s) => s.volume);
  const stopSeq = usePlayerStore((s) => s.stopSeq);
  const pendingStartAt = usePlayerStore((s) => s.pendingStartAt);
  const setCurrentTime = usePlayerStore((s) => s.setCurrentTime);
  const setDuration = usePlayerStore((s) => s.setDuration);
  const playNext = usePlayerStore((s) => s.playNext);
  const registerSeek = usePlayerStore((s) => s.registerSeek);

  const settings = useEffectiveSettings();

  // One element for the life of the window.
  useEffect(() => {
    const el = new Audio();
    el.preload = 'metadata';
    audioRef.current = el;

    const onTime = () => setCurrentTime(el.currentTime);
    const onMeta = () => setDuration(el.duration || 0);
    const onEnded = () => playNext();

    el.addEventListener('timeupdate', onTime);
    el.addEventListener('loadedmetadata', onMeta);
    el.addEventListener('ended', onEnded);

    return () => {
      el.removeEventListener('timeupdate', onTime);
      el.removeEventListener('loadedmetadata', onMeta);
      el.removeEventListener('ended', onEnded);
      el.pause();
      el.src = '';
      audioRef.current = null;
    };
  }, [setCurrentTime, setDuration, playNext]);

  const seek = useCallback((time: number) => {
    const el = audioRef.current;
    if (!el) return;
    el.currentTime = time;
    usePlayerStore.getState().setCurrentTime(time);
  }, []);

  useEffect(() => registerSeek(seek), [registerSeek, seek]);

  useEffect(() => {
    const el = audioRef.current;
    if (el) el.volume = volume;
  }, [volume]);

  // Load on track change. The path comes from the backend because the row's
  // `path` can be stale after a rename, and the command is the one place that
  // knows which it is.
  useEffect(() => {
    const el = audioRef.current;
    if (!el || !currentTrack) return;
    if (loadedIdRef.current === currentTrack.id) return;

    let cancelled = false;
    loadedIdRef.current = currentTrack.id;
    readyIdRef.current = null;

    // Both at once: the offset is a database read, and serialising it behind the
    // path read would delay the first frame of audio for no reason.
    Promise.all([
      keystoneService.getPlaybackPath(currentTrack.id),
      keystoneService.getStartOffsetMs(currentTrack.id),
    ])
      .then(([path, offsetMs]) => {
        if (cancelled || !audioRef.current) return;
        const el2 = audioRef.current;
        el2.src = mediaUrl(path);
        const start = startOffsetFor(currentTrack, pendingStartAt, offsetMs / 1000);
        if (start > 0) el2.currentTime = start;
        readyIdRef.current = currentTrack.id;

        // Playback starts HERE, not in the transport effect below, and that is
        // the whole fix for a double-clicked row doing nothing. `play()` sets
        // `isPlaying` synchronously, so the transport effect runs in the same
        // commit as this one and reaches `el.play()` while the path is still a
        // promise — on a fresh window there is no source to start, and with a
        // track already loaded it restarts the *previous* one. Reading the
        // flag from the store rather than from the closure keeps the answer
        // current: the user may have hit pause while the path was in flight.
        if (usePlayerStore.getState().isPlaying) {
          el2.play().catch((e) => console.error('[player] play rejected:', e));
        }
      })
      .catch((e) => {
        console.error('[player] could not load track:', e);
        loadedIdRef.current = null;
      });

    return () => {
      cancelled = true;
    };
  }, [currentTrack, pendingStartAt, settings?.startAtQuarter]);

  // Transport. `stop()` rewinds; `pause()` holds.
  useEffect(() => {
    const el = audioRef.current;
    if (!el) return;
    if (isPlaying) {
      // Nothing loaded for this track yet — the load effect starts it the
      // moment the source lands. Calling play() here would start whatever the
      // element last held, which is silence on the first track and the
      // previous track on every one after it.
      if (readyIdRef.current !== currentTrack?.id) return;
      el.play().catch((e) => console.error('[player] play rejected:', e));
      return;
    }
    el.pause();
    if (stopSeq !== stopSeqRef.current) {
      stopSeqRef.current = stopSeq;
      el.currentTime = 0;
      setCurrentTime(0);
    }
  }, [isPlaying, stopSeq, setCurrentTime, currentTrack?.id]);

  return { seek };
}

/**
 * Where a newly loaded track starts.
 *
 * `defaultStart` is the backend's answer for this track, which is where
 * SPEC.md's `Start playing the track 25% of the way in` is decided — the rule
 * lives there so the setting has one implementation rather than two that can
 * disagree. A cue the user clicked beats it.
 *
 * The backstop matters either way: an offset at or past the end plays from 0
 * rather than starting a source in silence, which is indistinguishable from a
 * dead play button.
 */
export function startOffsetFor(
  track: Track,
  pendingStartAt: number,
  defaultStart: number,
): number {
  const durationS = (track.durationMs ?? 0) / 1000;
  const wanted = pendingStartAt > 0 ? pendingStartAt : defaultStart;
  if (wanted <= 0) return 0;
  return durationS > 0 && wanted >= durationS ? 0 : wanted;
}
