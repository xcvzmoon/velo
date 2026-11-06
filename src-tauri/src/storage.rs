use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, params};
use velo_contracts::{HistoryEntry, SavedConfiguration};
use velo_engine::{RunConfig, RunError, RunSnapshot, RunUpdate};

pub(crate) struct Store {
    connection: Connection,
    _ownership: Option<std::fs::File>,
}

pub(crate) fn failure() -> RunError {
    RunError {
        code: "STORAGE_UNAVAILABLE".into(),
        message: "Local history could not be read or written. Check available disk space and permissions.".into(),
        field: None,
    }
}

impl Store {
    pub(crate) fn open(path: &Path) -> Result<Self, RunError> {
        let ownership = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("sqlite3.lock"))
            .map_err(|_| failure())?;
        ownership.try_lock().map_err(|_| failure())?;
        let connection = Connection::open(path).map_err(|_| failure())?;
        let mut store = Self::initialize(connection)?;
        store._ownership = Some(ownership);
        Ok(store)
    }

    fn initialize(connection: Connection) -> Result<Self, RunError> {
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(|_| failure())?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             PRAGMA secure_delete=ON;
             CREATE TABLE IF NOT EXISTS runs (
               id TEXT PRIMARY KEY, method TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()),
               snapshot TEXT NOT NULL, terminal INTEGER NOT NULL DEFAULT 0,
               interrupted INTEGER NOT NULL DEFAULT 0, incomplete INTEGER NOT NULL DEFAULT 0,
               stride INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE IF NOT EXISTS points (
               run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
               sequence INTEGER NOT NULL, payload TEXT NOT NULL,
               PRIMARY KEY(run_id, sequence));
             CREATE TABLE IF NOT EXISTS configurations (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, remembered INTEGER NOT NULL,
               config TEXT NOT NULL);
             UPDATE runs SET interrupted=1, incomplete=1 WHERE terminal=0;",
            )
            .map_err(|_| failure())?;
        Ok(Self {
            connection,
            _ownership: None,
        })
    }

    pub(crate) fn begin(
        &self,
        id: &str,
        method: &str,
        snapshot: &RunSnapshot,
    ) -> Result<(), RunError> {
        self.connection
            .execute(
                "INSERT INTO runs(id,snapshot,method) VALUES (?1,?2,?3)",
                params![
                    id,
                    serde_json::to_string(snapshot).map_err(|_| failure())?,
                    method
                ],
            )
            .map_err(|_| failure())?;
        Ok(())
    }

    pub(crate) fn record(
        &mut self,
        id: &str,
        update: &RunUpdate,
        incomplete: bool,
    ) -> Result<(), RunError> {
        let transaction = self.connection.transaction().map_err(|_| failure())?;
        let stride: u32 = transaction
            .query_row("SELECT stride FROM runs WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .map_err(|_| failure())?;
        let terminal = matches!(
            update.snapshot.state,
            velo_engine::RunState::Completed
                | velo_engine::RunState::Cancelled
                | velo_engine::RunState::Failed
        );
        let sequence = update.sequence.parse::<i64>().map_err(|_| failure())?;
        transaction.execute("UPDATE runs SET snapshot=?2, terminal=?3, incomplete=MAX(incomplete,?4) WHERE id=?1", params![id, serde_json::to_string(&update.snapshot).map_err(|_| failure())?, terminal, incomplete]).map_err(|_| failure())?;
        if sequence % i64::from(stride) == 0 {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO points VALUES (?1,?2,?3)",
                    params![
                        id,
                        sequence,
                        serde_json::to_string(update).map_err(|_| failure())?
                    ],
                )
                .map_err(|_| failure())?;
            let count: u32 = transaction
                .query_row("SELECT COUNT(*) FROM points WHERE run_id=?1", [id], |row| {
                    row.get(0)
                })
                .map_err(|_| failure())?;
            if count > 600 {
                let next = stride.checked_mul(2).ok_or_else(failure)?;
                transaction
                    .execute(
                        "DELETE FROM points WHERE run_id=?1 AND sequence % ?2 != 0",
                        params![id, next],
                    )
                    .map_err(|_| failure())?;
                transaction
                    .execute("UPDATE runs SET stride=?2 WHERE id=?1", params![id, next])
                    .map_err(|_| failure())?;
            }
        }
        transaction.commit().map_err(|_| failure())
    }

    pub(crate) fn history(&self, before: i64) -> Result<Vec<HistoryEntry>, RunError> {
        let mut statement = self.connection.prepare("SELECT id,created_at,interrupted,incomplete,stride,snapshot,method FROM runs WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 25").map_err(|_| failure())?;
        let rows = statement
            .query_map([before], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|_| failure())?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, created_at, interrupted, incomplete, stride, snapshot, method) =
                row.map_err(|_| failure())?;
            entries.push(HistoryEntry {
                method,
                id,
                created_at: created_at.to_string(),
                interrupted,
                incomplete,
                stride,
                snapshot: serde_json::from_str(&snapshot).map_err(|_| failure())?,
                points: Vec::new(),
            });
        }
        Ok(entries)
    }

    pub(crate) fn cursor(&self, id: &str) -> Result<i64, RunError> {
        self.connection
            .query_row("SELECT rowid FROM runs WHERE id=?1", [id], |row| row.get(0))
            .map_err(|_| failure())
    }

    pub(crate) fn detail(&self, id: &str) -> Result<HistoryEntry, RunError> {
        let cursor = self.cursor(id)?;
        let mut entry = self
            .history(cursor.saturating_add(1))?
            .into_iter()
            .next()
            .ok_or_else(failure)?;
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM points WHERE run_id=?1 ORDER BY sequence LIMIT 600")
            .map_err(|_| failure())?;
        let rows = statement
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(|_| failure())?;
        for row in rows {
            entry
                .points
                .push(serde_json::from_str(&row.map_err(|_| failure())?).map_err(|_| failure())?);
        }
        Ok(entry)
    }

    pub(crate) fn delete_run(&self, id: &str) -> Result<(), RunError> {
        self.connection
            .execute(
                "DELETE FROM runs WHERE id=?1 AND (terminal=1 OR interrupted=1)",
                [id],
            )
            .map_err(|_| failure())?;
        Ok(())
    }

    pub(crate) fn save(
        &self,
        id: &str,
        name: &str,
        remembered: bool,
        config: &RunConfig,
    ) -> Result<(), RunError> {
        self.connection
            .execute(
                "INSERT INTO configurations VALUES (?1,?2,?3,?4)",
                params![
                    id,
                    name,
                    remembered,
                    serde_json::to_string(config).map_err(|_| failure())?
                ],
            )
            .map_err(|_| failure())?;
        Ok(())
    }

    pub(crate) fn configurations(
        &self,
        before: Option<&str>,
    ) -> Result<Vec<SavedConfiguration>, RunError> {
        let cursor: i64 = if let Some(id) = before {
            self.connection
                .query_row(
                    "SELECT rowid FROM configurations WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(|_| failure())?
        } else {
            i64::MAX
        };
        let mut statement = self.connection.prepare("SELECT id,name,remembered,config FROM configurations WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 25").map_err(|_| failure())?;
        let rows = statement
            .query_map([cursor], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|_| failure())?;
        let mut result = Vec::new();
        for row in rows {
            let (id, name, remembered, config) = row.map_err(|_| failure())?;
            result.push(SavedConfiguration {
                id,
                name,
                remembered,
                config: serde_json::from_str(&config).map_err(|_| failure())?,
            });
        }
        Ok(result)
    }

    pub(crate) fn configuration(&self, id: &str) -> Result<SavedConfiguration, RunError> {
        let row = self
            .connection
            .query_row(
                "SELECT name,remembered,config FROM configurations WHERE id=?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| failure())?
            .ok_or_else(failure)?;
        Ok(SavedConfiguration {
            id: id.into(),
            name: row.0,
            remembered: row.1,
            config: serde_json::from_str(&row.2).map_err(|_| failure())?,
        })
    }

    pub(crate) fn delete_configuration(&self, id: &str) -> Result<(), RunError> {
        self.connection
            .execute("DELETE FROM configurations WHERE id=?1", [id])
            .map_err(|_| failure())?;
        Ok(())
    }
}

pub(crate) fn redacted(config: &RunConfig, origin: &str) -> RunConfig {
    RunConfig {
        url: format!("{origin}/"),
        headers: Vec::new(),
        query: Vec::new(),
        auth: velo_engine::RequestAuth::None,
        body: velo_engine::RequestBody::None,
        ..config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velo_engine::{RunManager, Workload};

    fn sample() -> RunUpdate {
        tauri::async_runtime::block_on(async {
            let manager = RunManager::default();
            let snapshot = manager
                .start(
                    RunConfig {
                        url: "http://127.0.0.1:9/private?token=secret".into(),
                        workload: Workload::Constant {
                            vus: 1,
                            duration_ms: 1,
                        },
                        grace_ms: 0,
                        ..RunConfig::default()
                    },
                    true,
                )
                .await
                .unwrap();
            manager.shutdown().await;
            manager.sample(&snapshot.run_id).await.unwrap()
        })
    }

    #[test]
    fn save_load_redacts_all_request_data_and_preserves_thresholds() {
        let store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let config = RunConfig {
            url: "https://example.com/private?token=secret".into(),
            method: velo_engine::RequestMethod::POST,
            headers: vec![velo_engine::NameValue {
                name: "x-private".into(),
                value: "secret".into(),
            }],
            query: vec![velo_engine::NameValue {
                name: "token".into(),
                value: "secret".into(),
            }],
            auth: velo_engine::RequestAuth::Bearer {
                token: "secret".into(),
            },
            body: velo_engine::RequestBody::Text("secret".into()),
            thresholds: vec![velo_engine::Threshold {
                metric: velo_engine::ThresholdMetric::RequestRate,
                operator: velo_engine::ThresholdOperator::GreaterThan,
                limit: 1.0,
            }],
            ..RunConfig::default()
        };
        let clean = redacted(&config, "https://example.com");
        store.save("saved", "Test", false, &clean).unwrap();
        let loaded = store.configuration("saved").unwrap();
        assert!(!loaded.config.to_string().contains("secret"));
        assert!(!loaded.config.to_string().contains("private"));
        assert_eq!(loaded.config["url"], "https://example.com/");
        assert_eq!(loaded.config["thresholds"].as_array().unwrap().len(), 1);
        assert_eq!(store.configurations(None).unwrap().len(), 1);
        store.delete_configuration("saved").unwrap();
        assert!(store.configurations(None).unwrap().is_empty());
    }

    #[test]
    fn bounded_series_transaction_rollback_and_cascade_deletion() {
        let mut store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let mut update = sample();
        store.begin("run", "GET", &update.snapshot).unwrap();
        for sequence in 1..=2401 {
            update.sequence = sequence.to_string();
            store.record("run", &update, sequence == 50).unwrap();
        }
        let entry = store.detail("run").unwrap();
        assert!(entry.points.len() <= 600);
        assert_eq!(entry.stride, 4);
        assert!(entry.incomplete);
        assert!(!entry.snapshot.to_string().contains("secret"));
        let before = entry.snapshot;
        store.connection.execute_batch("CREATE TRIGGER reject_point BEFORE INSERT ON points BEGIN SELECT RAISE(ABORT, 'disk failure'); END;").unwrap();
        update.sequence = "2404".into();
        update.snapshot.actual_us += 100;
        assert_eq!(
            store.record("run", &update, false).unwrap_err().code,
            "STORAGE_UNAVAILABLE"
        );
        assert_eq!(store.detail("run").unwrap().snapshot, before);
        store.delete_run("run").unwrap();
        let count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM points", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(store.history(i64::MAX).unwrap().is_empty());
    }

    #[test]
    fn restart_recovers_interrupted_runs_and_pages_without_loading_series() {
        let mut update = sample();
        update.snapshot.state = velo_engine::RunState::Running;
        let store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        for index in 0..30 {
            store
                .begin(&format!("run-{index}"), "GET", &update.snapshot)
                .unwrap();
        }
        let first = store.history(i64::MAX).unwrap();
        assert_eq!(first.len(), 25);
        assert!(first.iter().all(|entry| entry.points.is_empty()));
        let cursor = store.cursor(&first.last().unwrap().id).unwrap();
        assert_eq!(store.history(cursor).unwrap().len(), 5);
        store.delete_run("run-0").unwrap();
        assert!(store.detail("run-0").is_ok());
        let recovered = Store::initialize(store.connection).unwrap();
        assert!(recovered.detail("run-0").unwrap().interrupted);
        recovered.delete_run("run-0").unwrap();
        assert!(recovered.detail("run-0").is_err());
    }

    #[test]
    fn read_only_storage_returns_safe_failures_without_modification() {
        let store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        store
            .connection
            .execute_batch("PRAGMA query_only=ON;")
            .unwrap();
        let error = store
            .save("id", "name", false, &RunConfig::default())
            .unwrap_err();
        assert_eq!(error.code, "STORAGE_UNAVAILABLE");
        assert!(!error.to_string().contains("readonly"));
        assert!(store.configurations(None).unwrap().is_empty());
        assert!(Store::open(Path::new("/dev/null/velo.sqlite3")).is_err());
    }
    #[test]
    fn database_reopen_retains_summaries_and_recovers_active_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let mut update = sample();
        {
            let mut store = Store::open(&path).unwrap();
            store.begin("terminal", "GET", &update.snapshot).unwrap();
            store.record("terminal", &update, false).unwrap();
            update.snapshot.state = velo_engine::RunState::Running;
            store.begin("active", "GET", &update.snapshot).unwrap();
            store
                .save("saved", "Example", false, &RunConfig::default())
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert!(!store.detail("terminal").unwrap().interrupted);
        assert!(store.detail("active").unwrap().interrupted);
        assert_eq!(store.configurations(None).unwrap().len(), 1);
    }
    #[test]
    fn another_instance_cannot_recover_or_delete_an_owned_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let first = Store::open(&path).unwrap();
        assert!(Store::open(&path).is_err());
        drop(first);
        assert!(Store::open(&path).is_ok());
    }
}
