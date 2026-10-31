import { Add, ArrowLeft2, ArrowRight2 } from '../ui/icons';
import { useCuePoints, useSetCuePoint } from '../../hooks/useCues';
import { usePlayerStore } from '../../stores/playerStore';
import type { CuePoint } from '../../types';

/** The eight CUE slots of SPEC.md §1. Fixed, and mapped to hot cues A-H. */
const SLOTS = 8;

interface CueStepperProps {
  trackId: string;
  onSeek: (seconds: number) => void;
}

/**
 * `CUE POINTS <  >` and `ADD CUE`.
 *
 * Stepping is relative to the playhead rather than to an index, so it keeps
 * working after a marker is dragged past its neighbour — which is the normal
 * case, since the slots carry roles (intro, drop, outro) and not an order.
 */
export function CueStepper({ trackId, onSeek }: CueStepperProps) {
  const { data: cues = [] } = useCuePoints(trackId);
  const setCue = useSetCuePoint();
  const currentTime = usePlayerStore((s) => s.currentTime);

  const sorted = [...cues].sort((a, b) => a.positionMs - b.positionMs);
  const nowMs = currentTime * 1000;
  // 40 ms of slack, so stepping forward from a cue the user just jumped to
  // does not land back on the same one because playback has not moved.
  const prev = lastBefore(sorted, nowMs - 40);
  const next = sorted.find((c) => c.positionMs > nowMs + 40) ?? null;

  const freeSlot = firstFreeSlot(cues);

  return (
    <span className="flex items-center gap-1.5">
      <span className="label">Cue points</span>
      <span className="flex items-center">
        <StepButton
          label="Previous cue"
          disabled={!prev}
          onClick={() => prev && onSeek(prev.positionMs / 1000)}
        >
          <ArrowLeft2 size={13} color="currentColor" variant="Linear" />
        </StepButton>
        <StepButton
          label="Next cue"
          disabled={!next}
          onClick={() => next && onSeek(next.positionMs / 1000)}
        >
          <ArrowRight2 size={13} color="currentColor" variant="Linear" />
        </StepButton>
      </span>

      <button
        type="button"
        disabled={freeSlot == null || setCue.isPending}
        title={
          freeSlot == null
            ? 'All eight slots are in use'
            : 'Add a cue at the playhead'
        }
        onClick={() =>
          freeSlot != null &&
          setCue.mutate({
            trackId,
            slot: freeSlot,
            positionMs: nowMs,
            endMs: null,
            kind: 'user',
            label: null,
            color: null,
          })
        }
        className="label flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-gray-800 hover:text-keystone-white disabled:opacity-40 disabled:hover:bg-transparent"
      >
        <Add size={12} color="currentColor" variant="Linear" />
        Add cue
      </button>
    </span>
  );
}

function lastBefore(sorted: CuePoint[], ms: number): CuePoint | null {
  let found: CuePoint | null = null;
  for (const c of sorted) {
    if (c.positionMs >= ms) break;
    found = c;
  }
  return found;
}

/** Lowest unused slot, so CUE 1 fills before CUE 8. Null when all eight are. */
function firstFreeSlot(cues: CuePoint[]): number | null {
  const used = new Set(cues.map((c) => c.slot));
  for (let i = 0; i < SLOTS; i += 1) if (!used.has(i)) return i;
  return null;
}

function StepButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex h-6 w-6 items-center justify-center rounded text-gray-400 transition-colors hover:bg-gray-800 hover:text-keystone-white disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}
