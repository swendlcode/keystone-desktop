//! Collections: the sidebar's crates, plus the rule-driven views beside them.
//!
//! Two kinds share one table. A user collection's members are rows in
//! `collection_tracks` and carry an order the DJ set. A smart collection has no
//! rows at all — its membership is a serialized [`TrackFilters`] run on demand,
//! which is what keeps `Improve Tracks` honest as the library changes under it.

use std::sync::Arc;

use rusqlite::params_from_iter;

use crate::db::mapping::{collection_kind, collection_kind_str, new_id, now, placeholders, IN_CHUNK};
use crate::db::query_builder::{self as qb, LIVE_PREDICATE};
use crate::db::DatabasePool;
use crate::error::{KeystoneError, Result};
use crate::models::{Collection, CollectionKind, TrackFilters};

const COLLECTION_COLUMNS: &str = "id, name, kind, rule, color, position, created_at";

pub struct CollectionRepository {
    db: Arc<DatabasePool>,
}

impl CollectionRepository {
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// Ordered by `position`. Every row carries its count, because the sidebar
    /// shows one per row and a count per row would otherwise be a query per row.
    pub fn list(&self) -> Result<Vec<Collection>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLLECTION_COLUMNS} FROM collections ORDER BY position ASC, created_at ASC"
        ))?;
        let rows = stmt.query_map([], row_to_collection)?;
        let mut out: Vec<Collection> = rows.collect::<rusqlite::Result<_>>()?;
        for c in &mut out {
            c.track_count = self.count_for(&conn, c)?;
        }
        Ok(out)
    }

    pub fn get(&self, id: &str) -> Result<Option<Collection>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLLECTION_COLUMNS} FROM collections WHERE id = ?"
        ))?;
        let mut rows = stmt.query([id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let mut c = row_to_collection(row)?;
        c.track_count = self.count_for(&conn, &c)?;
        Ok(Some(c))
    }

    /// A member count that skips rows marked missing, spelled with the same
    /// predicate as the partial indexes: an unmounted drive should not inflate
    /// the number beside the crate's name.
    ///
    /// A smart collection's count comes from running its rule. A rule that no
    /// longer parses counts zero rather than failing the whole sidebar.
    fn count_for(&self, conn: &rusqlite::Connection, c: &Collection) -> Result<usize> {
        if c.kind == CollectionKind::Smart {
            let Some(filters) = c
                .rule
                .as_deref()
                .and_then(|r| serde_json::from_str::<TrackFilters>(r).ok())
            else {
                return Ok(0);
            };
            let (where_sql, params) = qb::build_where(&filters);
            let n: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM tracks WHERE {where_sql}"),
                params_from_iter(params.iter()),
                |r| r.get(0),
            )?;
            return Ok(n as usize);
        }
        let n: i64 = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM collection_tracks ct JOIN tracks t ON t.id = ct.track_id \
                 WHERE ct.collection_id = ? AND t.{LIVE_PREDICATE}"
            ),
            [&c.id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn create(
        &self,
        name: &str,
        kind: CollectionKind,
        rule: Option<&str>,
    ) -> Result<Collection> {
        let conn = self.db.get()?;
        let id = new_id();
        // Appended, not inserted at the top: a `+` on the header adds to the
        // end of a list the user has already ordered.
        conn.execute(
            "INSERT INTO collections (id, name, kind, rule, color, position, created_at) \
             VALUES (?1, ?2, ?3, ?4, NULL, \
               (SELECT COALESCE(MAX(position), -1) + 1 FROM collections), ?5)",
            rusqlite::params![id, name, collection_kind_str(kind), rule, now()],
        )?;
        self.get(&id)?
            .ok_or_else(|| KeystoneError::NotFound(format!("collection {id}")))
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<()> {
        self.one_field("name = ?1", id, name)
    }

    pub fn set_color(&self, id: &str, color: Option<&str>) -> Result<()> {
        self.db.get()?.execute(
            "UPDATE collections SET color = ?1 WHERE id = ?2",
            rusqlite::params![color, id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        // `collection_tracks` goes with it through ON DELETE CASCADE; the
        // tracks themselves are untouched, which is the point of a crate.
        self.db
            .get()?
            .execute("DELETE FROM collections WHERE id = ?", [id])?;
        Ok(())
    }

    /// Takes the full ordered id list rather than a from/to pair, so a drag that
    /// crossed several rows is one atomic write and the sidebar cannot show a
    /// half-applied order.
    pub fn reorder(&self, ordered_ids: &[String]) -> Result<()> {
        let mut conn = self.db.get()?;
        let tx = crate::db::write_tx(&mut conn)?;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE collections SET position = ?1 WHERE id = ?2")?;
            for (position, id) in ordered_ids.iter().enumerate() {
                stmt.execute(rusqlite::params![position as i64, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Appends at the end of the existing order. `INSERT OR IGNORE` makes
    /// dropping the same track twice a no-op rather than a constraint error,
    /// which matters because the drop target is a drag away from a duplicate.
    ///
    /// The `WHERE EXISTS` is what makes an id that is no longer in the library
    /// a skip rather than a failure. `OR IGNORE` does not cover it: SQLite's
    /// conflict clause does not apply to FOREIGN KEY constraints, so a missing
    /// `track_id` aborted the statement and rolled the whole transaction back
    /// — ninety-nine good tracks not filed because the hundredth had been
    /// removed since the caller read the list, reported as
    /// `database error: FOREIGN KEY constraint failed`. An agent working from
    /// a list it fetched a minute ago hits that routinely; so does a drag that
    /// races the reconciler.
    ///
    /// Callers that care tell the difference by comparing the returned count
    /// against what they asked for.
    pub fn add_tracks(&self, collection_id: &str, track_ids: &[String]) -> Result<usize> {
        let mut conn = self.db.get()?;
        let tx = crate::db::write_tx(&mut conn)?;
        let mut next: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM collection_tracks WHERE collection_id = ?",
            [collection_id],
            |r| r.get(0),
        )?;
        let mut added = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO collection_tracks \
                 (collection_id, track_id, position, added_at) \
                 SELECT ?1, ?2, ?3, ?4 \
                 WHERE EXISTS (SELECT 1 FROM tracks WHERE id = ?2)",
            )?;
            let stamp = now();
            for track_id in track_ids {
                let n = stmt.execute(rusqlite::params![collection_id, track_id, next, stamp])?;
                // Only advance for a row that actually landed, so a re-drop
                // cannot leave gaps in the order.
                if n > 0 {
                    next += 1;
                    added += 1;
                }
            }
        }
        tx.commit()?;
        Ok(added)
    }

    pub fn remove_tracks(&self, collection_id: &str, track_ids: &[String]) -> Result<usize> {
        let mut conn = self.db.get()?;
        let mut removed = 0usize;
        for chunk in track_ids.chunks(IN_CHUNK) {
            let sql = format!(
                "DELETE FROM collection_tracks WHERE collection_id = ? AND track_id IN ({})",
                placeholders(chunk.len())
            );
            let tx = crate::db::write_tx(&mut conn)?;
            {
                let mut bound: Vec<rusqlite::types::Value> =
                    vec![rusqlite::types::Value::Text(collection_id.to_string())];
                bound.extend(chunk.iter().map(|i| rusqlite::types::Value::Text(i.clone())));
                removed += tx.execute(&sql, params_from_iter(bound.iter()))?;
            }
            tx.commit()?;
        }
        Ok(removed)
    }

    pub fn set_track_positions(&self, collection_id: &str, ordered_ids: &[String]) -> Result<()> {
        let mut conn = self.db.get()?;
        let tx = crate::db::write_tx(&mut conn)?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE collection_tracks SET position = ?1 \
                 WHERE collection_id = ?2 AND track_id = ?3",
            )?;
            for (position, track_id) in ordered_ids.iter().enumerate() {
                stmt.execute(rusqlite::params![position as i64, collection_id, track_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn track_ids(&self, collection_id: &str) -> Result<Vec<String>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT track_id FROM collection_tracks WHERE collection_id = ? \
             ORDER BY position ASC, added_at ASC",
        )?;
        let rows = stmt.query_map([collection_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn one_field(&self, assignment: &str, id: &str, value: &str) -> Result<()> {
        self.db.get()?.execute(
            &format!("UPDATE collections SET {assignment} WHERE id = ?2"),
            rusqlite::params![value, id],
        )?;
        Ok(())
    }
}

fn row_to_collection(r: &rusqlite::Row<'_>) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: collection_kind(&r.get::<_, String>(2)?),
        rule: r.get(3)?,
        color: r.get(4)?,
        position: r.get(5)?,
        // Filled by the caller, which already holds the connection the count
        // needs. Zero here is never what the sidebar sees.
        track_count: 0,
        created_at: r.get(6)?,
    })
}

#[cfg(test)]
#[path = "collection_repo_tests.rs"]
mod tests;
