//! Shared reporting and atomic artifact publication, independent of desktop storage.

use std::{io::Write, path::Path};

use serde_json::Value;
use sha2::{Digest, Sha256};
use velo_contracts::{
    HistoryEntry,
    reports::{self, ComparisonReport, ComparisonSettings},
};
use velo_engine::{RunSnapshot, RunState};

use crate::{
    EXECUTION, Failure, INVALID, failure,
    input::{Arguments, read_file},
};

pub(crate) fn comparison(
    arguments: &Arguments,
) -> Result<(Option<HistoryEntry>, ComparisonSettings), Failure> {
    let settings = match &arguments.comparison {
        Some(path) => serde_json::from_str(&read_file(path)?)
            .map_err(|_| failure(INVALID, "INVALID_COMPARISON_SETTINGS"))?,
        None => ComparisonSettings {
            baseline_id: "none".into(),
            budgets: Vec::new(),
            configuration_reviewed: false,
        },
    };
    reports::validate_settings(&settings)
        .map_err(|_| failure(INVALID, "INVALID_COMPARISON_SETTINGS"))?;
    let baseline = match &arguments.baseline {
        Some(path) => {
            let report: ComparisonReport = serde_json::from_str(&read_file(path)?)
                .map_err(|_| failure(INVALID, "INVALID_BASELINE_REPORT"))?;
            if report.format_version != 1 || report.candidate.id != settings.baseline_id {
                return Err(failure(INVALID, "INVALID_BASELINE_REPORT"));
            }
            Some(report.candidate)
        }
        None => None,
    };
    Ok((baseline, settings))
}

pub(crate) fn entry(
    method: String,
    configuration: Value,
    snapshot: &RunSnapshot,
) -> Result<HistoryEntry, Failure> {
    let source = serde_json::to_vec(&configuration)
        .map_err(|_| failure(EXECUTION, "REPORT_ENCODING_FAILED"))?;
    Ok(HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| failure(EXECUTION, "CLOCK_UNAVAILABLE"))?
            .as_secs()
            .to_string(),
        method,
        interrupted: snapshot.state == RunState::Cancelled,
        incomplete: false,
        stride: 1,
        metadata: reports::RunMetadata::default(),
        definition_identity: Some(format!("safe-v1:{:x}", Sha256::digest(source))),
        configuration,
        snapshot: serde_json::to_value(snapshot)
            .map_err(|_| failure(EXECUTION, "REPORT_ENCODING_FAILED"))?,
        points: Vec::new(),
    })
}

fn scrub(value: &mut Value, sensitive: &[String], text_field: bool) {
    match value {
        Value::String(text) if text_field => {
            for secret in sensitive {
                if !secret.is_empty() && text.contains(secret) {
                    *text = text.replace(secret, "[redacted]");
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, sensitive, text_field);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                let text_field = matches!(
                    key.as_str(),
                    "origin"
                        | "id"
                        | "scenario_id"
                        | "step_id"
                        | "baseline_id"
                        | "git_sha"
                        | "release"
                        | "tags"
                        | "notes"
                        | "definition_identity"
                        | "assertion_ids"
                        | "baseline"
                        | "candidate"
                );
                scrub(value, sensitive, text_field);
            }
        }
        _ => {}
    }
}

pub(crate) fn redact(
    report: ComparisonReport,
    sensitive: &[String],
) -> Result<ComparisonReport, Failure> {
    let mut value =
        serde_json::to_value(report).map_err(|_| failure(EXECUTION, "REPORT_ENCODING_FAILED"))?;
    scrub(&mut value, sensitive, false);
    serde_json::from_value(value).map_err(|_| failure(EXECUTION, "REPORT_REDACTION_FAILED"))
}

fn write(path: &Path, content: &str) -> Result<(), Failure> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| failure(EXECUTION, "REPORT_WRITE_FAILED"))?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.as_file().sync_all())
        .map_err(|_| failure(EXECUTION, "REPORT_WRITE_FAILED"))?;
    file.persist(path)
        .map_err(|_| failure(EXECUTION, "REPORT_WRITE_FAILED"))?;
    Ok(())
}

pub(crate) fn export(arguments: &Arguments, report: &ComparisonReport) -> Result<(), Failure> {
    let mut failed = false;
    for (path, format) in [
        (&arguments.json, "json"),
        (&arguments.junit, "junit"),
        (&arguments.html, "html"),
    ] {
        if let Some(path) = path {
            let content = match format {
                "json" => reports::export_json(report),
                "junit" => reports::export_junit(report),
                _ => reports::export_html(report),
            }
            .map_err(|_| failure(EXECUTION, "REPORT_ENCODING_FAILED"))?;
            if write(path, &content).is_err() {
                failed = true;
            }
        }
    }
    if failed {
        Err(failure(EXECUTION, "REPORT_WRITE_FAILED"))
    } else {
        Ok(())
    }
}
