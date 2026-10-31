import { RadioGroup } from '../ui/Radio';
import { SettingGroup, SettingNote } from './SettingSection';
import type { RenamePattern, Settings } from '../../types';

const MODES = [
  { value: 'never', label: 'Do not rename' },
  { value: 'afterProcessing', label: 'Rename automatically after processing' },
];

const PATTERNS: Array<{ value: RenamePattern; label: string; hint: string }> = [
  { value: 'originalKey', label: 'Original - Key', hint: 'Brightest Lights - 10A' },
  {
    value: 'originalKeyTempo',
    label: 'Original - Key - Tempo',
    hint: 'Brightest Lights - 10A - 126',
  },
  { value: 'keyOriginal', label: 'Key - Original', hint: '10A - Brightest Lights' },
  {
    value: 'keyTempoOriginal',
    label: 'Key - Tempo - Original',
    hint: '10A - 126 - Brightest Lights',
  },
  {
    value: 'tempoKeyOriginal',
    label: 'Tempo - Key - Original',
    hint: '126 - 10A - Brightest Lights',
  },
];

/**
 * When to rename, and to what.
 *
 * Choosing the automatic mode does not write it: it calls `onArm`, and the
 * pane confirms first. Everything else in Settings commits on change, and this
 * is the one exception — the setting renames files from then on without asking.
 */
export function RenameOptions({
  settings,
  onArm,
  write,
}: {
  settings: Settings;
  onArm: () => void;
  write: (patch: Partial<Settings>) => void;
}) {
  return (
    <>
      <SettingGroup label="When">
        <RadioGroup
          name="renameMode"
          value={settings.renameMode}
          options={MODES}
          onChange={(v) => (v === 'afterProcessing' ? onArm() : write({ renameMode: 'never' }))}
        />
        <SettingNote tone="warn">
          Renaming moves the file. Your DJ software may lose it.
        </SettingNote>
      </SettingGroup>

      <SettingGroup label="Pattern">
        <RadioGroup
          name="renamePattern"
          value={settings.renamePattern}
          options={PATTERNS}
          onChange={(renamePattern) => write({ renamePattern })}
        />
      </SettingGroup>
    </>
  );
}
