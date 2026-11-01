import { useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Category, Keyboard, Music } from '../ui/icons';
import { useAddToCollection, useCollections } from '../../hooks/useCollections';
import { cueKeys } from '../../hooks/useCues';
import { trackKeys } from '../../hooks/useLibrary';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { useLibraryStore } from '../../stores/libraryStore';
import { useUiStore } from '../../stores/uiStore';
import { useSettingsWriter } from '../settings/useSettingsWriter';
import { TARGET_FLAG, TARGET_ORDER, useExportTargets } from '../settings/useExportTargets';
import { columnsSubmenu, keySystemSubmenu } from './submenus';
import {
  cueItems,
  fileItems,
  removeItem,
  tagItems,
  type MenuContext,
} from './trackMenuItems';
import type { MenuItem } from '../ui/menuItem';
import type { ExportTarget, Track } from '../../types';

export interface ConfirmRequest {
  title: string;
  body?: string;
  confirmLabel: string;
  run: () => void;
}

/**
 * SPEC.md §4, in the spec's order and grouping.
 *
 * `ask` is how the two items that cannot be undone reach a confirmation: the
 * menu closes on select, so it cannot host the dialog itself.
 */
export function useTrackMenu(
  track: Track | null,
  ask: (r: ConfirmRequest) => void,
): MenuItem[] {
  const qc = useQueryClient();
  const settings = useEffectiveSettings();
  const write = useSettingsWriter();
  const { data: targets = [] } = useExportTargets();
  const { data: collections = [] } = useCollections();
  const addToCollection = useAddToCollection();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const openInfo = useUiStore((s) => s.openInfo);

  const refresh = useCallback(
    () => qc.invalidateQueries({ queryKey: trackKeys.all }),
    [qc],
  );

  return useMemo(() => {
    if (!track || !settings) return [];

    const liveTargets: ExportTarget[] = TARGET_ORDER.filter((t) => {
      const info = targets.find((i) => i.target === t);
      return Boolean(settings[TARGET_FLAG[t]]) && info?.supported && info.installed;
    });

    const ctx: MenuContext = {
      track,
      settings,
      liveTargets,
      refresh: () => void refresh(),
      refreshCues: () => void qc.invalidateQueries({ queryKey: cueKeys.byTrack(track.id) }),
      openInfo: () => openInfo(track.id),
      ask,
    };

    // A right-click inside the selection acts on the whole selection; outside
    // it, on that row alone. The row handlers already select an unselected
    // row before opening the menu, so by the time this runs the two cases
    // have converged — but reading the set rather than assuming it keeps the
    // menu honest if a caller ever opens it some other way.
    const targetIds = selectedIds.has(track.id) ? [...selectedIds] : [track.id];
    const fileable = collections.filter((c) => c.kind !== 'smart');

    return [
      ...fileItems(ctx),
      ...tagItems(ctx),
      ...cueItems(ctx),
      {
        id: 'add-to-collection',
        label:
          targetIds.length > 1
            ? `Add ${targetIds.length} Tracks to Collection`
            : 'Add to Collection',
        icon: <Music size={12} color="currentColor" variant="Linear" />,
        separatorBefore: true,
        // A smart collection's membership is a rule, so filing into one would
        // appear to work and then not — the same reason the sidebar rows
        // refuse a drop.
        items: fileable.length
          ? fileable.map((c) => ({
              id: `collection-${c.id}`,
              label: c.name,
              onSelect: () =>
                addToCollection.mutate({
                  collectionId: c.id,
                  trackIds: targetIds,
                }),
            }))
          : [
              {
                id: 'no-collections',
                label: 'No collections yet',
                disabled: true,
              },
            ],
      },
      {
        id: 'columns',
        label: 'Columns',
        icon: <Category size={12} color="currentColor" variant="Linear" />,
        separatorBefore: true,
        items: columnsSubmenu(settings, write),
      },
      {
        id: 'key-system',
        label: 'Key System',
        icon: <Keyboard size={12} color="currentColor" variant="Linear" />,
        items: keySystemSubmenu(settings, write),
      },
      removeItem(ctx),
    ];
  }, [
    addToCollection,
    ask,
    collections,
    openInfo,
    qc,
    refresh,
    selectedIds,
    settings,
    targets,
    track,
    write,
  ]);
}
