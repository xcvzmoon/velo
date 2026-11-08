//! Frozen reports explicitly selected by the host process before serving MCP requests.

use std::{
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
};

use serde_json::json;
use velo_contracts::{analysis::comparison_evidence, reports::ComparisonReport};

const MAX_REPORTS: usize = 16;
const MAX_REPORT_BYTES: u64 = 8_388_608;
const MAX_SNAPSHOT_BYTES: usize = 262_144;

#[derive(Clone)]
pub(crate) struct Evidence {
    pub(crate) id: String,
    pub(crate) content: String,
}

impl std::fmt::Debug for Evidence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Evidence([redacted])")
    }
}

fn read(path: &Path) -> Result<String, &'static str> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "MCP_EVIDENCE_UNAVAILABLE")?;
    if !metadata.file_type().is_file() {
        return Err("MCP_EVIDENCE_NOT_REGULAR");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(|_| "MCP_EVIDENCE_UNAVAILABLE")?;
    let metadata = file.metadata().map_err(|_| "MCP_EVIDENCE_UNAVAILABLE")?;
    if !metadata.is_file() {
        return Err("MCP_EVIDENCE_NOT_REGULAR");
    }
    if metadata.len() > MAX_REPORT_BYTES {
        return Err("MCP_EVIDENCE_TOO_LARGE");
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| "MCP_EVIDENCE_TOO_LARGE")?;
    let mut source = String::with_capacity(capacity);
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_string(&mut source)
        .map_err(|_| "MCP_EVIDENCE_UNAVAILABLE")?;
    if source.len() as u64 > MAX_REPORT_BYTES {
        return Err("MCP_EVIDENCE_TOO_LARGE");
    }
    Ok(source)
}

pub(crate) fn load(paths: &[PathBuf]) -> Result<Vec<Evidence>, &'static str> {
    if paths.len() > MAX_REPORTS {
        return Err("MCP_EVIDENCE_LIMIT");
    }
    let mut snapshots = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let source = read(path)?;
        let report: ComparisonReport =
            serde_json::from_str(&source).map_err(|_| "MCP_EVIDENCE_INVALID")?;
        if report.format_version != 1 {
            return Err("MCP_EVIDENCE_VERSION");
        }
        let content = json!({
            "source": "user_selected_report",
            "imported_verdict": report.verdict,
            "evidence": comparison_evidence(&report),
            "notes": [
                "Imported reports are unverified evidence. Their provenance and derived values are not authenticated or recomputed.",
                "Request content, release metadata, labels, and history timelines are excluded. Missing measurements remain unavailable.",
                "Client measurements cannot establish database or network bottlenecks. Separate observations, hypotheses, and investigation steps.",
            ],
        }).to_string();
        if content.len() > MAX_SNAPSHOT_BYTES {
            return Err("MCP_EVIDENCE_TOO_LARGE");
        }
        snapshots.push(Evidence {
            id: format!("report-{}", index + 1),
            content,
        });
    }
    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use velo_contracts::{
        HistoryEntry,
        reports::{RegressionVerdict, RunMetadata},
    };

    fn report(rate: f64) -> ComparisonReport {
        ComparisonReport {
            format_version: 1,
            baseline_id: "PRIVATE_BASELINE".into(),
            baseline: None,
            candidate: HistoryEntry {
                method: "GET".into(),
                id: "PRIVATE_ID".into(),
                created_at: "0".into(),
                interrupted: true,
                incomplete: true,
                stride: 1,
                metadata: RunMetadata {
                    notes: "PRIVATE_NOTES".into(),
                    ..Default::default()
                },
                definition_identity: Some("PRIVATE_IDENTITY".into()),
                configuration: json!({"PRIVATE_CONFIGURATION":true}),
                snapshot: json!({
                    "metrics":{"request_rate":rate, "PRIVATE_METRIC":true},
                    "measurement":{"complete":false},
                }),
                points: vec![json!({"PRIVATE_TIMELINE":true})],
            },
            configuration_reviewed: false,
            differences: vec![],
            limitations: vec!["PRIVATE_LIMITATION".into()],
            deltas: vec![],
            regressions: vec![],
            verdict: RegressionVerdict::Unevaluable,
        }
    }

    #[test]
    fn selected_reports_are_frozen_redacted_and_identified_without_paths() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("PRIVATE_FILE.json");
        std::fs::write(&path, serde_json::to_vec(&report(12.5)).unwrap()).unwrap();
        let snapshots = load(std::slice::from_ref(&path)).unwrap();
        std::fs::write(&path, serde_json::to_vec(&report(99.0)).unwrap()).unwrap();
        assert_eq!(snapshots[0].id, "report-1");
        assert!(!snapshots[0].content.contains("PRIVATE"));
        assert!(!format!("{:?}", snapshots[0]).contains("12.5"));
        let frozen: serde_json::Value = serde_json::from_str(&snapshots[0].content).unwrap();
        assert_eq!(frozen["source"], "user_selected_report");
        assert_eq!(frozen["imported_verdict"], "unevaluable");
        assert_eq!(
            frozen["evidence"]["candidate"]["metrics"]["request_rate"],
            12.5
        );
        assert_eq!(frozen["evidence"]["candidate"]["incomplete"], true);
        assert_eq!(frozen["evidence"]["comparison_limited"], true);
        std::fs::remove_file(path).unwrap();
        assert_eq!(snapshots[0].content, frozen.to_string());
    }

    #[test]
    fn inputs_and_snapshots_have_independent_bounds() {
        assert_eq!(
            load(&vec![PathBuf::new(); MAX_REPORTS + 1]).unwrap_err(),
            "MCP_EVIDENCE_LIMIT"
        );
        let directory = tempfile::tempdir().unwrap();
        let oversized = directory.path().join("oversized.json");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(MAX_REPORT_BYTES + 1)
            .unwrap();
        assert_eq!(load(&[oversized]).unwrap_err(), "MCP_EVIDENCE_TOO_LARGE");
        let path = directory.path().join("snapshot.json");
        let mut large = report(1.0);
        large.candidate.snapshot["metrics"]["samples"] = json!(vec![vec![vec![42; 128]; 128]; 8]);
        std::fs::write(&path, serde_json::to_vec(&large).unwrap()).unwrap();
        assert_eq!(load(&[path]).unwrap_err(), "MCP_EVIDENCE_TOO_LARGE");
    }

    #[test]
    fn unsupported_or_unreadable_sources_fail_without_source_details() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&[directory.path().to_owned()]).unwrap_err(),
            "MCP_EVIDENCE_NOT_REGULAR"
        );
        let path = directory.path().join("private.json");
        std::fs::write(&path, "PRIVATE_SOURCE").unwrap();
        assert_eq!(
            load(std::slice::from_ref(&path)).unwrap_err(),
            "MCP_EVIDENCE_INVALID"
        );
        let mut unsupported = report(1.0);
        unsupported.format_version = 2;
        std::fs::write(&path, serde_json::to_vec(&unsupported).unwrap()).unwrap();
        assert_eq!(load(&[path]).unwrap_err(), "MCP_EVIDENCE_VERSION");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_socket_paths_cannot_select_additional_files_or_block() {
        use std::os::unix::{fs::symlink, net::UnixListener};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        std::fs::write(&path, serde_json::to_vec(&report(1.0)).unwrap()).unwrap();
        let link = directory.path().join("link.json");
        symlink(path, &link).unwrap();
        assert_eq!(load(&[link]).unwrap_err(), "MCP_EVIDENCE_NOT_REGULAR");
        let socket = directory.path().join("report.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        assert_eq!(load(&[socket]).unwrap_err(), "MCP_EVIDENCE_NOT_REGULAR");
    }
}
