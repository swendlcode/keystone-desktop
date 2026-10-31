import { Checkbox } from '../ui/Checkbox';
import { RadioGroup } from '../ui/Radio';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { Settings } from '../../types';

const LOAD_BEHAVIOUR = [
  { value: 'auto', label: 'Play automatically when loaded' },
  { value: 'click', label: 'Require a click' },
];

/** Auditioning behaviour. */
export function AudioPlayerPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();

  return (
    <SettingSection title="Audio Player">
      <SettingGroup label="On load">
        <RadioGroup
          name="playAutomatically"
          value={settings.playAutomatically ? 'auto' : 'click'}
          options={LOAD_BEHAVIOUR}
          onChange={(v) => write({ playAutomatically: v === 'auto' })}
        />
        <SettingNote>
          Require a click when other DJ software is holding the audio device.
        </SettingNote>
      </SettingGroup>

      <SettingGroup>
        <Checkbox
          checked={settings.startAtQuarter}
          onChange={(startAtQuarter) => write({ startAtQuarter })}
          label="Start playing the track 25% of the way in"
          hint="Past the intro, into the part you are matching."
        />
        <Checkbox
          checked={settings.cacheWaveformDuringAnalysis}
          onChange={(cacheWaveformDuringAnalysis) => write({ cacheWaveformDuringAnalysis })}
          label="Cache audio wave image during analysis"
          hint="Slower analysis, instant waveforms afterwards."
        />
      </SettingGroup>
    </SettingSection>
  );
}
