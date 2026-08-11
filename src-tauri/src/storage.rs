use crate::models::{AppSettings, TerminalSession, ToolRecord};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;

pub struct Storage {
    connection: Mutex<Connection>,
}

impl Storage {
    pub fn new(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS settings (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS tools (
                   id TEXT PRIMARY KEY,
                   data_json TEXT NOT NULL,
                   last_seen TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS sessions (
                   id TEXT PRIMARY KEY,
                   tool_id TEXT NOT NULL,
                   executable TEXT NOT NULL,
                   args_json TEXT NOT NULL,
                   cwd TEXT NOT NULL,
                   started_at TEXT NOT NULL,
                   ended_at TEXT,
                   exit_code INTEGER
                 );
                 CREATE TABLE IF NOT EXISTS updates (
                   id TEXT PRIMARY KEY,
                   tool_id TEXT NOT NULL,
                   display_command TEXT NOT NULL,
                   started_at TEXT NOT NULL,
                   exit_code INTEGER
                 );",
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn cache_tools(&self, tools: &[ToolRecord]) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("DELETE FROM tools", [])
            .map_err(|error| error.to_string())?;
        for tool in tools {
            let data = serde_json::to_string(tool).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO tools(id, data_json, last_seen) VALUES(?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET data_json=excluded.data_json, last_seen=excluded.last_seen",
                    params![tool.id, data, now],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn load_settings(&self) -> AppSettings {
        let connection = self.connection.lock();
        let value: Option<String> = connection
            .query_row("SELECT value FROM settings WHERE key='app'", [], |row| {
                row.get(0)
            })
            .ok();
        value
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let value = serde_json::to_string(settings).map_err(|error| error.to_string())?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO settings(key, value) VALUES('app', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [value],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn session_started(&self, session: &TerminalSession) -> Result<(), String> {
        let args = serde_json::to_string(&session.args).map_err(|error| error.to_string())?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO sessions(id, tool_id, executable, args_json, cwd, started_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    session.id,
                    session.tool_id,
                    session.executable,
                    args,
                    session.cwd,
                    session.started_at
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn session_ended(&self, id: &str, exit_code: Option<i32>) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "UPDATE sessions SET ended_at=?1, exit_code=?2 WHERE id=?3",
                params![chrono::Utc::now().to_rfc3339(), exit_code, id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn settings_round_trip() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        let settings = AppSettings {
            show_system_tools: true,
            show_all_executables: false,
            tray_enabled: false,
            default_working_directory: "/tmp".into(),
        };
        storage.save_settings(&settings).unwrap();
        assert_eq!(storage.load_settings().default_working_directory, "/tmp");
    }
}
