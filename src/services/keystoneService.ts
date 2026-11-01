import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type {
  AnalysisProgress,
  AnalysisResult,
  BackupEntry,
  BeatgridMarker,
  CleanCacheReport,
  Collection,
  CoverArt,
  CueEdit,
  CuePoint,
  ExportReport,
  ExportRequest,
  ExportTarget,
  KeyMode,
  LibraryFolder,
  MusicLibraryInfo,
  ReconcileReport,
  RenamePlan,
  RenameReport,
  ScanProgress,
  Settings,
  SortDirection,
  TagBackupEntry,
  TagEdit,
  TagField,
  TagSnapshot,
  TagWriteReport,
  TargetStatus,
  Track,
  TrackFilters,
  TrackPage,
  TrackQuery,
  TrackRegion,
  TrackSort,
  UpdateInfo,
  WatchedFolder,
  Waveform,
  WheelCounts,
} from '../types';

/**
 * The only module in the frontend that calls `invoke`.
 *
 * Components go component -> hook -> service -> invoke. The point is not
 * ceremony: it is that the IPC surface is one file, so when a Rust command's
 * name or payload changes there is exactly one place the compiler points at.
 * That is also why it stays one file as it grows — splitting it by domain would
 * mean the names drift in nine places instead of one.
 *
 * Two rules hold across every line here, and both are load-bearing:
 *
 * - The string is the Rust function's name, spelled exactly. Nothing checks it:
 *   a wrong name is a runtime rejection the type system is blind to.
 * - Argument keys are the Rust parameter names in camelCase, because every IPC
 *   struct carries `#[serde(rename_all = "camelCase")]`. A snake_case key
 *   arrives as a missing field and serde's error is about a type rather than a
 *   name, which is a bad afternoon.
 *
 * Return types are the Rust return types, not what a caller would prefer. A
 * command returning a count is typed as a count even where the UI ignores it.
 */
export const keystoneService = {
  // ── Library ───────────────────────────────────────────────────────────────
  addWatchedFolder(path: string): Promise<WatchedFolder> {
    return invoke('add_watched_folder', { path });
  },

  /** Keyed by path, not id: the backend cascades by path prefix. Returns the
      number of tracks that left the library with the folder. */
  removeWatchedFolder(path: string): Promise<number> {
    return invoke('remove_watched_folder', { path });
  },

  getWatchedFolders(): Promise<WatchedFolder[]> {
    return invoke('get_watched_folders');
  },

  /** Returns the number of files queued, not the progress: the walk reports
      itself on `keystone://scan-progress`. */
  scanFolder(path: string): Promise<number> {
    return invoke('scan_folder', { path });
  },

  /** Whatever was dropped on the window. Folders become watched roots; loose
      audio files are indexed on their own without watching their parent.
      Returns the number of loose files queued, not counting folder contents. */
  addDroppedPaths(paths: string[]): Promise<number> {
    return invoke('add_dropped_paths', { paths });
  },

  /**
   * What the Music app is publishing, and the playlists inside it.
   *
   * `path` reads an exported library XML instead of the default location.
   */
  getMusicLibrary(path: string | null = null): Promise<MusicLibraryInfo> {
    return invoke('get_music_library', { path });
  },

  importMusicPlaylists(xmlPath: string, names: string[]): Promise<number> {
    return invoke('import_music_playlists', { xmlPath, names });
  },

  /** An `.m3u`, `.m3u8` or `.pls` file. */
  importPlaylistFile(path: string): Promise<number> {
    return invoke('import_playlist_file', { path });
  },

  /** Write a diagnostic into the app's own log, beside the Rust side. */
  logClient(message: string): Promise<void> {
    return invoke('log_client', { message });
  },

  cancelScan(): Promise<void> {
    return invoke('cancel_scan');
  },

  getScanProgress(): Promise<ScanProgress> {
    return invoke('get_scan_progress');
  },

  runReconciliation(): Promise<ReconcileReport> {
    return invoke('run_reconciliation');
  },

  cleanCache(): Promise<CleanCacheReport> {
    return invoke('clean_cache');
  },

  getLibraryFolders(): Promise<LibraryFolder[]> {
    return invoke('get_library_folders');
  },

  // ── Tracks ────────────────────────────────────────────────────────────────
  searchTracks(query: TrackQuery): Promise<TrackPage> {
    return invoke('search_tracks', { query });
  },

  getTrack(id: string): Promise<Track | null> {
    return invoke('get_track', { id });
  },

  getTracks(ids: string[]): Promise<Track[]> {
    return invoke('get_tracks', { ids });
  },

  /** Per-segment counts for the wheel, under the current filters. */
  getWheelCounts(filters: TrackFilters): Promise<WheelCounts> {
    return invoke('get_wheel_counts', { filters });
  },

  /** The genre vocabulary, read out of the library itself. */
  getGenres(): Promise<string[]> {
    return invoke('get_genres');
  },

  /** `null` clears the rating rather than storing a zero. */
  setRating(id: string, rating: number | null): Promise<Track> {
    return invoke('set_rating', { id, rating });
  },

  /** A correction. Stamps the value `user`, which stops a re-analysis
      overwriting it. */
  setTrackKey(id: string, keyRoot: number, keyMode: KeyMode): Promise<Track> {
    return invoke('set_track_key', { id, keyRoot, keyMode });
  },

  setTrackTempo(id: string, bpm: number): Promise<Track> {
    return invoke('set_track_tempo', { id, bpm });
  },

  setTrackEnergy(id: string, energy: number): Promise<Track> {
    return invoke('set_track_energy', { id, energy });
  },

  /** `deleteFiles` is a separate argument rather than a setting so that nothing
      can delete a file without the call site having said so. */
  removeTracks(ids: string[], deleteFiles = false): Promise<number> {
    return invoke('remove_tracks_from_library', { ids, deleteFiles });
  },

  revealInFinder(path: string): Promise<void> {
    return invoke('reveal_in_finder', { path });
  },

  /** Copies the file into `destDir` and returns where it landed. */
  exportFile(id: string, destDir: string): Promise<string> {
    return invoke('export_file', { id, destDir });
  },

  /** Dry run. Renaming is the one irreversible operation, so this always runs
      first and its result is what the UI shows — and what `renameTracks` is
      then handed, rather than re-deriving a plan the user never saw. */
  planRename(ids: string[]): Promise<RenamePlan> {
    return invoke('preview_rename', { ids });
  },

  renameTracks(plan: RenamePlan): Promise<RenameReport> {
    return invoke('rename_tracks', { plan });
  },

  removeKeyFromFilename(ids: string[]): Promise<RenameReport> {
    return invoke('remove_key_from_filename', { ids });
  },

  incrementPlayCount(id: string): Promise<void> {
    return invoke('increment_play_count', { id });
  },

  /** One collection's tracks, in the collection's own order when sorted by
      position. Separate from `searchTracks` because membership order is a join
      column, not a track column. */
  getCollectionPage(args: {
    collectionId: string;
    sort: TrackSort;
    direction: SortDirection;
    limit: number;
    offset: number;
  }): Promise<TrackPage> {
    return invoke('get_collection_page', args);
  },

  // ── Analysis ──────────────────────────────────────────────────────────────
  enqueueTracks(trackIds: string[]): Promise<AnalysisProgress> {
    return invoke('enqueue_analysis', { trackIds });
  },

  enqueueAllUnanalyzed(): Promise<AnalysisProgress> {
    return invoke('enqueue_all_unanalyzed');
  },

  /** Forces a re-run even on a content-hash cache hit. */
  reanalyzeTracks(trackIds: string[]): Promise<AnalysisProgress> {
    return invoke('reanalyze_tracks', { trackIds });
  },

  getAnalysisProgress(): Promise<AnalysisProgress> {
    return invoke('get_analysis_progress');
  },

  pauseQueue(): Promise<AnalysisProgress> {
    return invoke('pause_analysis');
  },

  resumeQueue(): Promise<AnalysisProgress> {
    return invoke('resume_analysis');
  },

  clearQueue(): Promise<AnalysisProgress> {
    return invoke('cancel_analysis');
  },

  /** Null until the peaks have been cached for this track's content hash. */
  getWaveform(trackId: string): Promise<Waveform | null> {
    return invoke('get_waveform', { trackId });
  },

  getRegions(trackId: string): Promise<TrackRegion[]> {
    return invoke('get_track_regions', { trackId });
  },

  getAnalysis(trackId: string): Promise<AnalysisResult | null> {
    return invoke('get_analysis', { trackId });
  },

  // ── Cue points ────────────────────────────────────────────────────────────
  getCuePoints(trackId: string): Promise<CuePoint[]> {
    return invoke('get_cue_points', { trackId });
  },

  /** `[trackId, count]` pairs, for the table's CUE POINTS column: one query for
      a page of rows rather than one per row. */
  getCueCounts(trackIds: string[]): Promise<Array<[string, number]>> {
    return invoke('get_cue_counts', { trackIds });
  },

  /** Writes with `source: 'user'`; the detector will not overwrite it. */
  setCuePoint(edit: CueEdit): Promise<CuePoint> {
    return invoke('add_cue_point', { edit });
  },

  moveCuePoint(cueId: string, positionMs: number): Promise<CuePoint> {
    return invoke('move_cue_point', { cueId, positionMs });
  },

  deleteCuePoint(cueId: string): Promise<void> {
    return invoke('delete_cue_point', { cueId });
  },

  /** `includeUser` takes the hand-placed cues too, which is the one thing here
      with no undo — a re-analysis puts the detected cue back, not the user's. */
  deleteCuePoints(trackId: string, includeUser: boolean): Promise<number> {
    return invoke('remove_cue_points', { trackId, includeUser });
  },

  getBeatgrid(trackId: string): Promise<BeatgridMarker[]> {
    return invoke('get_beatgrid', { trackId });
  },

  // ── Collections ───────────────────────────────────────────────────────────
  getCollections(): Promise<Collection[]> {
    return invoke('list_collections');
  },

  createCollection(name: string): Promise<Collection> {
    return invoke('create_collection', { name });
  },

  renameCollection(id: string, name: string): Promise<void> {
    return invoke('rename_collection', { id, name });
  },

  deleteCollection(id: string): Promise<void> {
    return invoke('delete_collection', { id });
  },

  /** Full ordered list of ids; the backend rewrites `position` from the index. */
  reorderCollections(orderedIds: string[]): Promise<void> {
    return invoke('reorder_collections', { orderedIds });
  },

  setCollectionColor(id: string, color: string | null): Promise<void> {
    return invoke('set_collection_color', { id, color });
  },

  addToCollection(collectionId: string, trackIds: string[]): Promise<number> {
    return invoke('add_tracks_to_collection', { collectionId, trackIds });
  },

  removeFromCollection(
    collectionId: string,
    trackIds: string[],
  ): Promise<number> {
    return invoke('remove_tracks_from_collection', { collectionId, trackIds });
  },

  reorderCollectionTracks(
    collectionId: string,
    orderedIds: string[],
  ): Promise<void> {
    return invoke('reorder_collection_tracks', { collectionId, orderedIds });
  },

  // ── Tags ──────────────────────────────────────────────────────────────────
  readTags(trackId: string): Promise<TagSnapshot> {
    return invoke('get_track_tags', { trackId });
  },

  writeTags(trackId: string, edit: TagEdit): Promise<TagWriteReport> {
    return invoke('update_track_tags', { trackId, edit });
  },

  getCoverArt(trackId: string): Promise<CoverArt | null> {
    return invoke('get_cover_art', { trackId });
  },

  /** `null` removes the embedded artwork. */
  setCoverArt(trackId: string, art: CoverArt | null): Promise<TagWriteReport> {
    return invoke('set_cover_art', { trackId, art });
  },

  /**
   * SPEC.md §2 Global Replace, across every column the user ticked.
   *
   * `dryRun` is true at every call site in the UI before the write: this
   * rewrites the user's files, and the count has to be shown first. A null
   * `trackIds` means the whole library.
   */
  globalReplace(args: {
    fields: TagField[];
    find: string;
    replace: string;
    trackIds: string[] | null;
    dryRun: boolean;
  }): Promise<TagWriteReport> {
    return invoke('global_replace', args);
  },

  removeExtraSpaces(
    trackIds: string[],
    dryRun: boolean,
  ): Promise<TagWriteReport> {
    return invoke('remove_extra_spaces', { trackIds, dryRun });
  },

  /** Re-tag the given tracks, using the Update Tags settings. */
  retagTracks(trackIds: string[], dryRun: boolean): Promise<TagWriteReport> {
    return invoke('retag_tracks', { trackIds, dryRun });
  },

  /** RE-TAG EXISTING FILES over the whole library. */
  retagExistingFiles(dryRun: boolean): Promise<TagWriteReport> {
    return invoke('retag_existing_files', { dryRun });
  },

  removeKeyFromTags(trackIds: string[]): Promise<TagWriteReport> {
    return invoke('remove_key_from_tags', { trackIds });
  },

  listTagBackups(trackId: string): Promise<TagBackupEntry[]> {
    return invoke('list_tag_backups', { trackId });
  },

  restoreTagBackup(trackId: string, backupId: string): Promise<void> {
    return invoke('restore_tag_backup', { trackId, backupId });
  },

  // ── Cue-point export ──────────────────────────────────────────────────────
  detectExportTargets(): Promise<TargetStatus[]> {
    return invoke('detect_export_targets');
  },

  setExportPath(target: ExportTarget, path: string | null): Promise<void> {
    return invoke('set_export_path', { target, path });
  },

  /** The dry run. Pass the same `request` to `exportCuePoints` with `dryRun`
      flipped, so the write is logged under the run the plan allocated. */
  planCueExport(request: ExportRequest): Promise<ExportReport> {
    return invoke('dry_run_export', { request });
  },

  exportCuePoints(request: ExportRequest): Promise<ExportReport> {
    return invoke('export_cue_points', { request });
  },

  listExportBackups(target: ExportTarget): Promise<BackupEntry[]> {
    return invoke('list_export_backups', { target });
  },

  restoreExportBackup(
    target: ExportTarget,
    backupPath: string,
  ): Promise<void> {
    return invoke('restore_export_backup', { target, backupPath });
  },

  // ── Player ────────────────────────────────────────────────────────────────
  /** Returns a path. No decoding on this path — the webview decodes. */
  getPlaybackPath(trackId: string): Promise<string> {
    return invoke('get_playback_path', { trackId });
  },

  /** SPEC.md's `Start playing the track 25% of the way in`, resolved against
      the track's own duration. The rule lives in the backend so there is one
      answer rather than two that can disagree. */
  getStartOffsetMs(trackId: string): Promise<number> {
    return invoke('get_start_offset_ms', { trackId });
  },

  // ── Settings ──────────────────────────────────────────────────────────────
  getSettings(): Promise<Settings> {
    return invoke('get_settings');
  },

  updateSettings(settings: Settings): Promise<Settings> {
    return invoke('update_settings', { settings });
  },

  /**
   * Re-fold `tempoDisplay` from `tempoRaw` after the Tempo range changed.
   * Returns the number of rows touched. This is a database pass, not a
   * re-analysis — which is the whole reason the two columns are separate.
   */
  refoldTempoDisplay(): Promise<number> {
    return invoke('refold_tempo_display');
  },

  checkForUpdate(): Promise<UpdateInfo> {
    return invoke('check_for_update');
  },
};

/** Webview-loadable URL for a file on disk. A bare `file://` will not load. */
export function mediaUrl(path: string): string {
  return convertFileSrc(path);
}

/** Embedded cover art as a data URL. */
export function coverArtUrl(art: CoverArt): string {
  return `data:${art.mime};base64,${art.dataBase64}`;
}
