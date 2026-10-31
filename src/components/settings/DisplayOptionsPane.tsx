import { Checkbox } from '../ui/Checkbox';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import {
  COLUMN_LABELS,
  COLUMN_ORDER,
  type Settings,
  type TrackColumn,
} from '../../types';

/**
 * Title and Key result are not in SPEC.md's toggle list and are not offered
 * here: a library table with no title and no key is not a view of anything.
 */
const ALWAYS_ON: TrackColumn[] = ['title', 'keyResult'];
const TOGGLEABLE = COLUMN_ORDER.filter((c) => !ALWAYS_ON.includes(c));

export function DisplayOptionsPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();

  const setColumn = (column: TrackColumn, visible: boolean) =>
    write({ columns: { ...settings.columns, [column]: visible } });

  return (
    <SettingSection title="Display Options">
      <SettingGroup label="Columns">
        <div className="grid grid-cols-2 gap-x-6 gap-y-2">
          {TOGGLEABLE.map((c) => (
            <Checkbox
              key={c}
              checked={settings.columns[c]}
              onChange={(v) => setColumn(c, v)}
              label={COLUMN_LABELS[c]}
            />
          ))}
        </div>
        <SettingNote>Title and Key result are always shown.</SettingNote>
      </SettingGroup>

      <SettingGroup label="Key">
        <Checkbox
          checked={settings.padCamelotNumbers}
          onChange={(padCamelotNumbers) => write({ padCamelotNumbers })}
          label="Add a zero to single-digit Camelot numbers"
          hint="08A rather than 8A, so the column lines up."
        />
      </SettingGroup>

      <SettingGroup label="Waveform">
        <Checkbox
          checked={settings.showDetectedCuePoints}
          onChange={(showDetectedCuePoints) => write({ showDetectedCuePoints })}
          label="Show detected cue points"
        />
        <Checkbox
          checked={settings.showDetectedEnergyLevels}
          onChange={(showDetectedEnergyLevels) => write({ showDetectedEnergyLevels })}
          label="Show detected energy levels"
        />
      </SettingGroup>

      <SettingGroup label="Quality">
        {/* Keystone flags; it never corrects. Clip repair and loudness work is
            Capstone's job, and this flag is the handoff. */}
        <Checkbox
          checked={settings.showImprovableMessage}
          onChange={(showImprovableMessage) => write({ showImprovableMessage })}
          label="Display a message if the file can be improved"
          hint="Clipping and low-confidence results. Fixing them is Capstone's job."
        />
      </SettingGroup>
    </SettingSection>
  );
}
