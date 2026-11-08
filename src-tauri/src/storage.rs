use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension, params};
use velo_contracts::{HistoryEntry, SavedConfiguration};
#[cfg(test)]
use velo_engine::RunConfig;
use velo_engine::{RunError, RunSnapshot, RunUpdate};

pub(crate) struct Store {
    connection: Connection,
    _ownership: Option<StoreOwnership>,
}

/// Releases database ownership when the store ends, even if a child inherited the descriptor.
struct StoreOwnership(std::fs::File);

impl Drop for StoreOwnership {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            tracing::warn!(code = "STORAGE_UNLOCK_FAILED", kind = ?error.kind(), "Database ownership could not be released explicitly");
        }
    }
}

pub(crate) fn failure() -> RunError {
    RunError {
        code: "STORAGE_UNAVAILABLE".into(),
        message: "Local history could not be read or written. Check available disk space and permissions.".into(),
        field: None,
    }
}

pub(crate) fn failure_from(error: impl std::error::Error + 'static) -> RunError {
    let source: &(dyn std::error::Error + 'static) = &error;
    if let Some(rusqlite::Error::SqliteFailure(code, _)) = source.downcast_ref::<rusqlite::Error>()
    {
        tracing::warn!(
            code = "STORAGE_FAILURE",
            category = "sqlite",
            sqlite_code = code.extended_code,
            "Storage operation failed"
        );
    } else if let Some(error) = source.downcast_ref::<std::io::Error>() {
        tracing::warn!(code = "STORAGE_FAILURE", category = "io", kind = ?error.kind(), "Storage operation failed");
    } else {
        tracing::warn!(
            code = "STORAGE_FAILURE",
            category = "representation",
            "Storage operation failed"
        );
    }
    failure()
}

impl Store {
    pub(crate) fn open(path: &Path) -> Result<Self, RunError> {
        let ownership = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("sqlite3.lock"))
            .map_err(failure_from)?;
        ownership.try_lock().map_err(failure_from)?;
        let connection = Connection::open(path).map_err(failure_from)?;
        let mut store = Self::initialize(connection)?;
        store._ownership = Some(StoreOwnership(ownership));
        Ok(store)
    }

    fn initialize(connection: Connection) -> Result<Self, RunError> {
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(failure_from)?;
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
             CREATE TABLE IF NOT EXISTS run_context (
               run_id TEXT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
               metadata TEXT NOT NULL, identity TEXT, configuration TEXT NOT NULL DEFAULT 'null');
             CREATE TABLE IF NOT EXISTS baseline (slot INTEGER PRIMARY KEY CHECK(slot=1), run_id TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS comparisons (
               candidate_id TEXT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE, settings TEXT NOT NULL,
               verdict TEXT NOT NULL, regressions TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS points (
               run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
               sequence INTEGER NOT NULL, payload TEXT NOT NULL,
               PRIMARY KEY(run_id, sequence));
             CREATE TABLE IF NOT EXISTS configurations (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, remembered INTEGER NOT NULL,
               config TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS definitions (name TEXT PRIMARY KEY, source TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS credential_cleanup (id TEXT PRIMARY KEY);
             UPDATE runs SET interrupted=1, incomplete=1 WHERE terminal=0;",
            )
            .map_err(failure_from)?;
        Ok(Self {
            connection,
            _ownership: None,
        })
    }

    pub(crate) fn save_definition(&self, name: &str, source: &str) -> Result<(), RunError> {
        let allowed: bool = self.connection.query_row("SELECT (SELECT COUNT(*) FROM definitions) < 1000 OR EXISTS(SELECT 1 FROM definitions WHERE name=?1)", [name], |row| row.get(0)).map_err(failure_from)?;
        if !allowed {
            return Err(RunError {
                code: "DEFINITION_LIMIT_REACHED".into(),
                message: "Delete a saved test before adding another. The limit is 1,000 tests."
                    .into(),
                field: Some("name".into()),
            });
        }
        self.connection.execute("INSERT INTO definitions(name, source) VALUES (?1,?2) ON CONFLICT(name) DO UPDATE SET source=excluded.source", params![name, source]).map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn definition_names(&self) -> Result<Vec<String>, RunError> {
        let mut statement = self
            .connection
            .prepare("SELECT name FROM definitions ORDER BY name LIMIT 1000")
            .map_err(failure_from)?;
        statement
            .query_map([], |row| row.get(0))
            .map_err(failure_from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure_from)
    }

    pub(crate) fn definition(&self, name: &str) -> Result<String, RunError> {
        self.connection
            .query_row(
                "SELECT source FROM definitions WHERE name=?1",
                [name],
                |row| row.get(0),
            )
            .map_err(failure_from)
    }

    pub(crate) fn delete_definition(&self, name: &str) -> Result<(), RunError> {
        self.connection
            .execute("DELETE FROM definitions WHERE name=?1", [name])
            .map_err(failure_from)?;
        Ok(())
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
                    serde_json::to_string(snapshot).map_err(failure_from)?,
                    method
                ],
            )
            .map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn record(
        &mut self,
        id: &str,
        update: &RunUpdate,
        incomplete: bool,
    ) -> Result<(), RunError> {
        let transaction = self.connection.transaction().map_err(failure_from)?;
        let stride: u32 = transaction
            .query_row("SELECT stride FROM runs WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .map_err(failure_from)?;
        let terminal = matches!(
            update.snapshot.state,
            velo_engine::RunState::Completed
                | velo_engine::RunState::Cancelled
                | velo_engine::RunState::Failed
                | velo_engine::RunState::Aborted
        );
        let sequence = update.sequence.parse::<i64>().map_err(failure_from)?;
        transaction.execute("UPDATE runs SET snapshot=?2, terminal=?3, incomplete=MAX(incomplete,?4) WHERE id=?1", params![id, serde_json::to_string(&update.snapshot).map_err(failure_from)?, terminal, incomplete]).map_err(failure_from)?;
        if sequence % i64::from(stride) == 0 {
            let mut interval = update.clone();
            for metrics in [
                &mut interval.snapshot.metrics,
                &mut interval.snapshot.evaluated,
            ] {
                metrics.diagnostics.samples.clear();
                metrics.diagnostics.groups.clear();
                metrics.diagnostics.retained_bytes = 0;
            }
            transaction
                .execute(
                    "INSERT OR REPLACE INTO points VALUES (?1,?2,?3)",
                    params![
                        id,
                        sequence,
                        serde_json::to_string(&interval).map_err(failure_from)?
                    ],
                )
                .map_err(failure_from)?;
            let count: u32 = transaction
                .query_row("SELECT COUNT(*) FROM points WHERE run_id=?1", [id], |row| {
                    row.get(0)
                })
                .map_err(failure_from)?;
            if count > 600 {
                let next = stride.checked_mul(2).ok_or_else(failure)?;
                transaction
                    .execute(
                        "DELETE FROM points WHERE run_id=?1 AND sequence % ?2 != 0",
                        params![id, next],
                    )
                    .map_err(failure_from)?;
                transaction
                    .execute("UPDATE runs SET stride=?2 WHERE id=?1", params![id, next])
                    .map_err(failure_from)?;
            }
        }
        transaction.commit().map_err(failure_from)
    }

    pub(crate) fn history(&self, before: i64) -> Result<Vec<HistoryEntry>, RunError> {
        let mut statement = self.connection.prepare("SELECT id,created_at,interrupted,incomplete,stride,snapshot,method FROM runs WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 25").map_err(failure_from)?;
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
            .map_err(failure_from)?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, created_at, interrupted, incomplete, stride, snapshot, method) =
                row.map_err(failure_from)?;
            let (definition_identity, configuration) = self.run_context(&id)?;
            entries.push(HistoryEntry {
                metadata: self.run_metadata(&id)?,
                definition_identity,
                configuration,
                method,
                id,
                created_at: created_at.to_string(),
                interrupted,
                incomplete,
                stride,
                snapshot: serde_json::from_str(&snapshot).map_err(failure_from)?,
                points: Vec::new(),
            });
        }
        Ok(entries)
    }

    fn run_metadata(&self, id: &str) -> Result<velo_contracts::reports::RunMetadata, RunError> {
        let source: Option<String> = self
            .connection
            .query_row(
                "SELECT metadata FROM run_context WHERE run_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(failure_from)?;
        source
            .map(|source| serde_json::from_str(&source).map_err(failure_from))
            .unwrap_or_else(|| Ok(Default::default()))
    }

    fn run_context(&self, id: &str) -> Result<(Option<String>, serde_json::Value), RunError> {
        let source: Option<(Option<String>, String)> = self
            .connection
            .query_row(
                "SELECT identity,configuration FROM run_context WHERE run_id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(failure_from)?;
        match source {
            Some((identity, configuration)) => Ok((
                identity,
                serde_json::from_str(&configuration).map_err(failure_from)?,
            )),
            None => Ok((None, serde_json::Value::Null)),
        }
    }

    pub(crate) fn save_context(
        &self,
        id: &str,
        configuration: &serde_json::Value,
    ) -> Result<(), RunError> {
        use sha2::{Digest, Sha256};
        let source = serde_json::to_string(configuration).map_err(failure_from)?;
        let identity = format!("safe-v1:{:x}", Sha256::digest(source.as_bytes()));
        self.connection.execute("INSERT INTO run_context(run_id,metadata,identity,configuration) VALUES (?1,?2,?3,?4) ON CONFLICT(run_id) DO UPDATE SET identity=excluded.identity,configuration=excluded.configuration", params![id, serde_json::to_string(&velo_contracts::reports::RunMetadata::default()).map_err(failure_from)?, identity, source]).map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn update_metadata(
        &self,
        id: &str,
        metadata: &velo_contracts::reports::RunMetadata,
    ) -> Result<(), RunError> {
        self.cursor(id)?;
        self.connection.execute("INSERT INTO run_context(run_id,metadata) VALUES (?1,?2) ON CONFLICT(run_id) DO UPDATE SET metadata=excluded.metadata", params![id, serde_json::to_string(metadata).map_err(failure_from)?]).map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn baseline(&self) -> Result<Option<String>, RunError> {
        self.connection
            .query_row("SELECT run_id FROM baseline WHERE slot=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(failure_from)
    }

    pub(crate) fn pin_baseline(&self, id: Option<&str>) -> Result<(), RunError> {
        if let Some(id) = id {
            let entry = self.detail(id)?;
            if !entry.interrupted
                && !matches!(
                    entry.snapshot["state"].as_str(),
                    Some("completed" | "cancelled" | "failed" | "aborted")
                )
            {
                return Err(RunError {
                    code: "BASELINE_ACTIVE".into(),
                    message: "Wait for the run to finish before pinning it.".into(),
                    field: None,
                });
            }
            self.connection.execute("INSERT INTO baseline VALUES (1,?1) ON CONFLICT(slot) DO UPDATE SET run_id=excluded.run_id", [id]).map_err(failure_from)?;
        } else {
            self.connection
                .execute("DELETE FROM baseline", [])
                .map_err(failure_from)?;
        }
        Ok(())
    }

    pub(crate) fn comparison_settings(
        &self,
        id: &str,
    ) -> Result<Option<velo_contracts::reports::ComparisonSettings>, RunError> {
        let source: Option<String> = self
            .connection
            .query_row(
                "SELECT settings FROM comparisons WHERE candidate_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(failure_from)?;
        source
            .map(|source| serde_json::from_str(&source).map_err(failure_from))
            .transpose()
    }

    pub(crate) fn save_comparison(
        &self,
        id: &str,
        settings: &velo_contracts::reports::ComparisonSettings,
        report: &velo_contracts::reports::ComparisonReport,
    ) -> Result<(), RunError> {
        self.cursor(id)?;
        self.connection.execute("INSERT INTO comparisons VALUES (?1,?2,?3,?4) ON CONFLICT(candidate_id) DO UPDATE SET settings=excluded.settings,verdict=excluded.verdict,regressions=excluded.regressions", params![id, serde_json::to_string(settings).map_err(failure_from)?, serde_json::to_string(&report.verdict).map_err(failure_from)?, serde_json::to_string(&report.regressions).map_err(failure_from)?]).map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn comparison_report(
        &self,
        id: &str,
        settings: &velo_contracts::reports::ComparisonSettings,
    ) -> Result<velo_contracts::reports::ComparisonReport, RunError> {
        let candidate = self.detail(id)?;
        let baseline_exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE id=?1)",
                [&settings.baseline_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(failure_from)?;
        let baseline = if baseline_exists {
            Some(self.detail(&settings.baseline_id)?)
        } else {
            None
        };
        Ok(velo_contracts::reports::compare(
            baseline, candidate, settings,
        ))
    }

    pub(crate) fn cursor(&self, id: &str) -> Result<i64, RunError> {
        self.connection
            .query_row("SELECT rowid FROM runs WHERE id=?1", [id], |row| row.get(0))
            .map_err(failure_from)
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
            .map_err(failure_from)?;
        let rows = statement
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(failure_from)?;
        for row in rows {
            entry
                .points
                .push(serde_json::from_str(&row.map_err(failure_from)?).map_err(failure_from)?);
        }
        Ok(entry)
    }

    pub(crate) fn delete_run(&self, id: &str) -> Result<(), RunError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failure_from)?;
        let deleted = transaction
            .execute(
                "DELETE FROM runs WHERE id=?1 AND (terminal=1 OR interrupted=1)",
                [id],
            )
            .map_err(failure_from)?;
        if deleted > 0 {
            transaction.execute("UPDATE comparisons SET verdict='\"unevaluable\"',regressions='[]' WHERE json_extract(settings,'$.baseline_id')=?1", [id]).map_err(failure_from)?;
        }
        transaction.commit().map_err(failure_from)
    }

    #[cfg(test)]
    pub(crate) fn save(
        &self,
        id: &str,
        name: &str,
        remembered: bool,
        config: &RunConfig,
    ) -> Result<(), RunError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failure_from)?;
        transaction
            .execute(
                "INSERT INTO configurations VALUES (?1,?2,?3,?4)",
                params![
                    id,
                    name,
                    remembered,
                    serde_json::to_string(config).map_err(failure_from)?
                ],
            )
            .map_err(failure_from)?;
        transaction
            .execute("DELETE FROM credential_cleanup WHERE id=?1", [id])
            .map_err(failure_from)?;
        transaction.commit().map_err(failure_from)
    }

    #[cfg(test)]
    pub(crate) fn queue_cleanup(&self, id: &str) -> Result<(), RunError> {
        self.connection
            .execute("INSERT OR IGNORE INTO credential_cleanup VALUES (?1)", [id])
            .map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn complete_cleanup(&self, id: &str) -> Result<(), RunError> {
        self.connection
            .execute("DELETE FROM credential_cleanup WHERE id=?1", [id])
            .map_err(failure_from)?;
        Ok(())
    }

    pub(crate) fn pending_cleanup(&self) -> Result<Vec<String>, RunError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM credential_cleanup ORDER BY rowid LIMIT 25")
            .map_err(failure_from)?;
        statement
            .query_map([], |row| row.get(0))
            .map_err(failure_from)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(failure_from)
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
                .map_err(failure_from)?
        } else {
            i64::MAX
        };
        let mut statement = self.connection.prepare("SELECT id,name,remembered,config FROM configurations WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 25").map_err(failure_from)?;
        let rows = statement
            .query_map([cursor], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(failure_from)?;
        let mut result = Vec::new();
        for row in rows {
            let (id, name, remembered, config) = row.map_err(failure_from)?;
            result.push(SavedConfiguration {
                id,
                name,
                remembered,
                config: serde_json::from_str(&config).map_err(failure_from)?,
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
            .map_err(failure_from)?
            .ok_or_else(failure)?;
        Ok(SavedConfiguration {
            id: id.into(),
            name: row.0,
            remembered: row.1,
            config: serde_json::from_str(&row.2).map_err(failure_from)?,
        })
    }

    pub(crate) fn delete_configuration(&self, id: &str) -> Result<(), RunError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failure_from)?;
        transaction.execute("INSERT OR IGNORE INTO credential_cleanup SELECT id FROM configurations WHERE id=?1 AND remembered=1", [id]).map_err(failure_from)?;
        transaction
            .execute("DELETE FROM configurations WHERE id=?1", [id])
            .map_err(failure_from)?;
        transaction.commit().map_err(failure_from)
    }
}

#[cfg(test)]
pub(crate) fn redacted(config: &RunConfig, origin: &str) -> RunConfig {
    RunConfig {
        scenarios: None,
        data: None,
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
    fn infrastructure_diagnostics_keep_categories_without_source_text() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        struct Writer(Arc<Mutex<Vec<u8>>>);

        impl Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let captured = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || Writer(captured.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            let error = failure_from(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "dummy-storage-secret",
            ));
            assert_eq!(error.code, "STORAGE_UNAVAILABLE");
            failure_from(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(8),
                Some("dummy-sql-secret".into()),
            ));
        });
        let rendered = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        assert!(rendered.contains("PermissionDenied"));
        assert!(rendered.contains("sqlite_code=8"));
        assert!(!rendered.contains("dummy-storage-secret"));
        assert!(!rendered.contains("dummy-sql-secret"));
    }

    #[test]
    fn automatic_stop_evidence_survives_reopen_as_a_terminal_partial_run() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite");
        let mut store = Store::open(&path).unwrap();
        let mut update = sample();
        update.snapshot.state = velo_engine::RunState::Aborted;
        update.snapshot.measurement.complete = false;
        update.snapshot.measurement.warmup_ms = 200;
        update.snapshot.automatic_stop = Some(velo_engine::AbortEvidence {
            policy: velo_engine::AbortPolicy {
                rule: velo_engine::Threshold {
                    metric: velo_engine::ThresholdMetric::HttpFailurePercent,
                    operator: velo_engine::ThresholdOperator::LessThan,
                    limit: 50.0,
                },
                window_ms: 100,
                min_samples: 1,
                sustained_ms: 200,
            },
            observed: Some(100.0),
            samples: 5,
            at_us: 500_000,
            breached_us: 200_000,
        });
        store.begin("aborted", "GET", &update.snapshot).unwrap();
        store.record("aborted", &update, false).unwrap();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let entry = reopened.detail("aborted").unwrap();
        assert!(!entry.interrupted);
        assert_eq!(entry.snapshot["state"], "aborted");
        assert_eq!(entry.snapshot["measurement"]["complete"], false);
        assert_eq!(entry.snapshot["measurement"]["warmup_ms"], 200);
        assert_eq!(entry.snapshot["automatic_stop"]["at_us"], "500000");
        assert_eq!(entry.snapshot["automatic_stop"]["samples"], "5");
        assert_eq!(
            entry.points[0]["snapshot"]["automatic_stop"]["observed"],
            100.0
        );
    }

    #[test]
    fn diagnostics_persist_once_per_run_and_are_deleted_with_the_run() {
        let mut store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let mut update = sample();
        let diagnostics = &mut update.snapshot.metrics.diagnostics;
        let example = velo_engine::ResponseSample {
            assertion: None,
            status: Some(500),
            cause: "http_status".into(),
        };
        diagnostics.samples.push(example.clone());
        diagnostics
            .groups
            .push(velo_engine::FailureGroup { example, count: 17 });
        diagnostics.retained_bytes = serde_json::to_vec(&diagnostics.samples).unwrap().len() as u32;
        update.snapshot.evaluated.diagnostics = update.snapshot.metrics.diagnostics.clone();
        store.begin("diagnostics", "GET", &update.snapshot).unwrap();
        store.record("diagnostics", &update, false).unwrap();
        let entry = store.detail("diagnostics").unwrap();
        assert_eq!(
            entry.snapshot["metrics"]["diagnostics"]["groups"][0]["count"],
            "17"
        );
        assert_eq!(
            entry.snapshot["metrics"]["diagnostics"]["samples"][0]["status"],
            500
        );
        assert_eq!(
            entry.points[0]["snapshot"]["metrics"]["diagnostics"]["samples"],
            serde_json::json!([])
        );
        assert_eq!(
            entry.points[0]["snapshot"]["metrics"]["diagnostics"]["groups"],
            serde_json::json!([])
        );
        assert_eq!(
            entry.snapshot["evaluated"]["diagnostics"]["groups"][0]["count"],
            "17"
        );
        assert_eq!(
            entry.points[0]["snapshot"]["evaluated"]["diagnostics"]["samples"],
            serde_json::json!([])
        );
        assert_eq!(
            entry.points[0]["snapshot"]["evaluated"]["diagnostics"]["groups"],
            serde_json::json!([])
        );
        store.delete_run("diagnostics").unwrap();
        assert!(store.detail("diagnostics").is_err());
        let points: u32 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM points WHERE run_id='diagnostics'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(points, 0);
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
    fn telemetry_availability_and_native_windows_survive_history_compaction() {
        let mut store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let mut update = sample();
        update.snapshot.telemetry = velo_engine::HostTelemetry {
            status: velo_engine::TelemetryStatus::Available,
            sampled_at_us: Some(1_000_000),
            interval_us: Some(1_000_000),
            logical_cpus: Some(8),
            process_cpu_percent: Some(25.0),
            process_memory_bytes: Some(104_857_600),
            system_cpu_percent: None,
            system_available_memory_bytes: None,
            system_total_memory_bytes: None,
        };
        store.begin("telemetry", "GET", &update.snapshot).unwrap();
        for sequence in 1..=601 {
            update.sequence = sequence.to_string();
            store.record("telemetry", &update, false).unwrap();
        }
        let entry = store.detail("telemetry").unwrap();
        assert!(entry.points.len() <= 600);
        assert_eq!(entry.stride, 2);
        assert_eq!(
            entry.snapshot["telemetry"]["process_memory_bytes"],
            "104857600"
        );
        assert_eq!(
            entry.points[0]["snapshot"]["telemetry"]["sampled_at_us"],
            "1000000"
        );
        assert_eq!(
            entry.points[0]["snapshot"]["telemetry"]["interval_us"],
            "1000000"
        );
        assert!(entry.points[0]["snapshot"]["telemetry"]["system_cpu_percent"].is_null());
        update.sequence = "602".into();
        update.snapshot.telemetry = velo_engine::HostTelemetry {
            status: velo_engine::TelemetryStatus::Unavailable,
            ..Default::default()
        };
        store.record("telemetry", &update, false).unwrap();
        let entry = store.detail("telemetry").unwrap();
        assert_eq!(entry.snapshot["telemetry"]["status"], "unavailable");
        assert!(entry.snapshot["telemetry"]["process_cpu_percent"].is_null());
        assert_eq!(
            entry.points.last().unwrap()["snapshot"]["telemetry"]["status"],
            "unavailable"
        );
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
    #[test]
    fn store_lifetime_releases_ownership_when_a_descriptor_is_duplicated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let store = Store::open(&path).unwrap();
        let duplicate = store._ownership.as_ref().unwrap().0.try_clone().unwrap();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        drop(duplicate);
        drop(reopened);
    }
    #[test]
    fn baseline_metadata_and_budgets_survive_restart_without_mutating_measurements() {
        use velo_contracts::reports::{
            ComparisonSettings, RegressionBudget, RegressionMetric, RegressionVerdict, RunMetadata,
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reports.sqlite3");
        let rules = ComparisonSettings {
            baseline_id: "baseline".into(),
            configuration_reviewed: true,
            budgets: vec![RegressionBudget {
                metric: RegressionMetric::RequestRate,
                limit: 0.0,
                percentage: false,
            }],
        };
        let measured;
        {
            let mut store = Store::open(&path).unwrap();
            let mut update = sample();
            update.snapshot.state = velo_engine::RunState::Completed;
            update.snapshot.measurement.complete = true;
            update.snapshot.measurement.evaluated_us = 1000;
            update.snapshot.evaluated.counter_overflow = false;
            update.snapshot.evaluated.aggregation_interference = false;
            update.snapshot.metrics.aggregation_interference = false;
            update.snapshot.evaluated.http_failure_rate = Some(0.0);
            update.snapshot.evaluated.request_rate = 100.0;
            for id in ["baseline", "candidate"] {
                store.begin(id, "GET", &update.snapshot).unwrap();
                store.record(id, &update, false).unwrap();
                store
                    .save_context(id, &serde_json::json!({"timeout_ms":30000}))
                    .unwrap();
            }
            store.pin_baseline(Some("baseline")).unwrap();
            measured = store.detail("candidate").unwrap().snapshot;
            let metadata = RunMetadata {
                git_sha: "abc1234".into(),
                release: "v1.2".into(),
                tags: vec!["release".into()],
                notes: "First release".into(),
            };
            store.update_metadata("candidate", &metadata).unwrap();
            assert_eq!(store.detail("candidate").unwrap().snapshot, measured);
            let report = store.comparison_report("candidate", &rules).unwrap();
            assert_eq!(report.verdict, RegressionVerdict::Pass);
            store.save_comparison("candidate", &rules, &report).unwrap();
            let verdict: String = store
                .connection
                .query_row(
                    "SELECT verdict FROM comparisons WHERE candidate_id='candidate'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(verdict, "\"pass\"");
            store.pin_baseline(Some("candidate")).unwrap();
            assert_eq!(store.baseline().unwrap().as_deref(), Some("candidate"));
            store.pin_baseline(None).unwrap();
            assert!(store.baseline().unwrap().is_none());
            assert!(store.detail("baseline").is_ok());
            store.pin_baseline(Some("baseline")).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.baseline().unwrap().as_deref(), Some("baseline"));
        let entry = store.detail("candidate").unwrap();
        assert_eq!(entry.metadata.release, "v1.2");
        assert_eq!(entry.metadata.notes, "First release");
        assert_eq!(entry.snapshot, measured);
        assert!(entry.definition_identity.unwrap().starts_with("safe-v1:"));
        let restored = store.comparison_settings("candidate").unwrap().unwrap();
        assert_eq!(restored.baseline_id, "baseline");
        assert_eq!(restored.budgets[0].limit, 0.0);
        assert_eq!(
            store
                .comparison_report("candidate", &restored)
                .unwrap()
                .verdict,
            RegressionVerdict::Pass
        );
        store.delete_run("baseline").unwrap();
        assert_eq!(store.baseline().unwrap().as_deref(), Some("baseline"));
        let report = store.comparison_report("candidate", &restored).unwrap();
        assert!(report.baseline.is_none());
        assert_eq!(report.verdict, RegressionVerdict::Unevaluable);
        let verdict: String = store
            .connection
            .query_row(
                "SELECT verdict FROM comparisons WHERE candidate_id='candidate'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(verdict, "\"unevaluable\"");
        store.delete_run("candidate").unwrap();
        assert!(store.comparison_settings("candidate").unwrap().is_none());
    }

    #[test]
    fn missing_or_active_baseline_and_legacy_identity_are_explicit() {
        let mut store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        assert!(store.pin_baseline(Some("missing")).is_err());
        let mut update = sample();
        update.snapshot.state = velo_engine::RunState::Running;
        store.begin("active", "GET", &update.snapshot).unwrap();
        assert_eq!(
            store.pin_baseline(Some("active")).unwrap_err().code,
            "BASELINE_ACTIVE"
        );
        store.delete_run("active").unwrap();
        assert!(store.detail("active").is_ok());
        update.snapshot.state = velo_engine::RunState::Cancelled;
        store.record("active", &update, false).unwrap();
        store.pin_baseline(Some("active")).unwrap();
        let entry = store.detail("active").unwrap();
        assert!(entry.definition_identity.is_none());
        assert!(entry.metadata.notes.is_empty());
        assert!(entry.configuration.is_null());
        store
            .update_metadata("active", &Default::default())
            .unwrap();
        assert!(
            store
                .detail("active")
                .unwrap()
                .definition_identity
                .is_none()
        );
        assert!(
            store
                .update_metadata("missing", &Default::default())
                .is_err()
        );
    }
}
