use crate::models::{
    AppSettings, AuthenticationStatus, Capability, DiagnosticReport, Installation,
    LatestVersionInfo, Provider, ResourceCatalog, SessionHistoryRecord, ToolRecord,
    UpdateHistoryRecord, UpdatePlan,
};
use crate::resources::DiscoveredResources;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
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
                 PRAGMA secure_delete=ON;
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
                   cwd TEXT NOT NULL,
                   started_at TEXT NOT NULL,
                   ended_at TEXT,
                   exit_code INTEGER
                 );
                 CREATE TABLE IF NOT EXISTS updates (
                   id TEXT PRIMARY KEY,
                   operation TEXT NOT NULL DEFAULT 'update',
                   tool_id TEXT NOT NULL,
                   adapter_id TEXT,
                   tool_name TEXT NOT NULL DEFAULT '',
                   source TEXT NOT NULL DEFAULT '',
                   display_command TEXT NOT NULL,
                   current_version TEXT,
                   target_version TEXT NOT NULL DEFAULT 'latest',
                   observed_version TEXT,
                   session_id TEXT,
                   started_at TEXT NOT NULL,
                   finished_at TEXT,
                   verified_at TEXT,
                   exit_code INTEGER,
                   status TEXT NOT NULL DEFAULT 'running',
                   message TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE IF NOT EXISTS latest_checks (
                   target_id TEXT PRIMARY KEY,
                   current_version TEXT,
                   latest_version TEXT,
                   update_available INTEGER,
                   checked_at TEXT NOT NULL,
                   source TEXT NOT NULL,
                   status TEXT NOT NULL,
                   message TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS trusted_adapters (
                   path TEXT PRIMARY KEY,
                   fingerprint TEXT NOT NULL,
                   trusted_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS providers (
                   id TEXT PRIMARY KEY,
                   data_json TEXT NOT NULL,
                   last_seen TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS installations (
                   id TEXT PRIMARY KEY,
                   provider_id TEXT NOT NULL REFERENCES providers(id),
                   tool_id TEXT NOT NULL UNIQUE,
                   data_json TEXT NOT NULL,
                   last_seen TEXT NOT NULL,
                   active INTEGER NOT NULL DEFAULT 1
                 );
                 CREATE TABLE IF NOT EXISTS capabilities (
                   id TEXT PRIMARY KEY,
                   provider_id TEXT NOT NULL REFERENCES providers(id),
                   installation_id TEXT NOT NULL REFERENCES installations(id),
                   data_json TEXT NOT NULL,
                   last_seen TEXT NOT NULL,
                   available INTEGER NOT NULL DEFAULT 1
                 );
                 CREATE TABLE IF NOT EXISTS authentication_status (
                   tool_id TEXT PRIMARY KEY,
                   data_json TEXT NOT NULL,
                   checked_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS diagnostic_reports (
                   tool_id TEXT PRIMARY KEY,
                   data_json TEXT NOT NULL,
                   checked_at TEXT NOT NULL
                 );",
            )
            .map_err(|error| error.to_string())?;
        migrate_session_arguments(&connection)?;
        migrate_session_metadata(&connection)?;
        migrate_updates(&connection)?;
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

    pub fn sync_discovered_resources(&self, resources: &DiscoveredResources) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE installations SET active=0", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE capabilities SET available=0", [])
            .map_err(|error| error.to_string())?;
        for provider in &resources.providers {
            let json = serde_json::to_string(provider).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO providers(id, data_json, last_seen) VALUES(?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET data_json=excluded.data_json, last_seen=excluded.last_seen",
                    params![provider.id, json, now],
                )
                .map_err(|error| error.to_string())?;
        }
        for installation in &resources.installations {
            let json = serde_json::to_string(installation).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO installations(id, provider_id, tool_id, data_json, last_seen, active)
                     VALUES(?1, ?2, ?3, ?4, ?5, 1)
                     ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id,
                       tool_id=excluded.tool_id, data_json=excluded.data_json,
                       last_seen=excluded.last_seen, active=1",
                    params![
                        installation.id,
                        installation.provider_id,
                        installation.tool_id,
                        json,
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        for capability in &resources.capabilities {
            let json = serde_json::to_string(capability).map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO capabilities(
                       id, provider_id, installation_id, data_json, last_seen, available
                     ) VALUES(?1, ?2, ?3, ?4, ?5, 1)
                     ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id,
                       installation_id=excluded.installation_id, data_json=excluded.data_json,
                       last_seen=excluded.last_seen, available=1",
                    params![
                        capability.id,
                        capability.provider_id,
                        capability.installation_id,
                        json,
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn resource_catalog(&self) -> Result<ResourceCatalog, String> {
        let connection = self.connection.lock();
        let installations: Vec<Installation> = load_json_rows(
            &connection,
            "SELECT data_json FROM installations WHERE active=1 ORDER BY data_json",
        )?;
        let capabilities: Vec<Capability> = load_json_rows(
            &connection,
            "SELECT data_json FROM capabilities WHERE available=1 ORDER BY data_json",
        )?;
        let active_provider_ids: std::collections::HashSet<_> = installations
            .iter()
            .map(|installation| installation.provider_id.as_str())
            .collect();
        let providers: Vec<Provider> = load_json_rows::<Provider>(
            &connection,
            "SELECT data_json FROM providers ORDER BY data_json",
        )?
        .into_iter()
        .filter(|provider| active_provider_ids.contains(provider.id.as_str()))
        .collect();
        Ok(ResourceCatalog {
            providers,
            installations,
            capabilities,
        })
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

    pub fn save_scan_settings(&self, show_system: bool, show_all: bool) -> Result<(), String> {
        // Read and patch under the same lock so unrelated preferences stay intact.
        let connection = self.connection.lock();
        let value: Option<String> = connection
            .query_row("SELECT value FROM settings WHERE key='app'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| error.to_string())?;
        let mut settings: AppSettings = match value {
            Some(json) => serde_json::from_str(&json).map_err(|error| error.to_string())?,
            None => AppSettings::default(),
        };
        settings.show_system_tools = show_system;
        settings.show_all_executables = show_all;
        let value = serde_json::to_string(&settings).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO settings(key, value) VALUES('app', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [value],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn session_started(&self, session: &crate::models::TerminalSession) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "INSERT INTO sessions(id, tool_id, executable, cwd, started_at, title, kind)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    session.id,
                    session.tool_id,
                    session.executable,
                    session.cwd,
                    session.started_at,
                    session.title,
                    session.kind
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

    pub fn session_renamed(&self, id: &str, title: &str) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "UPDATE sessions SET title=?1 WHERE id=?2",
                params![title, id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn trusted_adapter_fingerprints(&self) -> HashMap<String, String> {
        let connection = self.connection.lock();
        let Ok(mut statement) =
            connection.prepare("SELECT path, fingerprint FROM trusted_adapters")
        else {
            return HashMap::new();
        };
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    pub fn mark_running_sessions_interrupted(&self) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "UPDATE sessions SET ended_at=?1 WHERE ended_at IS NULL",
                [chrono::Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn session_history(&self, limit: usize) -> Result<Vec<SessionHistoryRecord>, String> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, tool_id, title, kind, executable, cwd, started_at, ended_at, exit_code
                 FROM sessions ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([limit.clamp(1, 200) as i64], |row| {
                Ok(SessionHistoryRecord {
                    id: row.get(0)?,
                    tool_id: row.get(1)?,
                    title: row.get(2)?,
                    kind: row.get(3)?,
                    executable: row.get(4)?,
                    cwd: row.get(5)?,
                    started_at: row.get(6)?,
                    ended_at: row.get(7)?,
                    exit_code: row.get(8)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn save_authentication_status(&self, status: &AuthenticationStatus) -> Result<(), String> {
        let data = serde_json::to_string(status).map_err(|error| error.to_string())?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO authentication_status(tool_id, data_json, checked_at)
                 VALUES(?1, ?2, ?3)
                 ON CONFLICT(tool_id) DO UPDATE SET data_json=excluded.data_json,
                   checked_at=excluded.checked_at",
                params![status.tool_id, data, status.checked_at],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn authentication_statuses(&self) -> Result<Vec<AuthenticationStatus>, String> {
        load_json_rows(
            &self.connection.lock(),
            "SELECT data_json FROM authentication_status ORDER BY checked_at DESC",
        )
    }

    pub fn save_diagnostic_report(&self, report: &DiagnosticReport) -> Result<(), String> {
        let data = serde_json::to_string(report).map_err(|error| error.to_string())?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO diagnostic_reports(tool_id, data_json, checked_at)
                 VALUES(?1, ?2, ?3)
                 ON CONFLICT(tool_id) DO UPDATE SET data_json=excluded.data_json,
                   checked_at=excluded.checked_at",
                params![report.tool_id, data, report.checked_at],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn diagnostic_reports(&self) -> Result<Vec<DiagnosticReport>, String> {
        load_json_rows(
            &self.connection.lock(),
            "SELECT data_json FROM diagnostic_reports ORDER BY checked_at DESC",
        )
    }

    pub fn trust_adapter(&self, path: &str, fingerprint: &str) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "INSERT INTO trusted_adapters(path, fingerprint, trusted_at)
                 VALUES(?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET
                   fingerprint=excluded.fingerprint,
                   trusted_at=excluded.trusted_at",
                params![path, fingerprint, chrono::Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn revoke_adapter(&self, path: &str) -> Result<(), String> {
        self.connection
            .lock()
            .execute("DELETE FROM trusted_adapters WHERE path=?1", [path])
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn save_latest_check(&self, check: &LatestVersionInfo) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "INSERT INTO latest_checks(
                   target_id, current_version, latest_version, update_available,
                   checked_at, source, status, message
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(target_id) DO UPDATE SET
                   current_version=excluded.current_version,
                   latest_version=excluded.latest_version,
                   update_available=excluded.update_available,
                   checked_at=excluded.checked_at,
                   source=excluded.source,
                   status=excluded.status,
                   message=excluded.message",
                params![
                    check.target_id,
                    check.current_version,
                    check.latest_version,
                    check.update_available.map(i64::from),
                    check.checked_at,
                    check.source,
                    check.status,
                    check.message,
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn latest_checks(&self) -> Result<Vec<LatestVersionInfo>, String> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT target_id, current_version, latest_version, update_available,
                        checked_at, source, status, message
                 FROM latest_checks ORDER BY checked_at DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(LatestVersionInfo {
                    target_id: row.get(0)?,
                    current_version: row.get(1)?,
                    latest_version: row.get(2)?,
                    update_available: row.get::<_, Option<i64>>(3)?.map(|value| value != 0),
                    checked_at: row.get(4)?,
                    source: row.get(5)?,
                    status: row.get(6)?,
                    message: row.get(7)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn update_started(&self, plan: &UpdatePlan, session_id: &str) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "INSERT INTO updates(
                   id, operation, tool_id, adapter_id, tool_name, source,
                   display_command, current_version, target_version, session_id,
                   started_at, status, message
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'running', '')",
                params![
                    plan.id,
                    plan.operation,
                    plan.tool_id,
                    plan.adapter_id,
                    plan.tool_name,
                    plan.source,
                    plan.display_command,
                    plan.current_version,
                    plan.target_version,
                    session_id,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn update_context(&self, session_id: &str) -> Result<Option<UpdateContext>, String> {
        let connection = self.connection.lock();
        connection
            .query_row(
                "SELECT id, operation, tool_id, adapter_id, current_version, target_version, status
                 FROM updates WHERE session_id=?1",
                [session_id],
                |row| {
                    Ok(UpdateContext {
                        id: row.get(0)?,
                        operation: row.get(1)?,
                        tool_id: row.get(2)?,
                        adapter_id: row.get(3)?,
                        current_version: row.get(4)?,
                        target_version: row.get(5)?,
                        status: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn finish_update(
        &self,
        id: &str,
        exit_code: Option<i32>,
        observed_version: Option<&str>,
        status: &str,
        message: &str,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .lock()
            .execute(
                "UPDATE updates SET
                   finished_at=?1, verified_at=?1, exit_code=?2,
                   observed_version=?3, status=?4, message=?5
                 WHERE id=?6 AND status='running'",
                params![now, exit_code, observed_version, status, message, id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn update_history(&self, limit: usize) -> Result<Vec<UpdateHistoryRecord>, String> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, operation, tool_id, tool_name, source, display_command,
                        current_version, target_version, observed_version, started_at,
                        finished_at, verified_at, exit_code, status, message
                 FROM updates ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([limit.clamp(1, 500) as i64], |row| {
                Ok(UpdateHistoryRecord {
                    id: row.get(0)?,
                    operation: row.get(1)?,
                    tool_id: row.get(2)?,
                    tool_name: row.get(3)?,
                    source: row.get(4)?,
                    display_command: row.get(5)?,
                    current_version: row.get(6)?,
                    target_version: row.get(7)?,
                    observed_version: row.get(8)?,
                    started_at: row.get(9)?,
                    finished_at: row.get(10)?,
                    verified_at: row.get(11)?,
                    exit_code: row.get(12)?,
                    status: row.get(13)?,
                    message: row.get(14)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn session_exit_code(&self, session_id: &str) -> Result<Option<Option<i32>>, String> {
        let connection = self.connection.lock();
        connection
            .query_row(
                "SELECT exit_code FROM sessions WHERE id=?1 AND ended_at IS NOT NULL",
                [session_id],
                |row| row.get::<_, Option<i32>>(0),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn mark_running_updates_interrupted(&self) -> Result<(), String> {
        self.connection
            .lock()
            .execute(
                "UPDATE updates SET finished_at=?1, status='interrupted',
                  message='CliCove 在验证完成前退出' WHERE status='running'",
                [chrono::Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct UpdateContext {
    pub id: String,
    pub operation: String,
    pub tool_id: String,
    pub adapter_id: Option<String>,
    pub current_version: Option<String>,
    pub target_version: String,
    pub status: String,
}

fn load_json_rows<T: serde::de::DeserializeOwned>(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<T>, String> {
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        let json = row.map_err(|error| error.to_string())?;
        serde_json::from_str(&json).map_err(|error| error.to_string())
    })
    .collect()
}

fn migrate_session_arguments(connection: &Connection) -> Result<(), String> {
    let has_args = connection
        .prepare("PRAGMA table_info(sessions)")
        .and_then(|mut statement| {
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            Ok(columns
                .filter_map(Result::ok)
                .any(|name| name == "args_json"))
        })
        .map_err(|error| error.to_string())?;
    if !has_args {
        return Ok(());
    }
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             DROP TABLE IF EXISTS sessions_secure;
             CREATE TABLE sessions_secure (
               id TEXT PRIMARY KEY,
               tool_id TEXT NOT NULL,
               executable TEXT NOT NULL,
               cwd TEXT NOT NULL,
               started_at TEXT NOT NULL,
               ended_at TEXT,
               exit_code INTEGER,
               title TEXT NOT NULL DEFAULT '',
               kind TEXT NOT NULL DEFAULT 'interactive'
             );
             INSERT INTO sessions_secure(id, tool_id, executable, cwd, started_at, ended_at, exit_code)
               SELECT id, tool_id, executable, cwd, started_at, ended_at, exit_code FROM sessions;
             DROP TABLE sessions;
             ALTER TABLE sessions_secure RENAME TO sessions;
             COMMIT;
             PRAGMA wal_checkpoint(TRUNCATE);
             VACUUM;",
        )
        .map_err(|error| error.to_string())
}

fn migrate_session_metadata(connection: &Connection) -> Result<(), String> {
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(sessions)")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| error.to_string())?;
    for (name, definition) in [
        ("title", "TEXT NOT NULL DEFAULT ''"),
        ("kind", "TEXT NOT NULL DEFAULT 'interactive'"),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute_batch(&format!(
                    "ALTER TABLE sessions ADD COLUMN {name} {definition};"
                ))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn migrate_updates(connection: &Connection) -> Result<(), String> {
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(updates)")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| error.to_string())?;
    let additions = [
        ("operation", "TEXT NOT NULL DEFAULT 'update'"),
        ("adapter_id", "TEXT"),
        ("tool_name", "TEXT NOT NULL DEFAULT ''"),
        ("source", "TEXT NOT NULL DEFAULT ''"),
        ("current_version", "TEXT"),
        ("target_version", "TEXT NOT NULL DEFAULT 'latest'"),
        ("observed_version", "TEXT"),
        ("session_id", "TEXT"),
        ("finished_at", "TEXT"),
        ("verified_at", "TEXT"),
        ("status", "TEXT NOT NULL DEFAULT 'running'"),
        ("message", "TEXT NOT NULL DEFAULT ''"),
    ];
    for (name, definition) in additions {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute_batch(&format!(
                    "ALTER TABLE updates ADD COLUMN {name} {definition};"
                ))
                .map_err(|error| error.to_string())?;
        }
    }
    connection
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS updates_session_id
             ON updates(session_id) WHERE session_id IS NOT NULL;",
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ToolCategory, ToolSource};
    use tempfile::TempDir;

    fn discovered_resources() -> DiscoveredResources {
        let provider = Provider {
            id: "provider".into(),
            display_name: "Provider".into(),
            kind: "first-party".into(),
            category: ToolCategory::AiAgent,
            auth_owner: "external-cli".into(),
            website: None,
            installation_count: 1,
        };
        let source = ToolSource {
            kind: "user".into(),
            label: "User PATH".into(),
            package_name: None,
            package_version: None,
            scope: "user".into(),
        };
        DiscoveredResources {
            providers: vec![provider],
            installations: vec![Installation {
                id: "installation".into(),
                provider_id: "provider".into(),
                tool_id: "tool".into(),
                display_name: "Tool".into(),
                command: "tool".into(),
                executable: "/bin/tool".into(),
                aliases: vec!["tool".into()],
                version: Some("1.0.0".into()),
                source,
                adapter_id: Some("tool".into()),
                status: "ready".into(),
                last_seen: "now".into(),
                active: true,
            }],
            capabilities: vec![
                Capability {
                    id: "invoke".into(),
                    provider_id: "provider".into(),
                    installation_id: "installation".into(),
                    key: "invoke".into(),
                    display_name: "Invoke".into(),
                    kind: "invoke".into(),
                    description: "Invoke tool".into(),
                    risk_level: "controlled".into(),
                    agent_callable: true,
                    available: true,
                },
                Capability {
                    id: "update".into(),
                    provider_id: "provider".into(),
                    installation_id: "installation".into(),
                    key: "update".into(),
                    display_name: "Update".into(),
                    kind: "lifecycle".into(),
                    description: "Update tool".into(),
                    risk_level: "confirmation-required".into(),
                    agent_callable: false,
                    available: true,
                },
            ],
        }
    }

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

    #[test]
    fn scan_settings_preserve_other_preferences_across_reopen() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("clihub.db");
        let storage = Storage::new(&path).unwrap();
        storage
            .save_settings(&AppSettings {
                show_system_tools: false,
                show_all_executables: false,
                tray_enabled: true,
                default_working_directory: "/saved/workspace".into(),
            })
            .unwrap();
        storage.save_scan_settings(true, true).unwrap();
        drop(storage);
        let storage = Storage::new(&path).unwrap();
        let settings = storage.load_settings();
        assert!(settings.show_system_tools && settings.show_all_executables);
        assert!(settings.tray_enabled);
        assert_eq!(settings.default_working_directory, "/saved/workspace");
        storage.save_scan_settings(false, false).unwrap();
        let settings = storage.load_settings();
        assert!(!settings.show_system_tools && !settings.show_all_executables);
        assert!(settings.tray_enabled);
        assert_eq!(settings.default_working_directory, "/saved/workspace");
    }

    #[test]
    fn resource_catalog_works_without_retired_tables() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        storage
            .sync_discovered_resources(&discovered_resources())
            .unwrap();
        let catalog = storage.resource_catalog().unwrap();
        assert_eq!(catalog.providers.len(), 1);
        assert_eq!(catalog.installations.len(), 1);
        assert_eq!(catalog.capabilities.len(), 2);
        let retired_tables: i64 = storage.connection.lock().query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('profiles', 'agent_bindings')",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(retired_tables, 0);
        let json = serde_json::to_value(catalog).unwrap();
        assert!(json.get("profiles").is_none());
        assert!(json.get("agentBindings").is_none());
    }

    #[test]
    fn opening_legacy_database_preserves_retired_records() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("clihub.db");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TABLE profiles (id TEXT PRIMARY KEY, name TEXT);
             CREATE TABLE agent_bindings (id TEXT PRIMARY KEY, profile_id TEXT REFERENCES profiles(id));
             INSERT INTO profiles VALUES ('legacy-profile', 'Keep this record');
             INSERT INTO agent_bindings VALUES ('legacy-binding', 'legacy-profile');"
        ).unwrap();
        drop(connection);
        let storage = Storage::new(&path).unwrap();
        storage
            .sync_discovered_resources(&discovered_resources())
            .unwrap();
        assert_eq!(storage.resource_catalog().unwrap().installations.len(), 1);
        let retained: (String, String) = storage
            .connection
            .lock()
            .query_row(
                "SELECT p.name, b.id FROM profiles p JOIN agent_bindings b ON b.profile_id=p.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            retained,
            ("Keep this record".into(), "legacy-binding".into())
        );
    }

    #[test]
    fn legacy_session_arguments_are_removed_from_schema_and_disk() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("clihub.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE sessions (
                       id TEXT PRIMARY KEY,
                       tool_id TEXT NOT NULL,
                       executable TEXT NOT NULL,
                       args_json TEXT NOT NULL,
                       cwd TEXT NOT NULL,
                       started_at TEXT NOT NULL,
                       ended_at TEXT,
                       exit_code INTEGER
                     );
                     INSERT INTO sessions VALUES(
                       '1', 'tool', '/bin/tool', '[\"--token\",\"sensitive-value\"]',
                       '/tmp', 'now', NULL, NULL
                     );",
                )
                .unwrap();
        }
        let storage = Storage::new(&path).unwrap();
        let columns: Vec<String> = storage
            .connection
            .lock()
            .prepare("PRAGMA table_info(sessions)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(!columns.iter().any(|name| name == "args_json"));
        drop(storage);
        assert!(!String::from_utf8_lossy(&fs::read(path).unwrap()).contains("sensitive-value"));
    }

    #[test]
    fn adapter_trust_is_bound_to_path_and_fingerprint() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        storage
            .trust_adapter("/tmp/adapter.json", "abc123")
            .unwrap();
        assert_eq!(
            storage
                .trusted_adapter_fingerprints()
                .get("/tmp/adapter.json")
                .map(String::as_str),
            Some("abc123")
        );
        storage.revoke_adapter("/tmp/adapter.json").unwrap();
        assert!(storage.trusted_adapter_fingerprints().is_empty());
    }

    #[test]
    fn structured_health_cache_stores_only_normalized_results() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("clihub.db");
        let storage = Storage::new(&path).unwrap();
        storage
            .save_authentication_status(&AuthenticationStatus {
                tool_id: "tool".into(),
                installation: None,
                status: "authenticated".into(),
                method: Some("oauth".into()),
                expires_in_minutes: Some(30),
                checked_at: "now".into(),
                source: "safe status command".into(),
                message: "Authenticated".into(),
                fields: Vec::new(),
            })
            .unwrap();
        storage
            .save_diagnostic_report(&DiagnosticReport {
                tool_id: "tool".into(),
                installation: None,
                status: "healthy".into(),
                checked_at: "now".into(),
                duration_ms: 12,
                summary: "All checks passed".into(),
                checks: Vec::new(),
            })
            .unwrap();
        assert_eq!(storage.authentication_statuses().unwrap().len(), 1);
        assert_eq!(storage.diagnostic_reports().unwrap().len(), 1);
        let schema: String = storage
            .connection
            .lock()
            .prepare(
                "SELECT group_concat(sql, ' ') FROM sqlite_master
                 WHERE type='table' AND name IN ('authentication_status', 'diagnostic_reports')",
            )
            .unwrap()
            .query_row([], |row| row.get(0))
            .unwrap();
        for sensitive in ["raw_output", "token", "password", "secret", "credential"] {
            assert!(!schema.to_ascii_lowercase().contains(sensitive));
        }
    }

    #[test]
    fn session_history_has_title_and_kind_but_no_argument_or_output_columns() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        storage
            .session_started(&crate::models::TerminalSession {
                id: "session".into(),
                tool_id: "tool".into(),
                title: "Workspace".into(),
                executable: "/bin/tool".into(),
                args: vec!["--secret".into()],
                cwd: "/tmp".into(),
                started_at: "now".into(),
                status: "running".into(),
                kind: "interactive".into(),
                ended_at: None,
                exit_code: None,
            })
            .unwrap();
        storage.session_ended("session", Some(0)).unwrap();
        let history = storage.session_history(10).unwrap();
        assert_eq!(history[0].title, "Workspace");
        assert_eq!(history[0].kind, "interactive");
        let schema: String = storage
            .connection
            .lock()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='sessions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!schema.contains("args_json"));
        assert!(!schema.contains("terminal_output"));
    }

    #[test]
    fn startup_marks_abandoned_sessions_interrupted() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        storage
            .session_started(&crate::models::TerminalSession {
                id: "1".into(),
                tool_id: "tool".into(),
                title: "Tool".into(),
                executable: "/bin/tool".into(),
                args: Vec::new(),
                cwd: "/tmp".into(),
                started_at: "now".into(),
                status: "running".into(),
                kind: "interactive".into(),
                ended_at: None,
                exit_code: None,
            })
            .unwrap();
        storage.mark_running_sessions_interrupted().unwrap();
        let ended_at: String = storage
            .connection
            .lock()
            .query_row("SELECT ended_at FROM sessions WHERE id='1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!ended_at.is_empty());
    }

    #[test]
    fn legacy_update_table_is_migrated_and_running_records_are_closed() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("clihub.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE updates (
                       id TEXT PRIMARY KEY,
                       tool_id TEXT NOT NULL,
                       display_command TEXT NOT NULL,
                       started_at TEXT NOT NULL,
                       exit_code INTEGER
                     );
                     INSERT INTO updates VALUES('1', 'tool', 'safe command', 'now', NULL);",
                )
                .unwrap();
        }
        let storage = Storage::new(&path).unwrap();
        storage.mark_running_updates_interrupted().unwrap();
        let history = storage.update_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, "interrupted");
        assert_eq!(history[0].operation, "update");
        let columns: Vec<String> = storage
            .connection
            .lock()
            .prepare("PRAGMA table_info(updates)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(columns.iter().any(|name| name == "observed_version"));
        assert!(columns.iter().any(|name| name == "session_id"));
    }

    #[test]
    fn update_history_records_verified_version_without_terminal_output() {
        let temp = TempDir::new().unwrap();
        let storage = Storage::new(&temp.path().join("clihub.db")).unwrap();
        let plan = UpdatePlan {
            id: "plan-1".into(),
            operation: "update".into(),
            tool_id: "tool-1".into(),
            adapter_id: Some("tool".into()),
            tool_name: "Tool".into(),
            current_version: Some("1.0.0".into()),
            target_version: "1.1.0".into(),
            executable: "/bin/tool".into(),
            args: vec!["update".into()],
            display_command: "/bin/tool update".into(),
            source: "test".into(),
            requires_elevation: false,
        };
        storage.update_started(&plan, "session-1").unwrap();
        storage
            .finish_update(
                "plan-1",
                Some(0),
                Some("tool 1.1.0"),
                "succeeded",
                "verified",
            )
            .unwrap();
        let history = storage.update_history(10).unwrap();
        assert_eq!(history[0].observed_version.as_deref(), Some("tool 1.1.0"));
        assert_eq!(history[0].status, "succeeded");
        let schema: String = storage
            .connection
            .lock()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='updates'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!schema.contains("terminal_output"));
        assert!(!schema.contains("args_json"));
    }
}
