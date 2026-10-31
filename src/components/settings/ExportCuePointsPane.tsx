import { Toggle } from '../ui/Toggle';
import { ExportRunAction } from './ExportRunAction';
import { ExportTargetRow } from './ExportTargetRow';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { TARGET_FLAG, TARGET_ORDER, useExportTargets } from './useExportTargets';
import { useSettingsWriter } from './useSettingsWriter';
import type { ExportTarget, Settings } from '../../types';

/**
 * SPEC.md §3 Export Cue Points.
 *
 * Serato is not in v1, and it is shown disabled with the reason rather than
 * hidden or offered as a checkbox that does nothing: a DJ who uses Serato needs
 * to know in one glance whether this app is for them, and finding out after
 * three clicks is worse than being told.
 */
export function ExportCuePointsPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();
  const { data: targets = [], isLoading } = useExportTargets();

  const flagFor = (t: ExportTarget) => Boolean(settings[TARGET_FLAG[t]]);
  const byTarget = new Map(targets.map((t) => [t.target, t]));

  const enabled = TARGET_ORDER.filter((t) => {
    const info = byTarget.get(t);
    return flagFor(t) && info?.supported && info.installed;
  });

  return (
    <SettingSection title="Export Cue Points">
      <SettingGroup label="Targets">
        {isLoading && <SettingNote>Looking for installed DJ software...</SettingNote>}
        {TARGET_ORDER.map((t) => {
          const info = byTarget.get(t);
          if (!info) return null;
          return (
            <ExportTargetRow
              key={t}
              info={info}
              enabled={flagFor(t)}
              onToggle={(on) => write({ [TARGET_FLAG[t]]: on } as Partial<Settings>)}
            />
          );
        })}
      </SettingGroup>

      <SettingGroup>
        <Toggle
          checked={settings.overwriteExistingCues}
          onChange={(overwriteExistingCues) => write({ overwriteExistingCues })}
          label="Overwrite cue points already in the collection"
        />
        <SettingNote>Off keeps whatever is already there and adds nothing over it.</SettingNote>

        {/* Off by default: Traktor overwrites MUSICAL_KEY on its own re-analysis
            and its numbering is unresolved in public sources, so writing it is
            an advanced opt-in rather than part of a cue export. */}
        <Toggle
          className="mt-1"
          checked={settings.writeTraktorMusicalKey}
          onChange={(writeTraktorMusicalKey) => write({ writeTraktorMusicalKey })}
          label="Write Traktor's own key field"
          disabled={!flagFor('traktor')}
        />
        <SettingNote>Traktor may overwrite it when it re-analyses.</SettingNote>
      </SettingGroup>

      <ExportRunAction targets={enabled} overwrite={settings.overwriteExistingCues} />
    </SettingSection>
  );
}
