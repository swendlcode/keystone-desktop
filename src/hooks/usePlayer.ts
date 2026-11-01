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

    /**
     * The backstop that makes the play button impossible to deadlock.
     *
     * Everything else here starts playback at a moment some ref has to be
     * right about — when the path resolves, when the transport effect runs.
     * This one asks the only question that actually matters: the element can
     * play, and the store says it should be playing, so play. It fires on
     * every load and after every seek, so no bookkeeping slip can leave the
     * transport stuck with a loaded file and a dead button.
     */
    const onCanPlay = () => {
      if (!usePlayerStore.getState().isPlaying) return;
      if (!el.paused) return;
      void beginPlayback(el);
    };

    /**
     * The element gave up on the source.
     *
     * Named rather than logged. A file the webview cannot decode and a bug in
     * the transport are the same thing from the user's chair — no sound and a
     * button that appears dead — and the two have completely different fixes.
     */
    const onError = () => {
      usePlayerStore
        .getState()
        .setLoadError(`${describeMediaError(el)} — ${srcHint(el)}`);
      usePlayerStore.getState().pause();
    };

    el.addEventListener('timeupdate', onTime);
    el.addEventListener('loadedmetadata', onMeta);
    el.addEventListener('ended', onEnded);
    el.addEventListener('canplay', onCanPlay);
    el.addEventListener('error', onError);

    return () => {
      el.removeEventListener('timeupdate', onTime);
      el.removeEventListener('loadedmetadata', onMeta);
      el.removeEventListener('ended', onEnded);
      el.removeEventListener('canplay', onCanPlay);
      el.removeEventListener('error', onError);
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

    loadedIdRef.current = currentTrack.id;
    readyIdRef.current = null;
    const loadingId = currentTrack.id;

    // Both at once: the offset is a database read, and serialising it behind the
    // path read would delay the first frame of audio for no reason.
    Promise.all([
      keystoneService.getPlaybackPath(currentTrack.id),
      keystoneService.getStartOffsetMs(currentTrack.id),
    ])
      .then(([path, offsetMs]) => {
        // Keyed on the track id, NOT on a `cancelled` flag set by cleanup.
        // This effect depends on `currentTrack`, which is a fresh object
        // every time react-query patches the row — and the analysis queue
        // patches rows constantly. Cleanup would mark the in-flight load
        // cancelled, the effect would re-run and early-return because the id
        // had not changed, and `readyIdRef` would never be set: a play button
        // that was permanently dead for that track. Asking whether we are
        // still loading the same track survives any number of re-runs.
        if (loadedIdRef.current !== loadingId || !audioRef.current) return;
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
        if (usePlayerStore.getState().isPlaying) void beginPlayback(el2);
      })
      .catch((e) => {
        usePlayerStore
          .getState()
          .setLoadError(`Could not find the file: ${String(e)}`);
        // Clear it so the next render retries rather than sitting on a track
        // that is marked loaded and has no source.
        if (loadedIdRef.current === loadingId) loadedIdRef.current = null;
      });
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
      void beginPlayback(el);
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
 * One place that calls `play()`, so one place that reports why it did not.
 *
 * A rejected `play()` is the other half of the silent-failure problem: the
 * promise rejects, the element stays paused, and without this the only trace
 * is a console line. `NotAllowedError` in particular is the webview refusing
 * autoplay, which is a different problem from a file it cannot read, and the
 * message is the only thing that tells them apart.
 */
async function beginPlayback(el: HTMLAudioElement): Promise<void> {
  try {
    await el.play();
    usePlayerStore.getState().setLoadError(null);
  } catch (e) {
    const err = e as { name?: string; message?: string };
    const store = usePlayerStore.getState();
    store.pause();

    if (err.name === 'NotAllowedError') {
      store.setLoadError('The webview blocked playback. Press play again.');
      return;
    }

    // `NotSupportedError` is the ambiguous one: WebKit raises it both for a
    // codec it will not decode and for a source it could not fetch at all.
    // Rather than ask the user to read a console, probe the URL — a status
    // code means the asset protocol is serving the bytes and the codec is at
    // fault, and a throw means the protocol never delivered them.
    store.setLoadError(
      `Playback failed: ${err.name ?? ''} ${err.message ?? String(e)} — ${srcHint(el)}`.trim(),
    );
    const reach = await probeSource(el.src);
    usePlayerStore
      .getState()
      .setLoadError(
        `${err.name ?? 'Playback failed'} · ${srcHint(el)} · ${reach}`,
      );
    // The whole string, untruncated, into the log that already has the Rust
    // half of this story.
    void keystoneService
      .logClient(
        `playback failed: ${err.name} ${err.message} | src=${el.src} | ${reach} | ` +
          `readyState=${el.readyState} networkState=${el.networkState} ` +
          `mediaError=${el.error?.code ?? 'none'} canPlayMp3=${el.canPlayType('audio/mpeg') || 'no'}`,
      )
      .catch(() => {});
  }
}

/**
 * Can the webview fetch the source at all?
 *
 * One range request for two bytes. The answer separates the two causes of
 * `NotSupportedError` without a second round trip through the user.
 */
async function probeSource(src: string): Promise<string> {
  if (!src) return 'no source';
  try {
    const res = await fetch(src, { headers: { Range: 'bytes=0-1' } });
    const type = res.headers.get('content-type') ?? 'no content-type';
    return `fetch ${res.status} ${type}`;
  } catch (e) {
    return `fetch threw: ${(e as Error)?.message ?? String(e)}`;
  }
}

/**
 * The tail of the URL that failed, and the extension.
 *
 * `SRC_NOT_SUPPORTED` has two completely different causes — a codec the
 * webview will not decode, and a URL it could not fetch at all — and the two
 * are indistinguishable from the code alone. The extension settles the first
 * and seeing the actual asset URL settles the second, so the message carries
 * both rather than making it a guess.
 */
function srcHint(el: HTMLAudioElement): string {
  if (!el.src) return 'no source was set';
  let decoded = el.src;
  try {
    decoded = decodeURIComponent(el.src);
  } catch {
    /* leave it encoded; it is a hint either way */
  }
  const ext = decoded.split('.').pop()?.split(/[?#]/)[0] ?? '';
  const scheme = decoded.slice(0, decoded.indexOf('/', decoded.indexOf('//') + 2));
  const tail = decoded.length > 60 ? `…${decoded.slice(-60)}` : decoded;
  return `.${ext} · ${scheme || '?'} · ${tail}`;
}

/** A MediaError code as something a person can act on. */
function describeMediaError(el: HTMLAudioElement): string {
  const code = el.error?.code;
  const detail = el.error?.message ? ` (${el.error.message})` : '';
  switch (code) {
    case MediaError.MEDIA_ERR_ABORTED:
      return `Loading was aborted${detail}`;
    case MediaError.MEDIA_ERR_NETWORK:
      return `The file could not be read from disk${detail}`;
    case MediaError.MEDIA_ERR_DECODE:
      return `The file is corrupt or uses an unsupported encoding${detail}`;
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      // The common one, and it has two very different causes.
      return `This format cannot be played by the webview, or the file is missing${detail}`;
    default:
      return `The file could not be played${detail}`;
  }
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
