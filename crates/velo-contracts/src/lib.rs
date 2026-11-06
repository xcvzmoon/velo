//! Desktop command contracts shared by Tauri and the TypeScript exporter.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Input to the native diagnostic command.
///
/// The command validates the trimmed label before running the diagnostic.
///
/// # Examples
/// ```
/// let input = velo_contracts::DiagnosticInput { label: "desktop".into() };
/// assert_eq!(input.label, "desktop");
/// ```
#[derive(Deserialize, Serialize, TS)]
pub struct DiagnosticInput {
    /// Label containing 1 to 64 characters after trimming whitespace.
    pub label: String,
}

/// Build identity returned by a successful native diagnostic.
///
/// # Examples
/// ```
/// let report = velo_contracts::DiagnosticReport {
///     engine: "velo-engine".into(),
///     version: "0.0.0".into(),
/// };
/// assert_eq!(report.engine, "velo-engine");
/// ```
#[derive(Debug, Serialize, TS)]
pub struct DiagnosticReport {
    /// Name of the engine handling the diagnostic.
    pub engine: String,
    /// Engine package version.
    pub version: String,
}

/// Structured failure returned across the desktop command boundary.
///
/// # Examples
/// ```
/// let error = velo_contracts::CommandError {
///     code: "INVALID_DIAGNOSTIC_LABEL".into(),
///     message: "Enter a label between 1 and 64 characters.".into(),
///     field: Some("label".into()),
/// };
/// assert_eq!(error.to_string(), error.message);
/// ```
#[derive(Debug, Serialize, TS, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    /// Stable code identifying the failure, such as `INVALID_DIAGNOSTIC_LABEL`.
    pub code: String,
    /// Message safe to display to the user.
    pub message: String,
    /// Input field associated with the failure, or `None` for command failures.
    pub field: Option<String>,
}

/// A bounded history entry, with a safe summary and aggregate interval samples.
///
/// # Examples
/// ```
/// # fn example(entry: velo_contracts::HistoryEntry) {
/// assert!(entry.stride >= 1);
/// # }
/// ```
#[derive(Debug, Serialize, TS)]
pub struct HistoryEntry {
    /// HTTP method used for comparison eligibility.
    pub method: String,
    /// Durable identifier independent of engine process identifiers.
    pub id: String,
    /// UTC creation time as Unix seconds.
    pub created_at: String,
    /// True when the application closed before a terminal snapshot was recorded.
    pub interrupted: bool,
    /// True when some aggregate intervals could not be stored.
    pub incomplete: bool,
    /// Sampling stride; one retains every interval, two every other interval.
    pub stride: u32,
    /// Latest safe engine summary.
    #[ts(type = "RunSnapshot")]
    pub snapshot: serde_json::Value,
    /// At most 600 aggregate intervals; empty in paginated list responses.
    #[ts(type = "RunUpdate[]")]
    pub points: Vec<serde_json::Value>,
}

/// Saved request metadata with omitted fields or an explicit OS-store reference.
///
/// # Examples
/// ```
/// # fn example(config: velo_contracts::SavedConfiguration) {
/// assert!(!config.id.is_empty());
/// # }
/// ```
#[derive(Debug, Serialize, TS)]
pub struct SavedConfiguration {
    /// Durable configuration and credential reference identifier.
    pub id: String,
    /// User-provided display name, never inferred from request content.
    pub name: String,
    /// Whether loading requires the OS credential store.
    pub remembered: bool,
    /// Redacted configuration on save/list; restored only during explicit load.
    #[ts(type = "RunConfig")]
    pub config: serde_json::Value,
}
