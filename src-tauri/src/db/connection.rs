use std::path::Path;
use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::Result;

pub type PooledConn = r2d2::PooledConnection<SqliteConnectionManager>;

pub struct DatabasePool {
    pool: Pool<SqliteConnectionManager>,
}

impl DatabasePool {
    pub fn open(db_path: &Path) -> Result<Arc<Self>> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let manager = SqliteConnectionManager::file(db_path).with_init(|c: &mut Connection| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA mmap_size = 268435456;
                 PRAGMA cache_size = -32000;
                 PRAGMA busy_timeout = 10000;
                 PRAGMA wal_autocheckpoint = 1000;",
            )
        });

        // Deliberately small. WAL gives concurrent readers but a single writer,
        // so a bigger pool only multiplies write contention. The analysis
        // workers never touch this pool at all; they hand results to one
        // dedicated writer task.
        let pool = Pool::builder().max_size(4).build(manager)?;

        let mut conn = pool.get()?;
        super::migrations::run(&mut conn)?;

        Ok(Arc::new(Self { pool }))
    }

    pub fn get(&self) -> Result<PooledConn> {
        Ok(self.pool.get()?)
    }
}

/// A transaction that is going to write, taking the write lock up front.
///
/// **Every transaction that writes must use this, not `conn.transaction()`.**
///
/// `Connection::transaction()` is DEFERRED: it takes a read lock on the first
/// read and tries to upgrade to a write lock on the first write. SQLite cannot
/// perform that upgrade if another connection has committed since the read
/// began, and it returns `SQLITE_BUSY` *immediately* — the busy handler is
/// deliberately not invoked, because making two readers wait to upgrade would
/// deadlock them both. So `busy_timeout` does not cover this case at all,
/// which is the opposite of what the pragma list suggests.
///
/// The symptom was `database error: database is locked` from
/// `collection_repo::add_tracks`, which reads `MAX(position)` and then
/// inserts: six concurrent writers lost 7 of ~180 calls. The same shape is in
/// the app, not just in the MCP server — filing tracks into a crate while the
/// indexer is writing is two connections doing exactly this.
///
/// `Immediate` takes the write lock before any statement runs, and *then*
/// `busy_timeout` applies, so a contended writer waits its turn instead of
/// failing.
pub fn write_tx(conn: &mut Connection) -> Result<Transaction<'_>> {
    Ok(conn.transaction_with_behavior(TransactionBehavior::Immediate)?)
}
