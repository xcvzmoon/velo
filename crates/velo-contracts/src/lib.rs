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
