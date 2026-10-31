import { AboutPane } from '../components/settings/AboutPane';
import { AudioPlayerPane } from '../components/settings/AudioPlayerPane';
import { DisplayOptionsPane } from '../components/settings/DisplayOptionsPane';
import { ExportCuePointsPane } from '../components/settings/ExportCuePointsPane';
import { FeedbackPane } from '../components/settings/FeedbackPane';
import { KeyNotationPane } from '../components/settings/KeyNotationPane';
import { PersonalizePane } from '../components/settings/PersonalizePane';
import { RenameFilesPane } from '../components/settings/RenameFilesPane';
import { SettingsRail } from '../components/settings/SettingsRail';
import { TempoPane } from '../components/settings/TempoPane';
import { UpdateTagsPane } from '../components/settings/UpdateTagsPane';
import { ScrollArea } from '../components/ui/ScrollArea';
import { useEffectiveSettings } from '../hooks/useSettings';
import { useSettingsStore } from '../stores/settingsStore';

/**
 * SPEC.md §3 — the left rail and one pane at a time.
 *
 * Nothing here has a Save button: every control commits on change, so leaving
 * the pane, switching view or closing the window cannot lose a choice. Numeric
 * fields echo through `settingsStore.draft` while being typed and commit on
 * blur, which is the one place a keystroke is not a write.
 *
 * `useEffectiveSettings()` is null until the first fetch lands, and the panes
 * render nothing rather than render against invented defaults — the real
 * defaults live in Rust's `impl Default for Settings` and there is exactly one
 * copy of them.
 */
export function SettingsPage() {
  const section = useSettingsStore((s) => s.section);
  const settings = useEffectiveSettings();

  return (
    <div className="flex min-h-0 flex-1">
      <SettingsRail />
      <ScrollArea className="min-w-0 flex-1">
        <div className="mx-auto max-w-2xl p-6">
          {settings && (
            <>
              {section === 'personalize' && <PersonalizePane settings={settings} />}
              {section === 'updateTags' && <UpdateTagsPane settings={settings} />}
              {section === 'exportCuePoints' && <ExportCuePointsPane settings={settings} />}
              {section === 'renameFiles' && <RenameFilesPane settings={settings} />}
              {section === 'keyNotation' && <KeyNotationPane settings={settings} />}
              {section === 'tempo' && <TempoPane settings={settings} />}
              {section === 'audioPlayer' && <AudioPlayerPane settings={settings} />}
              {section === 'displayOptions' && <DisplayOptionsPane settings={settings} />}
            </>
          )}
          {section === 'feedback' && <FeedbackPane />}
          {section === 'about' && <AboutPane />}
        </div>
      </ScrollArea>
    </div>
  );
}
