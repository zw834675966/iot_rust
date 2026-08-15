use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

/// Embedded SQLite historian (Connection is Send but not Sync, so wrap in Mutex)
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (or create) the SQLite database and ensure schema exists
    pub fn open(path: &str) -> Result<Self> {
        let p = Path::new(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tag_history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                tag_name    TEXT    NOT NULL,
                value       REAL    NOT NULL,
                timestamp   INTEGER NOT NULL,
                quality     TEXT    NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tag_history_tag_ts
             ON tag_history (tag_name, timestamp)",
            [],
        )?;
        info!("SQLite historian ready at {path}");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert one tag value
    pub fn insert(&self, tag_name: &str, value: f64, timestamp: i64, quality: &str) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute(
            "INSERT INTO tag_history (tag_name, value, timestamp, quality) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tag_name, value, timestamp, quality],
        )?;
        Ok(())
    }

    /// Query the most recent N records for a tag
    pub fn recent(&self, tag_name: &str, limit: u32) -> Result<Vec<(i64, f64, String)>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT timestamp, value, quality FROM tag_history
             WHERE tag_name = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![tag_name, limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_recent() {
        let path = std::env::temp_dir().join(format!("iot_gateway_hist_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(path.to_str().expect("utf-8 temp path")).unwrap();
        db.insert("register1", 42.0, 1_700_000_000_000, "good")
            .unwrap();
        let rows = db.recent("register1", 8).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 1_700_000_000_000);
        assert_eq!(rows[0].1, 42.0);
        assert_eq!(rows[0].2, "good");
        let _ = std::fs::remove_file(&path);
    }
}
