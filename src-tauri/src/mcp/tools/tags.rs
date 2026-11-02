//! Tags, written into the user's own files.
//!
//! Every write here goes through [`TagPass`], the same shape the window's
//! Global Replace and Re-tag use: read, decide an edit, write through the
//! safe path, mirror the result into the row, and keep the replaced bytes as
//! a backup. An agent gets no shortcut past it — these are irreplaceable
//! files, and the backup is the only thing that matters when it goes wrong.
//!
//! `dryRun` defaults to **true** on every pass that can touch more than one
//! file. A tool call is cheap and an agent will make one to find out what
//! would happen; the expensive direction has to be the one that is asked for
//! explicitly.

use std::path::Path;

use serde_json::{json, Value};

use super::{bool_arg, ids_arg, ok, opt_str, str_arg};
use crate::commands::tag_commands::retag_edit;
use crate::commands::tag_pass::{all_track_ids, TagPass};
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::models::{TagEdit, TagField};
use crate::tags;

/// The fields an agent may name, as the user sees them.
fn field_from(name: &str) -> Result<TagField> {
    Ok(match name {
        "artist" => TagField::Artist,
        "title" => TagField::Title,
        "album" => TagField::Album,
        "genre" => TagField::Genre,
        "grouping" => TagField::Grouping,
        "comment" => TagField::Comment,
        "year" => TagField::Year,
        "bpm" => TagField::Bpm,
        "initialKey" => TagField::InitialKey,
        other => {
            return Err(KeystoneError::Other(format!(
                "`{other}` is not a tag field"
            )))
        }
    })
}

/// Inside the app the pass reports progress to the window, so a long
/// agent-driven re-tag shows up on the same bar a user-driven one does.
fn pass(ctx: &McpContext, dry_run: bool) -> TagPass {
    TagPass::with_app(
        ctx.tracks.clone(),
        ctx.live.as_ref().map(|l| l.app.clone()),
        dry_run,
    )
}

pub fn get_tags(ctx: &McpContext, args: &Value) -> Result<Value> {
    let path = track_path(ctx, &str_arg(args, "trackId")?)?;
    Ok(serde_json::to_value(tags::read::read(Path::new(&path))?)?)
}

/// Write named fields on one track.
///
/// Writes immediately: this is one file and the caller named every value, so
/// there is nothing a dry run would tell them that the arguments did not.
pub fn set_tags(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let fields = args
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| KeystoneError::Other("`fields` is an object of tag values".into()))?;

    let mut edit = TagEdit::default();
    for (name, value) in fields {
        let field = field_from(name)?;
        // `null` clears a field; the empty string is the same request written
        // the other way, and an agent sends both.
        match value {
            Value::Null => edit = edit.clear(field),
            Value::String(s) if s.is_empty() => edit = edit.clear(field),
            Value::String(s) => edit = edit.set(field, s.clone()),
            other => edit = edit.set(field, other.to_string()),
        }
    }
    if edit.fields.is_empty() {
        return Err(KeystoneError::Other("`fields` named nothing".into()));
    }

    let report = pass(ctx, false).run(&[id], move |_, _| Ok(Some(edit.clone())))?;
    Ok(serde_json::to_value(report)?)
}

/// Find and replace across the library, or across named tracks.
pub fn find_replace(ctx: &McpContext, args: &Value) -> Result<Value> {
    let find = str_arg(args, "find")?;
    let replace = opt_str(args, "replace").unwrap_or_default();
    let names = super::strings_arg(args, "fields");
    if names.is_empty() {
        return Err(KeystoneError::Other(
            "`fields` names the tag columns to search".into(),
        ));
    }
    let fields = names
        .iter()
        .map(|n| field_from(n))
        .collect::<Result<Vec<_>>>()?;

    let dry_run = bool_arg(args, "dryRun", true);
    let scope = args.get("trackIds").map(|_| ids_arg(args, "trackIds")).transpose()?;
    let pass = pass(ctx, dry_run);

    // The dry run counts rows in the database rather than opening files, the
    // same way the confirm step in the window does.
    if dry_run {
        let mut report = pass.empty_report();
        report.written = ctx
            .tracks
            .preview_replace(&fields, &find, scope.as_deref())?;
        return Ok(with_note(serde_json::to_value(report)?, true));
    }

    let ids = match scope {
        Some(ids) => ids,
        None => all_track_ids(&ctx.tracks)?,
    };
    let report = pass.run(&ids, |_, snapshot| {
        tags::patterns::global_replace(snapshot, &fields, &find, &replace)
    })?;
    Ok(with_note(serde_json::to_value(report)?, false))
}

pub fn remove_extra_spaces(ctx: &McpContext, args: &Value) -> Result<Value> {
    let ids = ids_arg(args, "trackIds")?;
    let dry_run = bool_arg(args, "dryRun", true);
    let report = pass(ctx, dry_run).run(&ids, |_, snapshot| {
        Ok(Some(tags::patterns::remove_extra_spaces(snapshot)))
    })?;
    Ok(with_note(serde_json::to_value(report)?, dry_run))
}

/// Write Keystone's key/energy marker into the files, per the user's Update
/// Tags settings. Safe to re-run: a track already carrying the right marker
/// composes the same string and is skipped.
pub fn retag(ctx: &McpContext, args: &Value) -> Result<Value> {
    if !ctx.settings.update_tags_enabled {
        return Err(KeystoneError::Other(
            "Update common tags is off in Settings, so there is nothing to write".into(),
        ));
    }
    let dry_run = bool_arg(args, "dryRun", true);
    let settings = ctx.settings.clone();
    let ids = match args.get("trackIds") {
        Some(_) => ids_arg(args, "trackIds")?,
        None => all_track_ids(&ctx.tracks)?,
    };
    let report = pass(ctx, dry_run)
        .run(&ids, |track, snapshot| retag_edit(track, snapshot, &settings))?;
    Ok(with_note(serde_json::to_value(report)?, dry_run))
}

/// The replaced tag bytes kept before each write, newest first.
pub fn list_backups(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    Ok(json!({ "backups": serde_json::to_value(tags::backup::list(&ctx.db, &id)?)? }))
}

pub fn restore_backup(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "trackId")?;
    let backup_id = str_arg(args, "backupId")?;
    let path = track_path(ctx, &id)?;
    tags::backup::restore(&ctx.db, &backup_id, Path::new(&path))?;
    ok()
}

fn track_path(ctx: &McpContext, id: &str) -> Result<String> {
    ctx.tracks
        .get_by_id(id)?
        .map(|t| t.path)
        .ok_or_else(|| KeystoneError::NotFound(format!("no track with id {id}")))
}

/// Says which half of the operation ran, in words rather than a flag an agent
/// has to remember the meaning of.
fn with_note(mut report: Value, dry_run: bool) -> Value {
    if let Some(obj) = report.as_object_mut() {
        obj.insert(
            "dryRun".into(),
            Value::Bool(dry_run),
        );
        obj.insert(
            "note".into(),
            Value::String(
                if dry_run {
                    "nothing was written; call again with dryRun false to apply"
                } else {
                    "written, with the replaced tag bytes kept as a backup"
                }
                .into(),
            ),
        );
    }
    report
}
