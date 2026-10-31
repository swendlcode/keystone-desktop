import { createElement } from 'react';
import { TickCircle } from '../ui/icons';
import {
  COLUMN_LABELS,
  COLUMN_ORDER,
  type KeyNotation,
  type Settings,
  type TrackColumn,
} from '../../types';
import type { MenuItem } from '../ui/menuItem';

/** Title and Key result are not optional — see DisplayOptionsPane. */
const ALWAYS_ON: TrackColumn[] = ['title', 'keyResult'];

/** A tick in the icon slot: `MenuItem` has no checked flag, and adding one
    would mean editing the shared type. */
function tick(on: boolean) {
  return on
    ? createElement(TickCircle, { size: 12, color: 'currentColor', variant: 'Bold' })
    : undefined;
}

/**
 * The Columns submenu — the same visibility flags as Display Options.
 *
 * Deliberately the same `Settings.columns` object rather than a per-view copy:
 * a DJ who hides Bitrate from the context menu has hidden it, and finding it
 * back the next time they open Settings would read as a bug.
 */
export function columnsSubmenu(
  settings: Settings,
  write: (patch: Partial<Settings>) => void,
): MenuItem[] {
  return COLUMN_ORDER.filter((c) => !ALWAYS_ON.includes(c)).map((c) => ({
    id: `col-${c}`,
    label: COLUMN_LABELS[c],
    icon: tick(settings.columns[c]),
    onSelect: () =>
      write({ columns: { ...settings.columns, [c]: !settings.columns[c] } }),
  }));
}

const NOTATIONS: Array<{ value: KeyNotation; label: string }> = [
  { value: 'camelot', label: 'Camelot' },
  { value: 'flats', label: 'Flats' },
  { value: 'sharps', label: 'Sharps' },
];

export function keySystemSubmenu(
  settings: Settings,
  write: (patch: Partial<Settings>) => void,
): MenuItem[] {
  return NOTATIONS.map((n) => ({
    id: `key-${n.value}`,
    label: n.label,
    icon: tick(settings.keyNotation === n.value),
    onSelect: () => write({ keyNotation: n.value }),
  }));
}
