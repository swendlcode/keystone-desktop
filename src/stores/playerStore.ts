import { create } from 'zustand';
import type { Track } from '../types';

/**
 * The transport.
 *
 * `stop()` and `pause()` both land on `isPlaying: false`, so `stopSeq` is how
 * the player hook tells them apart. This is not a hypothetical: in Stack,
 * collapsing the two meant every stop recorded the playhead as a resume point,
 * a few quick presses walked that point to the end of a short file, and the
 * play button then looked dead because every press started a source in
 * silence. Keystone's tracks are minutes long rather than seconds, but the
 * bug is the same shape and the fix costs one integer.
 *
 * `pause()` must keep resuming mid-track: it is the spacebar.
 */
interface PlayerStore {
  currentTrack: Track | null;
  /** The current page's rows, so previous / next work. Set by the table. */
  playlist: Track[];
  isPlaying: boolean;
  /** Seconds. */
  currentTime: number;
  duration: number;
  volume: number;
  /** Where the next load should start, in seconds. Set by `play(t, startAt)`;
      the `Start playing 25% of the way in` setting writes it. */
  pendingStartAt: number;
  /** Bumped by `stop()` only. Compared against a ref in `usePlayer`. */
  stopSeq: number;
  /** Zoom index into `Waveform.levels`. The player's - / + buttons. */
  zoom: number;
  /**
   * Centre of the visible waveform window, in seconds, or null to follow the
   * playhead. Written by a scroll across the waveform; cleared when playback
   * leaves the window, so a manual pan holds until it stops being useful.
   */
  viewCentreS: number | null;
  /** Which cue slot the user is dragging, or null. Drives the marker hit test. */
  draggingCueSlot: number | null;

  play: (track: Track, startAt?: number) => void;
  pause: () => void;
  resume: () => void;
  stop: () => void;
  playNext: () => void;
  playPrev: () => void;
  setPlaylist: (tracks: Track[]) => void;
  setCurrentTime: (time: number) => void;
  setDuration: (duration: number) => void;
  setVolume: (volume: number) => void;
  setZoom: (zoom: number) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  setViewCentre: (seconds: number | null) => void;
  setDraggingCueSlot: (slot: number | null) => void;
  clearCurrent: () => void;

  /** Registered by `usePlayer`, which owns the audio element. */
  seekTo: ((time: number) => void) | null;
  registerSeek: (fn: (time: number) => void) => void;
}

/**
 * 128x. Four steps were enough to look at a drop and not enough to place a cue
 * on it: on a five-minute track 16x still puts twenty seconds across the
 * canvas, which is about 90 ms per pixel. At 128x it is a little over two
 * seconds, so a cue can be dragged onto a transient rather than near it.
 */
const MAX_ZOOM = 7;
const VOLUME_KEY = 'keystone:volume';

/**
 * Volume is client state, persisted here rather than in `Settings`.
 *
 * It has to be known before the first track loads, and a settings round trip
 * does not beat the user double-clicking a row — so the first audition would
 * play at the hardcoded default instead of theirs.
 */
function loadVolume(): number {
  try {
    const v = localStorage.getItem(VOLUME_KEY);
    if (v !== null) {
      const n = Number(v);
      if (Number.isFinite(n) && n >= 0 && n <= 1) return n;
    }
  } catch {
    /* storage blocked — fall through */
  }
  return 0.9;
}

export const usePlayerStore = create<PlayerStore>((set, get) => ({
  currentTrack: null,
  playlist: [],
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  volume: loadVolume(),
  pendingStartAt: 0,
  stopSeq: 0,
  zoom: 0,
  viewCentreS: null,
  draggingCueSlot: null,
  seekTo: null,

  play: (track, startAt = 0) =>
    set({
      currentTrack: track,
      isPlaying: true,
      currentTime: startAt,
      duration: 0,
      pendingStartAt: startAt,
      viewCentreS: null,
    }),

  /** Hold the playhead. The spacebar and the transport's pause. */
  pause: () => set({ isPlaying: false }),

  resume: () => set({ isPlaying: true }),

  /** Back to the start. See the note on `stopSeq` above. */
  stop: () =>
    set((s) => ({ isPlaying: false, currentTime: 0, stopSeq: s.stopSeq + 1 })),

  playNext: () => {
    const { currentTrack, playlist } = get();
    if (!playlist.length) return;
    const i = currentTrack
      ? playlist.findIndex((t) => t.id === currentTrack.id)
      : -1;
    const next = playlist[i + 1];
    if (next) {
      set({
        currentTrack: next,
        isPlaying: true,
        currentTime: 0,
        duration: 0,
        pendingStartAt: 0,
      });
    }
  },

  playPrev: () => {
    const { currentTrack, playlist, currentTime, seekTo } = get();
    if (!playlist.length) return;
    // More than 3 s in, previous restarts the track — the DJ-deck convention.
    if (currentTime > 3 && seekTo) {
      seekTo(0);
      return;
    }
    const i = currentTrack
      ? playlist.findIndex((t) => t.id === currentTrack.id)
      : 1;
    const prev = playlist[i - 1];
    if (prev) {
      set({
        currentTrack: prev,
        isPlaying: true,
        currentTime: 0,
        duration: 0,
        pendingStartAt: 0,
      });
    }
  },

  setPlaylist: (playlist) => set({ playlist }),
  setCurrentTime: (currentTime) => set({ currentTime }),
  setDuration: (duration) => set({ duration }),
  setVolume: (volume) => {
    const clamped = Math.max(0, Math.min(1, volume));
    try {
      localStorage.setItem(VOLUME_KEY, String(clamped));
    } catch {
      /* unpersisted, still applied */
    }
    set({ volume: clamped });
  },
  setZoom: (zoom) => set({ zoom: Math.max(0, Math.min(MAX_ZOOM, zoom)) }),
  // The buttons re-centre on the playhead. Only a scroll pans, and a user who
  // reaches for the toolbar after scrolling away has asked to be brought back.
  zoomIn: () =>
    set((s) => ({ zoom: Math.min(MAX_ZOOM, s.zoom + 1), viewCentreS: null })),
  zoomOut: () =>
    set((s) => ({ zoom: Math.max(0, s.zoom - 1), viewCentreS: null })),
  setViewCentre: (viewCentreS) => set({ viewCentreS }),
  setDraggingCueSlot: (draggingCueSlot) => set({ draggingCueSlot }),
  clearCurrent: () =>
    set({ currentTrack: null, isPlaying: false, currentTime: 0, duration: 0 }),

  registerSeek: (seekTo) => set({ seekTo }),
}));

export { MAX_ZOOM };
