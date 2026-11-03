import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { SettingGroup, SettingNote, SettingSection } from './SettingSection';
import { ConfirmDialog } from './ConfirmDialog';
import { Button } from '../ui/Button';
import { FolderAdd, Refresh, Trash } from '../ui/icons';
import { CleanResult, FolderRow, ReconcileResult } from './LibraryPaneRows';
import { ScanProgressBar } from './ScanProgressBar';
import {
  useAddFolder,
  useCancelScan,
  useCleanCache,
  useReconcile,
  useRemoveFolder,
  useRescanFolder,
  useScanProgressOnce,
  useWatchedFolders,
} from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { formatCount } from '../../utils/formatters';
import type { WatchedFolder } from '../../types';

/**
 * Where the library comes from, and what to do when it drifts from the disk.
 *
 * This pane is new because the commands behind it were not reachable at all:
 * `scan_folder`, `cancel_scan`, `get_scan_progress`, `run_reconciliation` and
 * `clean_cache` were registered, exposed through `keystoneService` and called
 * by nothing. Tracks went in through ADD TRACKS and there was no way to
 * re-scan after adding files outside the app, no way to see the watched
 * folders, and no recovery at all for a track whose file had moved — the row
 * simply stopped working.
 *
 * The two maintenance actions are deliberately not alike, and the copy says
 * so. Reconciling only ever writes `index_status` and `last_seen_at`, so it
 * is safe to run at any time and sits behind no confirmation. Cleaning the
 * cache deletes the rows reconciliation marked missing, and a row carries the
 * track's cue points, overrides and rating with it — so an unplugged drive
 * cleaned away is a re-import rather than a reconnect. That one is behind a
 * destructive confirm.
 */
export function LibraryPane() {
  const { data: folders = [], isLoading } = useWatchedFolders();
  const scanProgress = useLibraryStore((s) => s.scanProgress);
  // Seeds the store when the pane is opened mid-scan; see the hook.
  useScanProgressOnce();

  const addFolder = useAddFolder();
  const remove = useRemoveFolder();
  const rescan = useRescanFolder();
  const cancelScan = useCancelScan();
  const reconcile = useReconcile();
  const cleanCache = useCleanCache();

  const [pendingRemove, setPendingRemove] = useState<WatchedFolder | null>(null);
  const [confirmClean, setConfirmClean] = useState(false);

  const scanning = scanProgress?.isScanning ?? false;

  const pickFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') addFolder.mutate(picked);
  };

  return (
    <SettingSection title="Library">
      <SettingGroup
        label="Watched folders"
        hint="Keystone indexes everything under these. Removing one takes its tracks out of the library; the files are not touched."
      >
        {isLoading && <SettingNote>Loading…</SettingNote>}
        {!isLoading && folders.length === 0 && (
          <SettingNote>
            No folders yet. Add one here or with ADD TRACKS in the sidebar.
          </SettingNote>
        )}

        <div className="flex flex-col gap-1">
          {folders.map((folder) => (
            <FolderRow
              key={folder.id}
              folder={folder}
              busy={scanning || rescan.isPending}
              onRescan={() => rescan.mutate(folder.path)}
              onRemove={() => setPendingRemove(folder)}
            />
          ))}
        </div>

        <div className="mt-1 flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            disabled={addFolder.isPending || scanning}
            onClick={() => void pickFolder()}
            icon={<FolderAdd size={13} color="currentColor" variant="Linear" />}
          >
            Add folder…
          </Button>
        </div>

        <ScanProgressBar onCancel={() => cancelScan.mutate()} cancelling={cancelScan.isPending} />
      </SettingGroup>

      <SettingGroup
        label="When files move"
        hint="Keystone watches these folders while it is open. Anything that changed while it was closed is found by checking again."
      >
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            disabled={reconcile.isPending || scanning}
            onClick={() => reconcile.mutate()}
            icon={<Refresh size={13} color="currentColor" variant="Linear" />}
          >
            {reconcile.isPending ? 'Checking…' : 'Check for missing files'}
          </Button>
          <SettingNote>Marks what is gone. Deletes nothing.</SettingNote>
        </div>

        {reconcile.data && <ReconcileResult report={reconcile.data} />}
        {reconcile.isError && (
          <SettingNote tone="warn">
            Could not check: {String(reconcile.error)}
          </SettingNote>
        )}
      </SettingGroup>

      <SettingGroup
        label="Clean up"
        hint="Removes the tracks marked missing, with their analysis and cached waveforms."
      >
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            disabled={cleanCache.isPending || scanning}
            onClick={() => setConfirmClean(true)}
            icon={<Trash size={13} color="currentColor" variant="Linear" />}
          >
            {cleanCache.isPending ? 'Cleaning…' : 'Clean cache'}
          </Button>
        </div>
        <SettingNote tone="warn">
          A removed track loses its cue points, its key and tempo overrides and
          its rating. If a drive is only unplugged, plug it back in and check
          for missing files instead — cleaning first turns that into a
          re-import.
        </SettingNote>

        {cleanCache.data && <CleanResult report={cleanCache.data} />}
        {cleanCache.isError && (
          <SettingNote tone="warn">
            Could not clean: {String(cleanCache.error)}
          </SettingNote>
        )}
      </SettingGroup>

      {pendingRemove && (
        <ConfirmDialog
          title={`Stop watching ${pendingRemove.path.split('/').pop()}?`}
          body={`Its ${formatCount(pendingRemove.trackCount)} tracks leave the library, along with their cue points and overrides. The files on disk are not touched.`}
          confirmLabel="Stop watching"
          destructive
          busy={false}
          onCancel={() => setPendingRemove(null)}
          onConfirm={() => {
            remove.mutate(pendingRemove.path);
            setPendingRemove(null);
          }}
        />
      )}

      {confirmClean && (
        <ConfirmDialog
          title="Clean the cache?"
          body="Every track currently marked missing is removed from the library, with its analysis, cue points, overrides and rating. This cannot be undone."
          confirmLabel="Clean cache"
          destructive
          busy={cleanCache.isPending}
          onCancel={() => setConfirmClean(false)}
          onConfirm={() => {
            setConfirmClean(false);
            cleanCache.mutate();
          }}
        />
      )}
    </SettingSection>
  );
}
