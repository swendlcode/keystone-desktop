//! Collections: the crates an agent can build.

use serde_json::{json, Value};

use super::{ids_arg, ok, str_arg};
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::models::CollectionKind;

pub fn list(ctx: &McpContext) -> Result<Value> {
    Ok(json!({ "collections": serde_json::to_value(ctx.collections.list()?)? }))
}

/// Creates a crate, optionally with its tracks already in order.
///
/// `trackIds` is accepted here so building a set is one call rather than
/// create-then-add: the order they arrive in is the order they are stored in,
/// which is what `add_tracks` does with `MAX(position) + 1`.
pub fn create(ctx: &McpContext, args: &Value) -> Result<Value> {
    let name = clean_name(str_arg(args, "name")?)?;
    let collection = ctx.collections.create(&name, CollectionKind::User, None)?;

    let mut out = serde_json::to_value(&collection)?;
    if let Ok(ids) = ids_arg(args, "trackIds") {
        let added = ctx.collections.add_tracks(&collection.id, &ids)?;
        out["added"] = json!(added);
        if added < ids.len() {
            out["skipped"] = json!(ids.len() - added);
            out["note"] = json!("skipped ids are not in the library");
        }
    }
    Ok(out)
}

/// A collection's tracks **in playing order**, which is what a crate is for.
///
/// `list_tracks` with `collectionId` answers a different question: it sorts by
/// a column, so it can tell you what is in the crate but never what order the
/// set runs in. Position is not one of the sortable columns, deliberately —
/// it only means anything within one collection.
pub fn tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    let collection = must_exist(ctx, &id)?;
    let ordered = ctx.collections.track_ids(&id)?;

    // `get_many` answers in whatever order the rows come back, so the ids are
    // the ordering authority and the rows are looked up through them.
    let rows = ctx.tracks.get_many(&ordered)?;
    let by_id: std::collections::HashMap<&str, &crate::models::Track> =
        rows.iter().map(|t| (t.id.as_str(), t)).collect();

    let tracks: Vec<Value> = ordered
        .iter()
        .enumerate()
        .filter_map(|(position, id)| {
            let track = by_id.get(id.as_str())?;
            let mut row = super::library::brief_of(track);
            row["position"] = json!(position);
            Some(row)
        })
        .collect();

    Ok(json!({
        "collection": collection.name,
        "total": tracks.len(),
        "tracks": tracks,
    }))
}

/// Puts a collection's tracks in the given order.
///
/// Every id in the collection must be named: a partial list would leave the
/// unnamed ones holding positions among the new ones, which reads as a
/// shuffle rather than as the order that was asked for.
pub fn reorder(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    let wanted = ids_arg(args, "trackIds")?;
    refuse_smart(ctx, &id, "reordered")?;

    let current = ctx.collections.track_ids(&id)?;
    let known: std::collections::HashSet<&str> = current.iter().map(String::as_str).collect();
    let unknown: Vec<&String> = wanted.iter().filter(|w| !known.contains(w.as_str())).collect();
    if !unknown.is_empty() {
        return Err(KeystoneError::Other(format!(
            "{} of those tracks are not in the collection; add them first",
            unknown.len()
        )));
    }
    if wanted.len() != current.len() {
        return Err(KeystoneError::Other(format!(
            "the collection holds {} tracks and {} were listed; list every one in the order you want",
            current.len(),
            wanted.len()
        )));
    }

    ctx.collections.set_track_positions(&id, &wanted)?;
    Ok(json!({ "ok": true, "ordered": wanted.len() }))
}

pub fn rename(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    let name = clean_name(str_arg(args, "name")?)?;
    refuse_smart(ctx, &id, "renamed")?;
    ctx.collections.rename(&id, &name)?;
    ok()
}

pub fn delete(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    refuse_smart(ctx, &id, "deleted")?;
    ctx.collections.delete(&id)?;
    // Worth saying back, because an agent relaying this to someone should be
    // able to say which half happened.
    Ok(json!({ "ok": true, "note": "the collection is gone; its tracks and files are not" }))
}

pub fn add_tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    let ids = ids_arg(args, "trackIds")?;
    refuse_smart(ctx, &id, "added to")?;

    let added = ctx.collections.add_tracks(&id, &ids)?;
    // `added` is lower than what was asked for when a track was already in the
    // crate or is no longer in the library. Saying so is the difference
    // between an agent reporting "filed 100" and "filed 98, two were gone".
    let skipped = ids.len().saturating_sub(added);
    let mut out = json!({ "added": added, "requested": ids.len() });
    if skipped > 0 {
        out["skipped"] = json!(skipped);
        out["note"] = json!(
            "skipped tracks were already in the collection, or are no longer in the library"
        );
    }
    Ok(out)
}

pub fn remove_tracks(ctx: &McpContext, args: &Value) -> Result<Value> {
    let id = str_arg(args, "collectionId")?;
    let ids = ids_arg(args, "trackIds")?;
    refuse_smart(ctx, &id, "removed from")?;
    Ok(json!({ "removed": ctx.collections.remove_tracks(&id, &ids)? }))
}

fn clean_name(name: String) -> Result<String> {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(KeystoneError::Other("a collection needs a name".into()));
    }
    Ok(name)
}

fn must_exist(ctx: &McpContext, id: &str) -> Result<crate::models::Collection> {
    ctx.collections
        .get(id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no collection with id {id}")))
}

/// A smart collection's membership is a rule, so writing to it would appear
/// to work and then not. The app greys these out; an agent gets the sentence.
fn refuse_smart(ctx: &McpContext, id: &str, verb: &str) -> Result<()> {
    let collection = ctx
        .collections
        .get(id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("no collection with id {id}")))?;
    if collection.kind == CollectionKind::Smart {
        return Err(KeystoneError::Other(format!(
            "`{}` is a smart collection: its membership is a rule, so it cannot be {verb}",
            collection.name
        )));
    }
    Ok(())
}
