import { Input } from '../ui/Input';
import { RadioGroup } from '../ui/Radio';
import { Toggle } from '../ui/Toggle';
import { useSettingsStore } from '../../stores/settingsStore';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { Settings, TempoDecimals } from '../../types';

const DECIMALS: Array<{ value: string; label: string; hint: string }> = [
  { value: '0', label: 'None', hint: '128' },
  { value: '1', label: 'One', hint: '128.1' },
  { value: '2', label: 'Two', hint: '128.14' },
];

/**
 * The displayed tempo range, and how many decimals it shows.
 *
 * Changing the range does not re-analyse anything: `tempoRaw` keeps the
 * detected value and `useUpdateSettings` re-folds `tempoDisplay` from it, which
 * is the whole reason the two columns exist.
 */
export function TempoPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();
  const patchDraft = useSettingsStore((s) => s.patchDraft);
  const off = !settings.restrictTempoRange;

  return (
    <SettingSection title="Tempo">
      <SettingGroup>
        <Toggle
          checked={settings.restrictTempoRange}
          onChange={(restrictTempoRange) => write({ restrictTempoRange })}
          label="Restrict displayed tempo to this range"
        />
        <SettingNote>
          Anything outside is doubled or halved, not reported wrong.
        </SettingNote>

        <div className={`mt-2 flex gap-3 ${off ? 'pointer-events-none opacity-40' : ''}`}>
          <BpmField
            label="Lowest"
            value={settings.tempoMin}
            disabled={off}
            onType={(tempoMin) => patchDraft({ tempoMin })}
            onCommit={(tempoMin) => write({ tempoMin })}
          />
          <BpmField
            label="Highest"
            value={settings.tempoMax}
            disabled={off}
            onType={(tempoMax) => patchDraft({ tempoMax })}
            onCommit={(tempoMax) => write({ tempoMax })}
          />
        </div>
      </SettingGroup>

      <SettingGroup label="Decimal places">
        <RadioGroup
          name="tempoDecimals"
          value={String(settings.tempoDecimals)}
          options={DECIMALS}
          onChange={(v) => write({ tempoDecimals: Number(v) as TempoDecimals })}
        />
        <SettingNote>
          Two places only show where the beat grid fit them; otherwise one.
        </SettingNote>
      </SettingGroup>
    </SettingSection>
  );
}

/**
 * A BPM field that echoes while typing and saves on blur.
 *
 * Committing per keystroke would send `tempoMin: 7` on the way to `79` and
 * re-fold the whole library's displayed tempo against it.
 */
function BpmField({
  label,
  value,
  disabled,
  onType,
  onCommit,
}: {
  label: string;
  value: number;
  disabled: boolean;
  onType: (value: number) => void;
  onCommit: (value: number) => void;
}) {
  return (
    <label className="flex-1">
      <span className="label mb-1 block">{label}</span>
      <Input
        numeric
        type="number"
        min={40}
        max={300}
        disabled={disabled}
        value={String(value)}
        trailing={<span className="mono text-[11px] text-gray-500">BPM</span>}
        onChange={(e) => onType(Number(e.target.value))}
        onBlur={(e) => onCommit(Number(e.target.value))}
        onKeyDown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
        }}
      />
    </label>
  );
}
