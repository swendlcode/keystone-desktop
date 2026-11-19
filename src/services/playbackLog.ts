import { keystoneService } from './keystoneService';

/**
 * The player's flight recorder.
 *
 * `PlaybackError` already puts the reason for a *failed* attempt on screen,
 * which covers everything the element is willing to report. Two outcomes get
 * past it and leave nothing at all.
 *
 * A press that makes no sound and raises no error: the load chain in
 * `usePlayer` is an unbounded `fetch` of an `asset://` URL, and a fetch that
 * parks resolves neither the `.then` nor the `.catch`, so `loadedIdRef` stays
 * pointing at a track that will never become ready and the play button is
 * dead for that row until another one is chosen. Nothing in the app notices.
 *
 * And a press that does make sound, eventually: the element is handed the
 * whole compressed file after two IPC round trips, so the wait is real and
 * nothing records which part of it was slow — the path lookup, the bytes, or
 * the element.
 *
 * Both are escaped by restarting, and the restart is what loses the evidence,
 * so the ring is in memory, costs a few dozen small objects, and is written
 * to the log file automatically rather than on request. Asking someone to
 * reproduce a fault and copy a log before they restart is asking them to
 * debug while they are trying to work.
 */

/** Press to sound, above which an audition feels broken rather than slow. */
const SLOW_START_MS = 1200;
/**
 * Press to sound, above which there is no sound.
 *
 * Deliberately generous. The element gets the whole compressed file before it
 * can start, so a cold read of a long 320kbps track is seconds and a tighter
 * bound would dump a log every time someone auditions a big file. What this
 * catches is not slow but *never*.
 */
const NO_SOUND_MS = 4000;
/** One dump per episode. Fifty presses in a bad minute are not fifty dumps. */
const DUMP_THROTTLE_MS = 10_000;
/**
 * Half a dozen events per press, so this holds the last dozen attempts —
 * enough to carry the comparison every report makes: the one before was fine.
 */
const RING_MAX = 80;

type Entry = { t: number; event: string; data?: Record<string, unknown> };

const ring: Entry[] = [];
let pressedAt = 0;
let answered = true;
let silenceTimer: number | null = null;
// Negative infinity, not 0: performance.now() starts near zero, so a zero
// here would swallow every dump in the app's first ten seconds — which is
// exactly when a cold first press is most likely to go wrong.
let lastDumpAt = -Infinity;

/** Add an event. Never throws and never awaits: it runs on the play path. */
export function plog(event: string, data?: Record<string, unknown>): void {
  ring.push({ t: Math.round(performance.now()), event, data });
  if (ring.length > RING_MAX) ring.shift();
}

/** Milliseconds since the press still waiting for sound, or -1. */
function sincePress(): number {
  return pressedAt > 0 ? Math.round(performance.now() - pressedAt) : -1;
}

/** A step on the way to sound, timed from the press rather than from itself. */
export function noteStep(event: string, data?: Record<string, unknown>): void {
  plog(event, { ...data, ms: sincePress() });
}

/**
 * The user asked for sound, now.
 *
 * Called from the store, which is the press instant: `play()` and `resume()`
 * run synchronously inside the double-click, click and keydown handlers that
 * cause them. Measuring from inside the play path instead would record 0ms
 * for a start the user waited a second for, because by that reckoning it was
 * instant — it was asked late.
 */
export function notePress(trackId: string | null, via: string): void {
  pressedAt = performance.now();
  answered = false;
  plog('press', { trackId, via });
  if (silenceTimer !== null) clearTimeout(silenceTimer);
  silenceTimer = window.setTimeout(() => {
    silenceTimer = null;
    if (answered) return;
    plog('silent', { trackId, ms: sincePress() });
    dump('no sound');
  }, NO_SOUND_MS);
}

/** Sound actually began. Closes the press and times the whole wait. */
export function noteAudible(): void {
  const ms = sincePress();
  answered = true;
  if (silenceTimer !== null) {
    clearTimeout(silenceTimer);
    silenceTimer = null;
  }
  plog('audible', { ms });
  if (ms >= SLOW_START_MS) dump(`slow start ${ms}ms`);
}

/**
 * The user stopped wanting sound.
 *
 * Without this, pausing while a long file is still loading looks exactly like
 * the silence the watchdog exists to catch, and every such pause would write
 * a log naming a fault that did not happen.
 */
export function noteStopped(reason: string): void {
  answered = true;
  if (silenceTimer !== null) {
    clearTimeout(silenceTimer);
    silenceTimer = null;
  }
  plog('stopped', { reason, ms: sincePress() });
}

/** The ring, oldest first, as plain text. */
export function formatLog(): string {
  return ring
    .map((e) => `${e.t} ${e.event}${e.data ? ' ' + JSON.stringify(e.data) : ''}`)
    .join('\n');
}

/** Write the ring to the log file. Throttled; failures are swallowed. */
export function dump(reason: string): void {
  const now = performance.now();
  if (now - lastDumpAt < DUMP_THROTTLE_MS) return;
  lastDumpAt = now;
  void keystoneService
    .logClient(`playback ${reason}\n${formatLog()}`)
    .catch(() => {});
}
