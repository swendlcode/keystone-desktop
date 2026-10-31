//! Stores the replaced tag before every write.
//!
//! This is the difference between RE-TAG EXISTING FILES being a button and
//! being a dare.
//!
//! The blob holds **two** things, and the reason is that lofty has no public
//! way to parse a bare tag block back out of bytes. So:
//!
//! - a JSON snapshot of the fields as they were, which is what [`restore`]
//!   actually applies. That is a complete undo of a Keystone write, because the
//!   write path loads a concrete `Id3v2Tag`/`Ilst`/`VorbisComments`/`ApeTag`,
//!   changes only the modelled fields and saves it back — it never drops the
//!   frames it does not understand, so there is nothing else to put back.
//! - the raw tag block as it was, kept verbatim. Nothing reads it today; it is
//!   the forensic record that lets a later version, or a human with a hex
//!   editor, reconstruct a file byte for byte if a write ever goes wrong in a
//!   way the snapshot cannot describe.
//!
//! Both in one row, because a backup split across two tables is a backup with a
//! way to go missing.

use std::path::Path;
use std::sync::Arc;

use lofty::tag::TagExt;

use super::concrete::{Primary, Tags};
use super::verify;
use super::write::{self, WriteOptions};
use crate::db::DatabasePool;
use crate::error::{KeystoneError, Result};
use crate::models::{Container, TagBackupEntry, TagEdit, TagField, TagSnapshot};

const MAGIC: &[u8; 4] = b"KSTB";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 9;

/// Every field a Keystone write can touch. Restore sets all of them, including
/// the ones that were empty: a field the write *added* has to come back out.
const ALL_FIELDS: [TagField; 9] = [
    TagField::Artist,
    TagField::Title,
    TagField::Album,
    TagField::Genre,
    TagField::Grouping,
    TagField::Comment,
    TagField::Year,
    TagField::Bpm,
    TagField::InitialKey,
];

/// Reads the raw tag block out of the file, unparsed. Captures everything the
/// write would replace, including frames Keystone has no model for.
pub fn raw_tag_bytes(path: &Path, container: Container) -> Result<Vec<u8>> {
    let tags = super::concrete::load(path)?;
    if tags.container != container {
        return Err(super::tag_err(
            path,
            format!(
                "holds a {} tag, not {}",
                tags.container.as_str(),
                container.as_str()
            ),
        ));
    }
    dump(&tags)
}

/// Serialises the primary tag block exactly as it stands. `dump_to` writes the
/// tag and nothing else — it does not produce a usable file, which is precisely
/// what is wanted here.
pub(crate) fn dump(tags: &Tags) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let opts = tags.write_options();
    match &tags.primary {
        Primary::Id3v2(t) => t.dump_to(&mut out, opts),
        Primary::Ilst(t) => t.dump_to(&mut out, opts),
        Primary::Vorbis(t) => t.dump_to(&mut out, opts),
        Primary::Ape(t) => t.dump_to(&mut out, opts),
    }
    .map_err(|e| KeystoneError::Tag(format!("could not capture the existing tag: {e}")))?;
    Ok(out)
}

/// Builds the blob that goes in the row: the snapshot restore applies, plus the
/// raw block for the record.
pub(crate) fn capture(path: &Path, tags: &Tags) -> Result<Vec<u8>> {
    let snapshot = super::read::read(path)?;
    let json = serde_json::to_vec(&snapshot)?;
    let raw = dump(tags)?;

    let mut blob = Vec::with_capacity(HEADER_LEN + json.len() + raw.len());
    blob.extend_from_slice(MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(json.len() as u32).to_le_bytes());
    blob.extend_from_slice(&json);
    blob.extend_from_slice(&raw);
    Ok(blob)
}

fn parse(blob: &[u8]) -> Result<TagSnapshot> {
    let malformed = || KeystoneError::Tag("the stored backup is malformed".into());
    if blob.len() < HEADER_LEN || &blob[..4] != MAGIC {
        return Err(malformed());
    }
    if blob[4] != VERSION {
        return Err(KeystoneError::Tag(format!(
            "this backup was written by format version {}, which this build cannot read",
            blob[4]
        )));
    }
    let len = u32::from_le_bytes(blob[5..9].try_into().map_err(|_| malformed())?) as usize;
    let json = blob.get(HEADER_LEN..HEADER_LEN + len).ok_or_else(malformed)?;
    Ok(serde_json::from_slice(json)?)
}

pub fn save(
    db: &Arc<DatabasePool>,
    track_id: &str,
    container: Container,
    raw: &[u8],
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    db.get()?.execute(
        "INSERT INTO tag_backup (id, track_id, container, raw, written_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![&id, track_id, container.as_str(), raw, now()],
    )?;
    Ok(id)
}

pub fn list(db: &Arc<DatabasePool>, track_id: &str) -> Result<Vec<TagBackupEntry>> {
    let conn = db.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, track_id, container, LENGTH(raw), written_at FROM tag_backup \
         WHERE track_id = ?1 ORDER BY written_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([track_id], |row| {
        Ok(TagBackupEntry {
            id: row.get(0)?,
            track_id: row.get(1)?,
            container: container_from_str(&row.get::<_, String>(2)?),
            size_bytes: row.get::<_, i64>(3)? as usize,
            written_at: row.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(KeystoneError::from)
}

/// Puts the stored values back through the same copy-modify-verify-rename path
/// as any other write, and takes a backup of the current state first so that
/// restoring the wrong one is itself undoable.
pub fn restore(db: &Arc<DatabasePool>, backup_id: &str, path: &Path) -> Result<()> {
    let (track_id, blob): (String, Vec<u8>) = db
        .get()?
        .query_row(
            "SELECT track_id, raw FROM tag_backup WHERE id = ?1",
            [backup_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| KeystoneError::NotFound(format!("tag backup {backup_id}")))?;

    let snapshot = parse(&blob)?;
    let mut edit = TagEdit::default();
    for field in ALL_FIELDS {
        edit.fields
            .push((field, verify::snapshot_field(&snapshot, field)));
    }
    write::write_with_backup(db, &track_id, path, &edit, &WriteOptions::default())?;
    Ok(())
}

/// Keeps the table from growing without bound on a library that gets re-tagged
/// repeatedly. Keeps the newest `keep_per_track` per track.
pub fn prune(db: &Arc<DatabasePool>, keep_per_track: usize) -> Result<usize> {
    let removed = db.get()?.execute(
        "DELETE FROM tag_backup WHERE id IN ( \
            SELECT id FROM ( \
                SELECT id, ROW_NUMBER() OVER ( \
                    PARTITION BY track_id ORDER BY written_at DESC, id DESC \
                ) AS rn FROM tag_backup \
            ) WHERE rn > ?1 \
         )",
        [keep_per_track as i64],
    )?;
    Ok(removed)
}

/// Unknown spellings fall back to ID3v2 rather than failing the whole listing:
/// the container is shown to the user, not used to decide how to write.
fn container_from_str(s: &str) -> Container {
    match s {
        "ilst" => Container::Ilst,
        "vorbis" => Container::VorbisComments,
        "ape" => Container::Ape,
        "riff" => Container::Riff,
        "aiff" => Container::Aiff,
        _ => Container::Id3v2,
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_envelope_round_trips_the_snapshot_and_keeps_the_raw_block() {
        let snapshot = TagSnapshot {
            artist: Some("Swendl".into()),
            title: Some("A - B".into()),
            bpm: Some(128.0),
            initial_key: Some("8A".into()),
            ..TagSnapshot::default()
        };
        let json = serde_json::to_vec(&snapshot).unwrap();
        let raw = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();

        let mut blob = Vec::new();
        blob.extend_from_slice(MAGIC);
        blob.push(VERSION);
        blob.extend_from_slice(&(json.len() as u32).to_le_bytes());
        blob.extend_from_slice(&json);
        blob.extend_from_slice(&raw);

        let back = parse(&blob).unwrap();
        assert_eq!(back.artist.as_deref(), Some("Swendl"));
        assert_eq!(back.title.as_deref(), Some("A - B"));
        assert_eq!(back.initial_key.as_deref(), Some("8A"));
        // The raw block sits after the JSON and is left untouched.
        assert_eq!(&blob[HEADER_LEN + json.len()..], &raw[..]);
    }

    #[test]
    fn a_blob_from_another_format_version_is_refused_not_guessed_at() {
        let mut blob = MAGIC.to_vec();
        blob.push(VERSION + 1);
        blob.extend_from_slice(&0u32.to_le_bytes());
        assert!(parse(&blob).is_err());
        assert!(parse(b"nope").is_err());
    }
}
