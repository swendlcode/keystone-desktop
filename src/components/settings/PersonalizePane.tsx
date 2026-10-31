import { Moon, Sun1 } from '../ui/icons';
import { Select } from '../ui/Select';
import { useSettingsStore } from '../../stores/settingsStore';
import { SettingGroup, SettingNote, SettingRow, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { Settings, Theme } from '../../types';

const THREADS = [0, 1, 2, 4, 6, 8, 12, 16];

/** Appearance and how hard the machine works. */
export function PersonalizePane({ settings }: { settings: Settings }) {
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const write = useSettingsWriter();

  return (
    <SettingSection title="Personalize">
      <ThemeGroup theme={theme} setTheme={setTheme} />

      <SettingGroup label="Performance">
        <SettingRow label="Scan threads" hint="Walking folders.">
          <Select
            className="w-40"
            aria-label="Scan threads"
            value={String(settings.scannerConcurrency)}
            options={THREADS.map((n) => ({
              value: String(n),
              label: n === 0 ? 'Automatic' : `${n}`,
            }))}
            onChange={(v) => write({ scannerConcurrency: Number(v) })}
          />
        </SettingRow>
        <SettingRow label="Analysis threads" hint="Key, tempo and energy.">
          <Select
            className="w-40"
            aria-label="Analysis threads"
            value={String(settings.analysisConcurrency)}
            options={THREADS.map((n) => ({
              value: String(n),
              label: n === 0 ? 'Automatic' : `${n}`,
            }))}
            onChange={(v) => write({ analysisConcurrency: Number(v) })}
          />
        </SettingRow>
        <SettingNote>Automatic leaves a core free so the app stays responsive.</SettingNote>
      </SettingGroup>
    </SettingSection>
  );
}

function ThemeGroup({
  theme,
  setTheme,
}: {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}) {
  return (
    <SettingGroup label="Theme">
      <div className="flex gap-2">
        {(['dark', 'light'] as Theme[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTheme(t)}
            className={`flex h-9 flex-1 items-center justify-center gap-2 rounded-md border text-xs capitalize transition-colors ${
              theme === t
                ? 'border-keystone-lime bg-keystone-lime/10 text-keystone-lime'
                : 'border-gray-600 text-gray-300 hover:border-gray-500 hover:text-keystone-white'
            }`}
          >
            {t === 'dark' ? (
              <Moon size={13} color="currentColor" variant="Linear" />
            ) : (
              <Sun1 size={13} color="currentColor" variant="Linear" />
            )}
            {t}
          </button>
        ))}
      </div>
      {/* The theme is the one setting the backend does not hold: it has to be
          readable before the first paint, which a SQLite round trip cannot
          beat. It is in localStorage instead. */}
      <SettingNote>Applies at once. Kept locally, not in your library.</SettingNote>
    </SettingGroup>
  );
}
