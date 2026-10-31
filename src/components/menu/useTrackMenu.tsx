import { useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Category, Keyboard } from '../ui/icons';
import { cueKeys } from '../../hooks/useCues';
import { trackKeys } from '../../hooks/useLibrary';
import { useEffectiveSettings } from '../../hooks/useSettings';
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

    return [
      ...fileItems(ctx),
      ...tagItems(ctx),
      ...cueItems(ctx),
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
  }, [ask, openInfo, qc, refresh, settings, targets, track, write]);
}
