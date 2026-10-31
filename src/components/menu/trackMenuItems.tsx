import { save } from '@tauri-apps/plugin-dialog';
import {
  DocumentDownload,
  Export,
  FolderOpen,
  InfoCircle,
  Play,
  Refresh,
  Tag,
  Trash,
} from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { usePlayerStore } from '../../stores/playerStore';
import type { MenuItem } from '../ui/menuItem';
import type { ExportTarget, Settings, Track } from '../../types';
import type { ConfirmRequest } from './useTrackMenu';

/** Shared by every builder below: what the menu needs to do its work. */
export interface MenuContext {
  track: Track;
  settings: Settings;
  /** Export targets that are enabled, supported and actually installed. */
  liveTargets: ExportTarget[];
  /** Invalidate the library rows after a write. */
  refresh: () => void;
  /** Invalidate this track's cue points. */
  refreshCues: () => void;
  /** Open the Song Info panel. */
  openInfo: () => void;
  ask: (request: ConfirmRequest) => void;
}

/** Play Song · Song Info · Reveal in Finder · Export File, in SPEC.md's order. */
export function fileItems({ track, settings, openInfo }: MenuContext): MenuItem[] {
  return [
    {
      id: 'play',
      label: 'Play Song',
      icon: <Play size={12} color="currentColor" variant="Linear" />,
      onSelect: () => {
        // The Audio Player setting decides where a preview starts; past the
        // intro is where a DJ is listening from.
        const at =
          settings.startAtQuarter && track.durationMs
            ? (track.durationMs / 1000) * 0.25
            : 0;
        usePlayerStore.getState().play(track, at);
      },
    },
    {
      id: 'info',
      label: 'Song Info...',
      icon: <InfoCircle size={12} color="currentColor" variant="Linear" />,
      onSelect: openInfo,
    },
    {
      id: 'reveal',
      label: 'Reveal in Finder',
      icon: <FolderOpen size={12} color="currentColor" variant="Linear" />,
      onSelect: () => void keystoneService.revealInFinder(track.path),
    },
    {
      id: 'export-file',
      label: 'Export File...',
      icon: <DocumentDownload size={12} color="currentColor" variant="Linear" />,
      onSelect: () => {
        save({ defaultPath: track.filename }).then((dest) => {
          if (typeof dest === 'string') void keystoneService.exportFile(track.id, dest);
        });
      },
    },
  ];
}

/**
 * Remove Key from File Name · from Common Tags · Re-tag Song.
 *
 * All three read as destructive and none asks first: the two removals are
 * exactly reversible — the file name from the stored `originalFilename`, the
 * tags from the backup taken before the write — and a dialog in front of a
 * reversible action teaches people to click through them.
 */
export function tagItems({ track, refresh }: MenuContext): MenuItem[] {
  const id = track.id;
  return [
    {
      id: 'strip-filename',
      label: 'Remove Key from File Name',
      separatorBefore: true,
      danger: true,
      onSelect: () => void keystoneService.removeKeyFromFilename([id]).then(refresh),
    },
    {
      id: 'strip-tags',
      label: 'Remove Key from Common Tags',
      danger: true,
      onSelect: () => void keystoneService.removeKeyFromTags([id]).then(refresh),
    },
    {
      id: 'retag',
      label: 'Re-tag Song',
      icon: <Tag size={12} color="currentColor" variant="Linear" />,
      onSelect: () => void keystoneService.retagTracks([id], false).then(refresh),
    },
  ];
}

/**
 * Export Cue Points · Remove Cue Points.
 *
 * Removal confirms because it is the one item here with no undo: a cue the user
 * dragged into place is not recoverable from analysis, which would put the
 * detected one back instead.
 */
export function cueItems({ track, liveTargets, ask, refreshCues }: MenuContext): MenuItem[] {
  const id = track.id;
  return [
    {
      id: 'export-cues',
      label: 'Export Cue Points',
      icon: <Export size={12} color="currentColor" variant="Linear" />,
      separatorBefore: true,
      disabled: liveTargets.length === 0,
      onSelect: () => void exportToAll(liveTargets, id),
    },
    {
      id: 'remove-cues',
      label: 'Remove Cue Points',
      danger: true,
      onSelect: () =>
        ask({
          title: 'Remove every cue point?',
          body: 'Cues you placed by hand go too, and that cannot be undone.',
          confirmLabel: 'Remove',
          // The hand-placed cues go too, which is exactly what the dialog said.
          run: () => void keystoneService.deleteCuePoints(id, true).then(refreshCues),
        }),
    },
    {
      id: 'reanalyze',
      label: 'Re-analyze Song',
      icon: <Refresh size={12} color="currentColor" variant="Linear" />,
      separatorBefore: true,
      onSelect: () => void keystoneService.reanalyzeTracks([id]),
    },
  ];
}

export function removeItem({ track, refresh, ask }: MenuContext): MenuItem {
  return {
    id: 'remove',
    label: 'Remove Song from Library...',
    icon: <Trash size={12} color="currentColor" variant="Linear" />,
    separatorBefore: true,
    danger: true,
    onSelect: () =>
      ask({
        title: `Remove ${track.filename}?`,
        body: 'It leaves Keystone. The file on disk is untouched.',
        confirmLabel: 'Remove',
        run: () => void keystoneService.removeTracks([track.id]).then(refresh),
      }),
  };
}

/**
 * Sequential: each target backs up and rewrites a library file of its own.
 *
 * The dry run is not skipped even for one track. It is what allocates the run
 * id the write is logged under, and it is the same code path as the write, so
 * a refusal — the target app being open — surfaces before anything is touched.
 */
async function exportToAll(targets: ExportTarget[], trackId: string): Promise<void> {
  for (const target of targets) {
    const request = {
      target,
      trackIds: [trackId],
      overwriteExisting: false,
      dryRun: true,
      outputPath: null,
      runId: crypto.randomUUID(),
    };
    await keystoneService.planCueExport(request);
    await keystoneService.exportCuePoints({ ...request, dryRun: false });
  }
}
