import { Add, Trash } from '../ui/icons';
import { Menu } from '../menu/Menu';
import { useDeleteCuePoint, useSetCuePoint } from '../../hooks/useCues';
import type { CuePoint } from '../../types';
import { formatTime } from '../../utils/formatters';
import type { MenuItem } from '../ui/menuItem';

/** SPEC.md's eight slots. A ninth cue has nowhere to live. */
const MAX_CUES = 8;

export interface WaveformMenuAnchor {
  x: number;
  y: number;
  /** Where on the track the click landed. */
  positionMs: number;
  /** The cue that was right-clicked, if the click was on one. */
  cue: CuePoint | null;
}

/**
 * Right-click on the waveform: add a cue here, or delete the one clicked.
 *
 * Adding was previously only possible through ADD CUE in the readout, which
 * drops a cue at the playhead — so placing one anywhere else meant seeking
 * there first, losing your listening position to do it. Deleting had no route
 * at all short of the per-track menu's clear-everything.
 *
 * Add is offered at the position clicked, not at the playhead: the click is
 * the user saying where.
 */
export function WaveformMenu({
  anchor,
  trackId,
  cues,
  onClose,
}: {
  anchor: WaveformMenuAnchor;
  trackId: string;
  cues: CuePoint[];
  onClose: () => void;
}) {
  const setCue = useSetCuePoint();
  const deleteCue = useDeleteCuePoint();

  const used = new Set(cues.map((c) => c.slot));
  let freeSlot = -1;
  for (let i = 0; i < MAX_CUES; i += 1) {
    if (!used.has(i)) {
      freeSlot = i;
      break;
    }
  }

  const items: MenuItem[] = [
    {
      id: 'add-cue',
      label:
        freeSlot === -1
          ? `Add Cue — all ${MAX_CUES} slots used`
          : `Add Cue ${freeSlot + 1} at ${formatTime(anchor.positionMs / 1000)}`,
      icon: <Add size={12} color="currentColor" variant="Linear" />,
      // Named rather than silently missing: "all 8 slots used" is a thing the
      // user can act on, and a greyed row with no reason reads as a bug.
      disabled: freeSlot === -1,
      onSelect: () => {
        if (freeSlot === -1) return;
        setCue.mutate({
          trackId,
          slot: freeSlot,
          positionMs: Math.round(anchor.positionMs),
          endMs: null,
          kind: null,
          label: null,
          color: null,
        });
      },
    },
  ];

  if (anchor.cue) {
    const cue = anchor.cue;
    items.push({
      id: 'delete-cue',
      label: `Delete Cue ${cue.slot + 1}`,
      icon: <Trash size={12} color="currentColor" variant="Linear" />,
      separatorBefore: true,
      danger: true,
      onSelect: () => deleteCue.mutate({ id: cue.id, trackId }),
    });
  }

  return <Menu x={anchor.x} y={anchor.y} items={items} onClose={onClose} />;
}
