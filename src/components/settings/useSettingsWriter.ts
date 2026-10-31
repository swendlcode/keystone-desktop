import { useCallback } from 'react';
import { useEffectiveSettings, useUpdateSettings } from '../../hooks/useSettings';
import type { Settings } from '../../types';

/**
 * Write one or more settings fields through, immediately.
 *
 * There is no Save button anywhere in Settings: every control commits on
 * change, so switching pane or closing the window cannot lose a choice. Text
 * and number fields are the exception — they echo into `settingsStore.draft`
 * while being typed and commit on blur, because committing per keystroke would
 * send `tempoMin: 7` on the way to `79`.
 *
 * It spreads `useEffectiveSettings()` rather than the saved object, and is the
 * only writer in the app for that reason: a half-typed BPM range sitting in the
 * draft would otherwise be thrown away by the next toggle in the same pane.
 */
export function useSettingsWriter(): (patch: Partial<Settings>) => void {
  const settings = useEffectiveSettings();
  const update = useUpdateSettings();

  return useCallback(
    (patch: Partial<Settings>) => {
      // Null until the first fetch lands. Writing then would persist Rust's
      // defaults as if the user had chosen them.
      if (!settings) return;
      update.mutate({ ...settings, ...patch });
    },
    [settings, update],
  );
}
