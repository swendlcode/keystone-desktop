import { Checkbox } from '../ui/Checkbox';
import { RadioGroup } from '../ui/Radio';
import { Toggle } from '../ui/Toggle';
import { RetagAction } from './RetagAction';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { Settings, TagWriteContent, TagWritePlacement } from '../../types';

const CONTENT: Array<{ value: TagWriteContent; label: string; hint: string }> = [
  { value: 'keyOnly', label: 'Key only', hint: '10A' },
  { value: 'energyWithWord', label: 'Energy, with the word', hint: 'Energy 7' },
  { value: 'keyAndEnergyWithWord', label: 'Key and energy, with the word', hint: '10A - Energy 7' },
  { value: 'energyOnly', label: 'Energy only', hint: '7' },
  { value: 'keyAndEnergy', label: 'Key and energy', hint: '10A - 7' },
  { value: 'keyAndTempo', label: 'Key and tempo', hint: '10A - 126' },
];

const PLACEMENT: Array<{ value: TagWritePlacement; label: string }> = [
  { value: 'frontOfArtist', label: 'In front of the artist' },
  { value: 'frontOfTitle', label: 'In front of the song name' },
  { value: 'endOfTitle', label: 'At the end of the song name' },
  { value: 'frontOfComment', label: 'In front of the comments' },
  { value: 'overwriteComment', label: 'Overwrite comments' },
];

const SAMPLE: Record<TagWriteContent, string> = {
  keyOnly: '10A',
  energyWithWord: 'Energy 7',
  keyAndEnergyWithWord: '10A - Energy 7',
  energyOnly: '7',
  keyAndEnergy: '10A - 7',
  keyAndTempo: '10A - 126',
};

/** What one re-tagged file would look like, from the two choices above. */
function preview(s: Settings): string {
  const stamp = SAMPLE[s.tagWriteContent];
  switch (s.tagWritePlacement) {
    case 'frontOfArtist':
      return `${stamp} - Lane 8`;
    case 'frontOfTitle':
      return `${stamp} - Brightest Lights`;
    case 'endOfTitle':
      return `Brightest Lights - ${stamp}`;
    case 'frontOfComment':
      return `${stamp} | existing comment`;
    case 'overwriteComment':
      return stamp;
  }
}

/**
 * SPEC.md §3 Update Tags.
 *
 * `Update common tags` is the parent of the whole pane: with it off nothing
 * below it can be reached, because every control here describes part of one
 * composed string and half of that string is not a setting anyone wants.
 */
export function UpdateTagsPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();
  const off = !settings.updateTagsEnabled;

  return (
    <SettingSection title="Update Tags">
      <SettingGroup>
        <Toggle
          checked={settings.updateTagsEnabled}
          onChange={(updateTagsEnabled) => write({ updateTagsEnabled })}
          label="Update common tags"
        />
        <SettingNote>
          Writes the key and energy into the tags your DJ software already reads.
        </SettingNote>
      </SettingGroup>

      <SettingGroup label="What to write" disabled={off}>
        <RadioGroup
          name="tagWriteContent"
          disabled={off}
          value={settings.tagWriteContent}
          options={CONTENT}
          onChange={(tagWriteContent) => write({ tagWriteContent })}
        />
      </SettingGroup>

      <SettingGroup label="Where to write it" disabled={off}>
        <RadioGroup
          name="tagWritePlacement"
          disabled={off}
          value={settings.tagWritePlacement}
          options={PLACEMENT}
          onChange={(tagWritePlacement) => write({ tagWritePlacement })}
        />
        <p className="mono mt-1 text-[12px] text-keystone-lime">{preview(settings)}</p>
      </SettingGroup>

      <AlsoGroup settings={settings} off={off} write={write} />

      <RetagAction disabled={off} />
    </SettingSection>
  );
}

/**
 * The three extra tag writes, and the two sub-options that depend on them.
 *
 * Each sub-option is indented under its parent and disabled with it, because
 * "Keep existing tempo values" is meaningless until the tempo tag is being
 * written at all — an enabled control with no effect is a bug report waiting.
 */
function AlsoGroup({
  settings,
  off,
  write,
}: {
  settings: Settings;
  off: boolean;
  write: (patch: Partial<Settings>) => void;
}) {
  return (
    <SettingGroup label="Also" disabled={off}>
      <Checkbox
        disabled={off}
        checked={settings.updateInitialKeyTag}
        onChange={(updateInitialKeyTag) => write({ updateInitialKeyTag })}
        label={'Update custom "Initial Key" tag'}
        hint="The dedicated key field Serato and Traktor read."
      />
      {/* Camelot is outside TKEY's spec-legal value set, so it is an explicit
          opt-in under the Initial Key switch rather than a second control that
          could disagree with the Key Notation setting. */}
      <Checkbox
        className="ml-6"
        disabled={off || !settings.updateInitialKeyTag}
        checked={settings.writeCamelotToInitialKey}
        onChange={(writeCamelotToInitialKey) => write({ writeCamelotToInitialKey })}
        label="Write Camelot there"
        hint="Non-standard. DJ apps accept it; some taggers mangle it."
      />

      <Checkbox
        disabled={off}
        checked={settings.writeEnergyToGrouping}
        onChange={(writeEnergyToGrouping) => write({ writeEnergyToGrouping })}
        label="Write energy level in front of the Grouping"
      />

      <Checkbox
        disabled={off}
        checked={settings.updateTempoTag}
        onChange={(updateTempoTag) => write({ updateTempoTag })}
        label={'Update "Tempo" tag'}
        hint="Whole numbers only. The decimal tempo stays in Keystone."
      />
      <Checkbox
        className="ml-6"
        disabled={off || !settings.updateTempoTag}
        checked={settings.keepExistingTempo}
        onChange={(keepExistingTempo) => write({ keepExistingTempo })}
        label="Keep existing tempo values"
      />
    </SettingGroup>
  );
}
