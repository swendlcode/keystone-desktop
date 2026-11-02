use std::path::Path;

use tauri::State;

use crate::commands::blocking;
use crate::commands::tag_pass::{all_track_ids, TagPass};
use crate::error::{KeystoneError, Result};
use crate::models::{
    CoverArt, Settings, TagBackupEntry, TagEdit, TagField, TagSnapshot, TagWritePlacement,
    TagWriteReport, Track,
};
use crate::state::AppState;
use crate::tags;

#[tauri::command]
pub async fn get_track_tags(track_id: String, state: State<'_, AppState>) -> Result<TagSnapshot> {
    let path = track_path(&state, track_id).await?;
    blocking(move || tags::read::read(Path::new(&path))).await
}

/// In-place cell edits from the tag-editing table.
#[tauri::command]
pub async fn update_track_tags(
    track_id: String,
    edit: TagEdit,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let pass = TagPass::new(&state, false);
    blocking(move || pass.run(&[track_id], |_, _| Ok(Some(edit.clone())))).await
}

#[tauri::command]
pub async fn get_cover_art(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Option<CoverArt>> {
    let path = track_path(&state, track_id).await?;
    blocking(move || tags::read::read_cover_art(Path::new(&path))).await
}

#[tauri::command]
pub async fn set_cover_art(
    track_id: String,
    art: Option<CoverArt>,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let pass = TagPass::new(&state, false);
    let path = track_path(&state, track_id).await?;
    blocking(move || {
        let mut report = pass.empty_report();
        tags::write::set_cover_art(
            Path::new(&path),
            art.as_ref(),
            &tags::write::WriteOptions::default(),
        )?;
        report.written = 1;
        Ok(report)
    })
    .await
}

/// Global Replace. `dry_run` is how the count is shown before anything touches
/// a file — and this is the operation that a careless implementation turns into
/// "every Serato GEOB blob in the library, deleted invisibly". The write path
/// uses concrete tag types only; this handler never reaches past it.
#[tauri::command]
pub async fn global_replace(
    fields: Vec<TagField>,
    find: String,
    replace: String,
    track_ids: Option<Vec<String>>,
    dry_run: bool,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    if find.is_empty() {
        return Err(KeystoneError::Other(
            "a replace with nothing to find would rewrite every file".into(),
        ));
    }
    if fields.is_empty() {
        return Err(KeystoneError::Other("pick at least one column".into()));
    }
    let tracks = state.tracks.clone();
    let pass = TagPass::new(&state, dry_run);
    blocking(move || {
        // The dry run counts rows in the database rather than opening files: the
        // confirm step must be instant even over 160,000 tracks.
        if dry_run {
            let mut report = pass.empty_report();
            report.written = tracks.preview_replace(&fields, &find, track_ids.as_deref())?;
            return Ok(report);
        }
        let ids = match track_ids {
            Some(ids) => ids,
            None => all_track_ids(&tracks)?,
        };
        pass.run(&ids, |_, snapshot| {
            tags::patterns::global_replace(snapshot, &fields, &find, &replace)
        })
    })
    .await
}

#[tauri::command]
pub async fn remove_extra_spaces(
    track_ids: Vec<String>,
    dry_run: bool,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let pass = TagPass::new(&state, dry_run);
    blocking(move || {
        pass.run(&track_ids, |_, snapshot| {
            Ok(Some(tags::patterns::remove_extra_spaces(snapshot)))
        })
    })
    .await
}

/// The context menu's Re-tag Song, and Settings' RE-TAG EXISTING FILES when it
/// is scoped to a selection.
///
/// `dry_run` is not optional ceremony: the settings pane shows the count before
/// it opens a file, which is the same safety rule the rename and the export
/// follow, and a re-tag over a whole working library is the write with the most
/// to lose.
#[tauri::command]
pub async fn retag_tracks(
    track_ids: Vec<String>,
    dry_run: bool,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let settings = state.settings.read().clone();
    let pass = TagPass::new(&state, dry_run);
    blocking(move || pass.run(&track_ids, |track, snapshot| retag_edit(track, snapshot, &settings)))
        .await
}

/// Settings -> Update Tags -> RE-TAG EXISTING FILES, over the whole library.
///
/// Safe to re-run, which is what makes an interrupted pass resumable: a track
/// whose marker is already correct composes the same string, so the strip and
/// the insert cancel out and the file is skipped rather than rewritten.
#[tauri::command]
pub async fn retag_existing_files(
    dry_run: bool,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let settings = state.settings.read().clone();
    if !settings.update_tags_enabled {
        return Err(KeystoneError::Other(
            "Update common tags is off, so there is nothing to write".into(),
        ));
    }
    let tracks = state.tracks.clone();
    let pass = TagPass::new(&state, dry_run);
    blocking(move || {
        let ids = all_track_ids(&tracks)?;
        pass.run(&ids, |track, snapshot| retag_edit(track, snapshot, &settings))
    })
    .await
}

/// Remove Key from Common Tags. Strips exactly what Keystone wrote, by
/// composing the same marker again rather than matching a pattern — a greedy
/// pattern would eat a track genuinely called `Energy`.
#[tauri::command]
pub async fn remove_key_from_tags(
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<TagWriteReport> {
    let settings = state.settings.read().clone();
    let pass = TagPass::new(&state, false);
    blocking(move || {
        pass.run(&track_ids, |track, snapshot| {
            let Some(marker) = tags::patterns::compose(track, &settings) else {
                return Ok(None);
            };
            let field = placement_field(settings.tag_write_placement);
            let current = field_value(snapshot, field).unwrap_or_default();
            let stripped = tags::patterns::strip_exact(&current, &marker);
            let mut edit = TagEdit::default().set(field, stripped);
            if settings.update_initial_key_tag {
                edit = edit.clear(TagField::InitialKey);
            }
            Ok(Some(edit))
        })
    })
    .await
}

#[tauri::command]
pub async fn list_tag_backups(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TagBackupEntry>> {
    let db = state.db.clone();
    blocking(move || tags::backup::list(&db, &track_id)).await
}

/// Restores the replaced tag bytes, through the same safe write path.
///
/// Takes the track as well as the backup: a backup id alone cannot name a file,
/// and resolving it would mean a query that belongs in the db layer.
#[tauri::command]
pub async fn restore_tag_backup(
    track_id: String,
    backup_id: String,
    state: State<'_, AppState>,
) -> Result<()> {
    let db = state.db.clone();
    let path = track_path(&state, track_id).await?;
    blocking(move || tags::backup::restore(&db, &backup_id, Path::new(&path))).await
}

/// The marker the Update Tags settings describe, placed where they say.
///
/// `previous` is the same composed string: re-tagging a library twice would
/// otherwise produce `10A - 10A - Title`, and the marker a track carries is
/// always the one its own key and energy compose.
pub(crate) fn retag_edit(
    track: &Track,
    snapshot: &TagSnapshot,
    settings: &Settings,
) -> Result<Option<TagEdit>> {
    let Some(text) = tags::patterns::compose(track, settings) else {
        return Ok(None);
    };
    let mut edit = tags::patterns::apply_placement(
        snapshot,
        &text,
        Some(&text),
        settings.tag_write_placement,
    );
    if settings.update_initial_key_tag {
        if let (Some(root), Some(mode)) = (track.key_root, track.key_mode) {
            edit = edit.set(
                TagField::InitialKey,
                tags::initial_key::tag_value(
                    root,
                    mode,
                    settings.key_notation,
                    settings.write_camelot_to_initial_key,
                    settings.pad_camelot_numbers,
                ),
            );
        }
    }
    if settings.write_energy_to_grouping {
        if let Some(energy) = track.energy {
            edit = edit.set(TagField::Grouping, tags::patterns::grouping_prefix(energy));
        }
    }
    if settings.update_tempo_tag {
        if let Some(bpm) = tags::patterns::tempo_tag_value(track, settings) {
            edit = edit.set(TagField::Bpm, bpm);
        }
    }
    Ok(Some(edit))
}

/// Which field a placement writes into. The two comment placements differ in
/// how, not where, which is `apply_placement`'s business.
fn placement_field(placement: TagWritePlacement) -> TagField {
    match placement {
        TagWritePlacement::FrontOfArtist => TagField::Artist,
        TagWritePlacement::FrontOfTitle | TagWritePlacement::EndOfTitle => TagField::Title,
        TagWritePlacement::FrontOfComment | TagWritePlacement::OverwriteComment => {
            TagField::Comment
        }
    }
}

fn field_value(snapshot: &TagSnapshot, field: TagField) -> Option<String> {
    match field {
        TagField::Artist => snapshot.artist.clone(),
        TagField::Title => snapshot.title.clone(),
        TagField::Comment => snapshot.comment.clone(),
        _ => None,
    }
}

async fn track_path(state: &State<'_, AppState>, track_id: String) -> Result<String> {
    let tracks = state.tracks.clone();
    blocking(move || {
        tracks
            .get_by_id(&track_id)?
            .map(|t| t.path)
            .ok_or_else(|| KeystoneError::NotFound(format!("track {track_id}")))
    })
    .await
}
