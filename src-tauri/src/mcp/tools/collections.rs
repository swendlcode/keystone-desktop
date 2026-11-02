//! Collections: the crates an agent can build.

use serde_json::{json, Value};

use super::{ids_arg, ok, str_arg};
use crate::error::{KeystoneError, Result};
use crate::mcp::McpContext;
use crate::models::CollectionKind;

pub fn list(ctx: &McpContext) -> Result<Value> {
    Ok(json!({ "collections": serde_json::to_value(ctx.collections.list()?)? }))
}

pub fn create(ctx: &McpContext, args: &Value) -> Result<Value> {
    let name = clean_name(str_arg(args, "name")?)?;
    Ok(serde_json::to_value(
        ctx.collections.create(&name, CollectionKind::User, None)?,
    )?)
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
