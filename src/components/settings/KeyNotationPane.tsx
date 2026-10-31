import { RadioGroup } from '../ui/Radio';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { ExtraKeyColumn, KeyNotation, Settings } from '../../types';

const PRIMARY: Array<{ value: KeyNotation; label: string; hint: string }> = [
  { value: 'camelot', label: 'Camelot', hint: '11A' },
  { value: 'flats', label: 'Flats', hint: 'G♭m' },
  { value: 'sharps', label: 'Sharps', hint: 'F#m' },
];

const EXTRA: Array<{ value: ExtraKeyColumn; label: string; hint?: string }> = [
  { value: 'none', label: 'None' },
  { value: 'flats', label: 'Flats', hint: 'G♭m' },
  { value: 'sharps', label: 'Sharps', hint: 'F#m' },
];

/**
 * How a key reads, everywhere.
 *
 * The primary notation drives the table badge, the player readout and the
 * wheel's labels together — a DJ matches a badge to a ring segment by colour,
 * and two notations in one window break that. The extra column exists for the
 * other half of the habit: Camelot for mixing, note names for talking about it.
 */
export function KeyNotationPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();

  return (
    <SettingSection title="Key Notation">
      <SettingGroup label="Primary">
        <RadioGroup
          name="keyNotation"
          value={settings.keyNotation}
          options={PRIMARY}
          onChange={(keyNotation) => write({ keyNotation })}
        />
      </SettingGroup>

      <SettingGroup label="Extra key column">
        <RadioGroup
          name="extraKeyColumn"
          value={settings.extraKeyColumn}
          options={EXTRA}
          onChange={(extraKeyColumn) => write({ extraKeyColumn })}
        />
        <SettingNote>
          A second column beside the Camelot badge. Does not change what gets written to
          your files.
        </SettingNote>
      </SettingGroup>
    </SettingSection>
  );
}
