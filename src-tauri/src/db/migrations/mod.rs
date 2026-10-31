mod m001_initial;

use rusqlite::Connection;

use crate::error::Result;

/// One transaction per migration, applied in order, recorded by name. Adding a
/// migration means appending to this slice; never edit an entry that has
/// shipped, because installs in the field have already recorded its name.
const MIGRATIONS: &[(&str, &str)] = &[("001_initial", m001_initial::SQL)];

pub fn run(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE name = ?1",
                [name],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if already_applied {
            continue;
        }

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, strftime('%s','now'))",
            [name],
        )?;
        tx.commit()?;
        tracing::info!("applied migration: {}", name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn migration_001_applies_and_is_idempotent() {
        let mut conn = migrated();
        // A second run must be a no-op rather than an error, because every
        // launch calls it.
        run(&mut conn).unwrap();
    }

    #[test]
    fn every_table_and_index_exists() {
        let conn = migrated();
        for name in [
            "tracks",
            "analysis_cache",
            "cue_points",
            "beatgrid",
            "track_regions",
            "collections",
            "collection_tracks",
            "watched_folders",
            "tag_backup",
            "export_log",
            "file_ops",
            "tracks_fts",
            "tracks_vocab",
        ] {
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "missing schema object: {name}");
        }

        let indexes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 19, "index count changed; update the schema notes");
    }

    #[test]
    fn fts_follows_the_tracks_table() {
        // The triggers are the whole reason an external-content FTS table does
        // not need a backfill pass later. Guard them.
        let conn = migrated();
        conn.execute(
            "INSERT INTO tracks (id, path, filename, extension, artist, title, date_added, created_at, updated_at, last_seen_at)
             VALUES ('t1', '/x/a.mp3', 'a.mp3', 'mp3', 'Aphex Twin', 'Windowlicker', 0, 0, 0, 0)",
            [],
        )
        .unwrap();
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks_fts WHERE tracks_fts MATCH 'window*'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);

        conn.execute("UPDATE tracks SET title = 'Come to Daddy' WHERE id = 't1'", [])
            .unwrap();
        let stale: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks_fts WHERE tracks_fts MATCH 'window*'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stale, 0, "FTS kept a row the content table no longer has");

        conn.execute("DELETE FROM tracks WHERE id = 't1'", []).unwrap();
        let after_delete: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks_fts WHERE tracks_fts MATCH 'daddy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after_delete, 0);
    }
}
