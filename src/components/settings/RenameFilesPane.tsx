import { useCallback, useMemo, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { RenameDialogs, PreviewGroup } from './RenameDialogs';
import { RenameOptions } from './RenameOptions';
import { SettingSection } from './SettingSection';
import { useFileWriteRun } from './useFileWriteRun';
import { useSettingsWriter } from './useSettingsWriter';
import type { RenamePlan, RenameReport, Settings } from '../../types';

/**
 * SPEC.md §3 Rename Files.
 *
 * The only pane that moves files on disk, and the automatic mode moves them
 * without being asked again — so turning it on is confirmed, the pattern has a
 * preview, and the undo is a button in the same group rather than something to
 * find in a menu. The undo is exact: the planner stores `originalFilename`, so
 * restoring is a rename back to a recorded name, not a regex that strips what
 * it hopes Keystone added.
 */
export function RenameFilesPane({ settings }: { settings: Settings }) {
  const write = useSettingsWriter();
  const qc = useQueryClient();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const ids = useMemo(() => [...selectedIds], [selectedIds]);
  const [armAuto, setArmAuto] = useState(false);
  const [undone, setUndone] = useState<RenameReport | null>(null);

  const dryRun = useCallback(() => keystoneService.planRename(ids), [ids]);
  // The plan the user confirmed is the plan that runs: the backend renames to
  // the `toPath` it already computed rather than re-deriving one from ids,
  // which could differ if a tag landed between the preview and the confirm.
  const apply = useCallback(
    (plan: RenamePlan) =>
      keystoneService.renameTracks({
        items: plan.items.filter((i) => !i.skipped),
      }),
    [],
  );
  const onDone = useCallback(() => {
    qc.invalidateQueries({ queryKey: trackKeys.all });
  }, [qc]);

  const run = useFileWriteRun<RenamePlan, RenameReport>({ dryRun, apply, onDone });

  const restore = () => {
    keystoneService
      .removeKeyFromFilename(ids)
      .then((report) => {
        setUndone(report);
        onDone();
      })
      .catch(() => setUndone(null));
  };

  return (
    <SettingSection title="Rename Files">
      <RenameOptions settings={settings} write={write} onArm={() => setArmAuto(true)} />

      <PreviewGroup
        count={ids.length}
        busy={run.busy}
        report={run.result}
        undone={undone}
        error={run.error}
        onPreview={run.start}
        onRestore={restore}
      />

      <RenameDialogs
        plan={run.plan}
        busy={run.busy}
        arming={armAuto}
        onCancelPlan={run.cancel}
        onConfirmPlan={run.confirm}
        onCancelArm={() => setArmAuto(false)}
        onConfirmArm={() => {
          setArmAuto(false);
          write({ renameMode: 'afterProcessing' });
        }}
      />
    </SettingSection>
  );
}
