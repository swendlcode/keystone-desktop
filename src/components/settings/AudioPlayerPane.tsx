import { Checkbox } from '../ui/Checkbox';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { useSettingsWriter } from './useSettingsWriter';
import type { Settings } from '../../types';

/** Auditioning behaviour. */
export function AudioPlayerPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();

  return (
    <SettingSection title="Audio Player">
      {/* The radio that used to sit here chose whether selecting a row also
          auditioned it. Nothing selects-and-plays any more — a double-click
          plays and a click only selects — so the choice had no behaviour
          left behind it. A control that switches nothing is worse than no
          control. The field stays in Settings for the backend; only the
          switch is gone. */}
      <SettingGroup label="Auditioning">
        <SettingNote>
          Double-click a track to play it. A single click only selects, so
          moving through the list never takes the audio device from other DJ
          software.
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
