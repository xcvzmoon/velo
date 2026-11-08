//! Bounded functional history, with response content persistence disabled by default.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use velo_engine::{FunctionalRun, RunError};

use crate::{
    history::History,
    storage::{Store, failure_from},
};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ApiHistorySettings {
    limit: u32,
    persist_content: bool,
}

impl Store {
    fn api_schema(&self) -> Result<(), RunError> {
        self.connection.execute_batch("CREATE TABLE IF NOT EXISTS api_runs (id TEXT PRIMARY KEY, sequence INTEGER NOT NULL, evidence TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS api_history_settings (slot INTEGER PRIMARY KEY CHECK(slot=1), retention INTEGER NOT NULL, persist_content INTEGER NOT NULL);
            INSERT OR IGNORE INTO api_history_settings VALUES (1,25,0);").map_err(failure_from)
    }

    fn api_settings(&self) -> Result<ApiHistorySettings, RunError> {
        self.api_schema()?;
        self.connection
            .query_row(
                "SELECT retention, persist_content FROM api_history_settings WHERE slot=1",
                [],
                |row| {
                    Ok(ApiHistorySettings {
                        limit: row.get(0)?,
                        persist_content: row.get(1)?,
                    })
                },
            )
            .map_err(failure_from)
    }

    fn update_api_settings(&self, limit: u32, persist_content: bool) -> Result<(), RunError> {
        if limit > 100 {
            return Err(crate::api::failure(
                "INVALID_API_RETENTION",
                "Keep 0 to 100 functional runs.",
            ));
        }
        self.api_schema()?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failure_from)?;
        transaction
            .execute(
                "UPDATE api_history_settings SET retention=?1, persist_content=?2 WHERE slot=1",
                params![limit, persist_content],
            )
            .map_err(failure_from)?;
        if !persist_content {
            for mut run in self.api_runs()? {
                for result in &mut run.results {
                    if let Some(response) = &mut result.response {
                        response.content_retained = false;
                        response.body.clear();
                        response.headers.clear();
                        response.redacted = true;
                    }
                }
                transaction
                    .execute(
                        "UPDATE api_runs SET evidence=?1 WHERE id=?2",
                        params![serde_json::to_string(&run).map_err(failure_from)?, run.id],
                    )
                    .map_err(failure_from)?;
            }
        }
        self.trim_api_runs(limit)?;
        transaction.commit().map_err(failure_from)
    }

    pub(crate) fn save_api_run(&self, report: &FunctionalRun) -> Result<(), RunError> {
        let settings = self.api_settings()?;
        if settings.limit == 0 {
            return Ok(());
        }
        let mut report = report.clone();
        report.session = Default::default();
        if !settings.persist_content {
            for result in &mut report.results {
                if let Some(response) = &mut result.response {
                    response.content_retained = false;
                    response.body.clear();
                    response.headers.clear();
                    response.redacted = true;
                }
            }
        }
        let evidence = serde_json::to_string(&report).map_err(failure_from)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failure_from)?;
        transaction.execute("INSERT INTO api_runs(id, sequence, evidence) VALUES (?1, (SELECT COALESCE(MAX(sequence),0)+1 FROM api_runs), ?2) ON CONFLICT(id) DO UPDATE SET evidence=excluded.evidence", params![report.id,evidence]).map_err(failure_from)?;
        self.trim_api_runs(settings.limit)?;
        transaction.commit().map_err(failure_from)
    }

    fn trim_api_runs(&self, limit: u32) -> Result<(), RunError> {
        self.connection.execute("DELETE FROM api_runs WHERE id NOT IN (SELECT id FROM api_runs ORDER BY sequence DESC LIMIT ?1)", [limit]).map_err(failure_from)?;
        Ok(())
    }

    fn api_runs(&self) -> Result<Vec<FunctionalRun>, RunError> {
        self.api_schema()?;
        let mut statement = self
            .connection
            .prepare("SELECT evidence FROM api_runs ORDER BY sequence DESC LIMIT 100")
            .map_err(failure_from)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(failure_from)?;
        let mut reports = Vec::new();
        for row in rows {
            reports.push(serde_json::from_str(&row.map_err(failure_from)?).map_err(failure_from)?);
        }
        Ok(reports)
    }
}

#[tauri::command]
pub(crate) async fn api_history(
    state: tauri::State<'_, History>,
) -> Result<Vec<FunctionalRun>, RunError> {
    state.call(|store| store.api_runs()).await
}

#[tauri::command]
pub(crate) async fn api_history_settings(
    state: tauri::State<'_, History>,
) -> Result<ApiHistorySettings, RunError> {
    state.call(|store| store.api_settings()).await
}

#[tauri::command]
pub(crate) async fn set_api_history_settings(
    state: tauri::State<'_, History>,
    limit: u32,
    persist_content: bool,
) -> Result<(), RunError> {
    state
        .call(move |store| store.update_api_settings(limit, persist_content))
        .await
}

#[tauri::command]
pub(crate) async fn delete_api_history(
    state: tauri::State<'_, History>,
    id: Option<String>,
) -> Result<(), RunError> {
    state
        .call(move |store| {
            store.api_schema()?;
            match id {
                Some(id) => {
                    store
                        .connection
                        .execute("DELETE FROM api_runs WHERE id=?1", [id])
                        .map_err(failure_from)?;
                }
                None => {
                    store
                        .connection
                        .execute("DELETE FROM api_runs", [])
                        .map_err(failure_from)?;
                }
            }
            Ok(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::Store;
    use velo_engine::{
        ApiSessionView, FunctionalResult, FunctionalRun, FunctionalState, NameValue,
        PreviewResponse,
    };

    fn report(id: &str) -> FunctionalRun {
        FunctionalRun {
            kind: velo_engine::FunctionalKind::Functional,
            id: id.into(),
            collection_id: "collection".into(),
            environment: Some("staging".into()),
            state: velo_engine::FunctionalRunState::Completed,
            total: 1,
            results: vec![FunctionalResult {
                request_id: "request".into(),
                name: "Saved request".into(),
                iteration: 1,
                state: FunctionalState::Passed,
                response: Some(PreviewResponse {
                    status: 200,
                    elapsed_ms: 10.0,
                    body: "retained-content".into(),
                    content_retained: true,
                    headers: vec![NameValue {
                        name: "content-type".into(),
                        value: "application/json".into(),
                    }],
                    ..Default::default()
                }),
                error: None,
            }],
            requests: Vec::new(),
            session: ApiSessionView {
                variables: vec!["transient-session-name".into()],
                cookies: vec![NameValue {
                    name: "transient-cookie-name".into(),
                    value: "private-session-value".into(),
                }],
            },
            storage_error: None,
        }
    }

    #[test]
    fn default_history_keeps_outcomes_and_omits_content_and_session_state() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("history.sqlite3")).unwrap();
        let settings = store.api_settings().unwrap();
        assert_eq!(settings.limit, 25);
        assert!(!settings.persist_content);
        store.save_api_run(&report("first")).unwrap();
        let history = store.api_runs().unwrap();
        assert_eq!(history.len(), 1);
        assert!(matches!(
            history[0].kind,
            velo_engine::FunctionalKind::Functional
        ));
        assert_eq!(history[0].results[0].state, FunctionalState::Passed);
        let response = history[0].results[0].response.as_ref().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.elapsed_ms, 10.0);
        assert!(response.body.is_empty());
        assert!(response.headers.is_empty());
        assert!(response.redacted);
        assert!(!response.content_retained);
        let encoded = serde_json::to_string(&history).unwrap();
        for omitted in [
            "retained-content",
            "transient-session-name",
            "transient-cookie-name",
            "private-session-value",
        ] {
            assert!(!encoded.contains(omitted));
        }
    }

    #[test]
    fn disabling_content_scrubs_existing_history_and_retention_zero_erases_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("history.sqlite3")).unwrap();
        store.update_api_settings(3, true).unwrap();
        store.save_api_run(&report("first")).unwrap();
        assert_eq!(
            store.api_runs().unwrap()[0].results[0]
                .response
                .as_ref()
                .unwrap()
                .body,
            "retained-content"
        );
        let evidence: String = store
            .connection
            .query_row(
                "SELECT evidence FROM api_runs WHERE id='first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!evidence.contains("private-session-value"));
        store.update_api_settings(3, false).unwrap();
        let evidence: String = store
            .connection
            .query_row(
                "SELECT evidence FROM api_runs WHERE id='first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!evidence.contains("retained-content"));
        store.update_api_settings(0, true).unwrap();
        assert!(store.api_runs().unwrap().is_empty());
        store.save_api_run(&report("second")).unwrap();
        assert!(store.api_runs().unwrap().is_empty());
    }

    #[test]
    fn retention_keeps_newest_runs_and_settings_survive_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        {
            let store = Store::open(&path).unwrap();
            store.update_api_settings(2, false).unwrap();
            for id in ["first", "second", "third"] {
                store.save_api_run(&report(id)).unwrap();
            }
            assert_eq!(
                store
                    .api_runs()
                    .unwrap()
                    .iter()
                    .map(|run| run.id.as_str())
                    .collect::<Vec<_>>(),
                ["third", "second"]
            );
            assert!(store.update_api_settings(101, true).is_err());
        }
        let store = Store::open(&path).unwrap();
        let settings = store.api_settings().unwrap();
        assert_eq!(settings.limit, 2);
        assert!(!settings.persist_content);
        store.update_api_settings(1, false).unwrap();
        assert_eq!(store.api_runs().unwrap()[0].id, "third");
        assert_eq!(store.api_runs().unwrap().len(), 1);
    }
}
