import { Warning2 } from '../ui/icons';
import { usePlayerStore } from '../../stores/playerStore';

/**
 * Why the last attempt made no sound.
 *
 * Audio fails silently. A file the webview cannot decode, a path that moved
 * since it was indexed, and a bug in the transport all look identical from
 * the user's chair — nothing plays and the button looks dead — and they have
 * nothing in common as fixes. Everything that can stop playback writes its
 * reason here, so the answer is on screen instead of in a console nobody has
 * open.
 *
 * It renders nothing when there is nothing wrong, so it costs no space in the
 * normal case.
 */
export function PlaybackError() {
  const error = usePlayerStore((s) => s.loadError);
  if (!error) return null;

  return (
    <span className="pointer-events-none absolute bottom-1 left-1 right-1 z-40 flex items-start gap-1.5 rounded bg-gray-900/92 px-2 py-1 text-[11px] leading-snug text-amber-400">
      <Warning2
        size={12}
        color="currentColor"
        variant="Linear"
        className="mt-px shrink-0"
      />
      {/* Wraps to two lines rather than truncating. The tail of this string
          is the diagnostic — the probe result — so cutting it off removes
          the only part worth reading. */}
      <span className="line-clamp-2 break-all" title={error}>
        {error}
      </span>
    </span>
  );
}
