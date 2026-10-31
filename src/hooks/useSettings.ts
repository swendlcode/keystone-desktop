import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import { useSettingsStore } from '../stores/settingsStore';
import { trackKeys } from './useLibrary';
import type { Settings } from '../types';

export const settingsKeys = {
  all: ['settings'] as const,
};

export function useSettings() {
  return useQuery({
    queryKey: settingsKeys.all,
    queryFn: () => keystoneService.getSettings(),
    staleTime: Infinity,
  });
}

/**
 * The saved settings with any unsaved draft merged over them.
 *
 * Controls read this so typing in a field is visible before the save lands.
 * Null until the first fetch, which is a section's cue to render nothing
 * rather than render against invented defaults — the real defaults live in
 * Rust's `impl Default for Settings` and there must be only one copy of them.
 */
export function useEffectiveSettings(): Settings | null {
  const { data } = useSettings();
  const draft = useSettingsStore((s) => s.draft);
  if (!data) return null;
  return { ...data, ...draft };
}

export function useUpdateSettings() {
  const qc = useQueryClient();
  const clearDraft = useSettingsStore((s) => s.clearDraft);

  return useMutation({
    mutationFn: (settings: Settings) =>
      keystoneService.updateSettings(settings),
    onSuccess: (saved, sent) => {
      qc.setQueryData(settingsKeys.all, saved);
      clearDraft();

      // The Tempo range changes what the table shows without changing any
      // analysis, so re-fold `tempoDisplay` from `tempoRaw` instead of
      // re-reading files. That separation is the whole reason both columns
      // exist, and it is also why this is a mutation side effect rather than
      // something the Settings page has to remember to call.
      const previous = qc.getQueryData<Settings>(settingsKeys.all);
      const rangeChanged =
        !previous ||
        previous.restrictTempoRange !== sent.restrictTempoRange ||
        previous.tempoMin !== sent.tempoMin ||
        previous.tempoMax !== sent.tempoMax;
      if (rangeChanged) {
        keystoneService
          .refoldTempoDisplay()
          .then(() => qc.invalidateQueries({ queryKey: trackKeys.all }))
          .catch((e) => console.error('[settings] tempo re-fold failed:', e));
      }
    },
  });
}
