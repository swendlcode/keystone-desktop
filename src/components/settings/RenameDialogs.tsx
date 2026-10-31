import { Button } from '../ui/Button';
import { Refresh } from '../ui/icons';
import { ConfirmDialog } from './ConfirmDialog';
import { RenamePreview } from './RenamePreview';
import { SettingGroup, SettingNote } from './SettingSection';
import type { RenamePlan, RenameReport } from '../../types';

/**
 * The plan, and the gate in front of the automatic mode.
 *
 * Turning automatic renaming on is confirmed because nothing will ask again:
 * from then on every analysed file is renamed on the spot.
 */
export function RenameDialogs({
  plan,
  busy,
  arming,
  onCancelPlan,
  onConfirmPlan,
  onCancelArm,
  onConfirmArm,
}: {
  plan: RenamePlan | null;
  busy: boolean;
  arming: boolean;
  onCancelPlan: () => void;
  onConfirmPlan: () => void;
  onCancelArm: () => void;
  onConfirmArm: () => void;
}) {
  return (
    <>
      {plan && (
        <ConfirmDialog
          title="Rename these files?"
          destructive
          busy={busy}
          confirmLabel="Rename"
          cancelLabel="Close"
          body={<RenamePreview plan={plan} />}
          onCancel={onCancelPlan}
          onConfirm={onConfirmPlan}
        />
      )}
      {arming && (
        <ConfirmDialog
          title="Rename every file after analysis?"
          destructive
          confirmLabel="Turn it on"
          body="Files are renamed as soon as they are analysed, without asking again. Restore original names undoes it."
          onCancel={onCancelArm}
          onConfirm={onConfirmArm}
        />
      )}
    </>
  );
}

/**
 * The preview group: the two buttons and whatever the last run reported.
 *
 * Restore sits next to Preview rather than somewhere safer, because the undo
 * for a destructive automatic setting has to be findable by someone who has
 * just been surprised by it. It renames back to the stored `originalFilename`,
 * so it is exact rather than a regex guessing at what Keystone added.
 *
 * Restore is next to Preview rather than somewhere safer, because the undo for
 * a destructive automatic setting has to be findable by someone who has just
 * been surprised by it. It renames back to the stored `originalFilename`, so it
 * is exact rather than a regex guessing at what Keystone added.
 */
export function PreviewGroup({
  count,
  busy,
  report,
  undone,
  error,
  onPreview,
  onRestore,
}: {
  count: number;
  busy: boolean;
  report: RenameReport | null;
  undone: RenameReport | null;
  error: string | null;
  onPreview: () => void;
  onRestore: () => void;
}) {
  return (
    <SettingGroup label="Preview">
      <div className="flex gap-2">
        <Button variant="secondary" size="sm" disabled={busy || !count} onClick={onPreview}>
          PREVIEW {count || ''}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={!count}
          onClick={onRestore}
          icon={<Refresh size={13} color="currentColor" variant="Linear" />}
        >
          RESTORE ORIGINAL NAMES
        </Button>
      </div>
      {count === 0 && <SettingNote>Select tracks in the library first.</SettingNote>}
      {error && <SettingNote tone="warn">{error}</SettingNote>}
      {report && (
        <SettingNote>
          {report.renamed} renamed, {report.skipped} left alone, {report.failed} failed.
        </SettingNote>
      )}
      {undone && <SettingNote>{undone.renamed} names put back.</SettingNote>}
    </SettingGroup>
  );
}
