import { useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { Button } from '../ui/Button';
import { Checkbox } from '../ui/Checkbox';
import { Archive } from '../ui/icons';
import { SettingNote } from './SettingSection';
import { TargetMark } from './TargetMark';
import {
  TARGET_LABEL,
  useExportBackups,
  useRestoreExportBackup,
  useSetExportPath,
} from './useExportTargets';
import { targetPath, type ExportTarget, type TargetStatus } from '../../types';

/**
 * One export target: the checkbox, its detected path, and CHANGE.
 *
 * CHANGE is mandatory rather than a nicety. Traktor makes a new root directory
 * on every version update and the root is user-settable in its own preferences,
 * so detection is best-effort by construction.
 */
export function ExportTargetRow({
  info,
  enabled,
  onToggle,
}: {
  info: TargetStatus;
  enabled: boolean;
  onToggle: (on: boolean) => void;
}) {
  const setPath = useSetExportPath();
  const [showBackups, setShowBackups] = useState(false);

  const blocked = !info.supported || !info.installed;
  const label = TARGET_LABEL[info.target];
  const path = targetPath(info);

  const pickPath = async () => {
    // rekordbox gets a file Keystone writes itself; the other two point at a
    // library file that already exists and must be chosen, not created.
    const picked =
      info.target === 'rekordbox'
        ? await save({ defaultPath: 'keystone.xml', filters: [{ name: 'XML', extensions: ['xml'] }] })
        : await open({ multiple: false, directory: false });
    if (typeof picked === 'string') setPath.mutate({ target: info.target, path: picked });
  };

  return (
    <div className="flex flex-col gap-1.5 border-b border-gray-700 pb-3 last:border-0 last:pb-0">
      <div className="flex items-center justify-between gap-3">
        <span className="flex min-w-0 items-center gap-2">
          <TargetMark target={info.target} dimmed={blocked} />
          <Checkbox
            checked={enabled && !blocked}
            disabled={blocked}
            onChange={onToggle}
            label={label}
          />
        </span>
        {!blocked && (
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="sm" onClick={pickPath}>
              CHANGE
            </Button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={`${label} backups`}
              onClick={() => setShowBackups((v) => !v)}
              icon={<Archive size={13} color="currentColor" variant="Linear" />}
            />
          </div>
        )}
      </div>

      {path && !blocked && (
        <p className="mono truncate pl-6 text-[12px] text-gray-500" title={path}>
          {path}
        </p>
      )}

      <TargetNotes info={info} blocked={blocked} label={label} />

      {showBackups && <BackupList target={info.target} />}
    </div>
  );
}

/**
 * Why a target is unavailable, or why a write would be refused right now.
 *
 * A disabled checkbox with nothing beside it reads as a bug, and "Traktor is
 * open" is not a warning to be dismissed — Traktor rewrites its collection on
 * exit and would silently discard the export.
 */
function TargetNotes({
  info,
  blocked,
  label,
}: {
  info: TargetStatus;
  blocked: boolean;
  label: string;
}) {
  if (blocked) {
    return (
      <div className="pl-6">
        <SettingNote>{info.reasonUnavailable ?? `${label} was not found.`}</SettingNote>
      </div>
    );
  }
  if (!info.appRunning) return null;
  return (
    <div className="pl-6">
      <SettingNote tone="warn">
        {label} is open. Close it first, or the write is discarded.
      </SettingNote>
    </div>
  );
}

/**
 * The dated copies taken before the first write to this target.
 *
 * Restore is a click rather than a file-manager errand: when a cue export goes
 * wrong, the backup is the only thing that matters, and asking the user to find
 * it on disk at that moment is asking them to give up.
 */
function BackupList({ target }: { target: ExportTarget }) {
  const backups = useExportBackups(target, true);
  const restore = useRestoreExportBackup();
  const entries = backups.data ?? [];

  if (entries.length === 0) {
    return (
      <div className="pl-6">
        <SettingNote>No backups yet. One is taken before the first write.</SettingNote>
      </div>
    );
  }

  return (
    <div className="pl-6">
      {entries.slice(0, 5).map((b) => (
        <button
          key={b.path}
          type="button"
          disabled={restore.isPending}
          onClick={() => restore.mutate({ target, path: b.path })}
          className="mono block w-full truncate text-left text-[12px] text-gray-400 hover:text-keystone-lime"
          title={b.path}
        >
          Restore {new Date(b.createdUnix * 1000).toLocaleString()}
        </button>
      ))}
    </div>
  );
}
