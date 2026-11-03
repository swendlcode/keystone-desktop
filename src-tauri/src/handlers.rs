//! The invoke registration table.
//!
//! Tauri permits exactly one `invoke_handler`, and every command the product has
//! must be listed in it, so this is one unavoidably long list. It lives in a
//! macro rather than a function for two reasons: a `macro_rules!` body is a
//! declaration rather than a function body, so `run` stays short enough to read;
//! and no later agent has to edit `lib.rs` to add a command.
//!
//! Adding a command means one line here and one `#[tauri::command]` in
//! `commands/`. Nothing else.

#[macro_export]
macro_rules! keystone_invoke_handler {
    () => {
        tauri::generate_handler![
            // library
            $crate::commands::library_commands::add_watched_folder,
            $crate::commands::library_commands::add_dropped_paths,
            $crate::commands::library_commands::remove_watched_folder,
            $crate::commands::library_commands::get_watched_folders,
            $crate::commands::library_commands::scan_folder,
            $crate::commands::library_commands::get_scan_progress,
            $crate::commands::library_commands::cancel_scan,
            $crate::commands::library_commands::run_reconciliation,
            $crate::commands::library_commands::clean_cache,
            $crate::commands::library_commands::get_library_folders,

            $crate::commands::import_commands::get_music_library,
            $crate::commands::import_commands::import_music_playlists,
            $crate::commands::import_commands::import_playlist_file,
            $crate::commands::import_commands::log_client,
            // tracks
            $crate::commands::track_commands::search_tracks,
            $crate::commands::track_commands::get_track,
            $crate::commands::track_commands::get_tracks,
            $crate::commands::track_commands::get_wheel_counts,
            $crate::commands::track_commands::get_genres,
            $crate::commands::track_commands::get_collection_page,
            $crate::commands::track_commands::set_rating,
            $crate::commands::track_commands::set_track_key,
            $crate::commands::track_commands::set_track_tempo,
            $crate::commands::track_commands::set_track_energy,
            $crate::commands::track_commands::remove_tracks_from_library,
            $crate::commands::track_commands::reveal_in_finder,
            $crate::commands::track_commands::export_file,
            $crate::commands::track_commands::preview_rename,
            $crate::commands::track_commands::rename_tracks,
            $crate::commands::track_commands::remove_key_from_filename,
            $crate::commands::track_commands::increment_play_count,
            // analysis
            $crate::commands::analysis_commands::enqueue_analysis,
            $crate::commands::analysis_commands::enqueue_all_unanalyzed,
            $crate::commands::analysis_commands::reanalyze_tracks,
            $crate::commands::analysis_commands::get_analysis_progress,
            $crate::commands::analysis_commands::pause_analysis,
            $crate::commands::analysis_commands::resume_analysis,
            $crate::commands::analysis_commands::cancel_analysis,
            $crate::commands::analysis_commands::get_waveform,
            $crate::commands::analysis_commands::get_track_regions,
            $crate::commands::analysis_commands::get_analysis,
            // cues
            $crate::commands::cue_commands::get_cue_points,
            $crate::commands::cue_commands::get_cue_counts,
            $crate::commands::cue_commands::add_cue_point,
            $crate::commands::cue_commands::move_cue_point,
            $crate::commands::cue_commands::delete_cue_point,
            $crate::commands::cue_commands::remove_cue_points,
            $crate::commands::cue_commands::get_beatgrid,
            // collections
            $crate::commands::collection_commands::list_collections,
            $crate::commands::collection_commands::create_collection,
            $crate::commands::collection_commands::rename_collection,
            $crate::commands::collection_commands::set_collection_color,
            $crate::commands::collection_commands::delete_collection,
            $crate::commands::collection_commands::reorder_collections,
            $crate::commands::collection_commands::add_tracks_to_collection,
            $crate::commands::collection_commands::remove_tracks_from_collection,
            $crate::commands::collection_commands::reorder_collection_tracks,
            $crate::commands::collection_commands::export_collection_csv,
            // tags
            $crate::commands::tag_commands::get_track_tags,
            $crate::commands::tag_commands::update_track_tags,
            $crate::commands::tag_commands::get_cover_art,
            $crate::commands::tag_commands::set_cover_art,
            $crate::commands::tag_commands::global_replace,
            $crate::commands::tag_commands::remove_extra_spaces,
            $crate::commands::tag_commands::retag_tracks,
            $crate::commands::tag_commands::retag_existing_files,
            $crate::commands::tag_commands::remove_key_from_tags,
            $crate::commands::tag_commands::list_tag_backups,
            $crate::commands::tag_commands::restore_tag_backup,
            // export
            $crate::commands::export_commands::detect_export_targets,
            $crate::commands::export_commands::set_export_path,
            $crate::commands::export_commands::dry_run_export,
            $crate::commands::export_commands::export_cue_points,
            $crate::commands::export_commands::list_export_backups,
            $crate::commands::export_commands::restore_export_backup,
            // player
            $crate::commands::player_commands::get_playback_path,
            $crate::commands::player_commands::get_start_offset_ms,
            // settings
            $crate::commands::settings_commands::get_settings,
            $crate::commands::settings_commands::update_settings,
            $crate::commands::settings_commands::refold_tempo_display,
            // updater
        ]
    };
}
