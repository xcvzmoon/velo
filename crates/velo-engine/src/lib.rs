//! Desktop-independent HTTP load engine with bounded constant-VU execution.

mod config;
mod metrics;
mod runner;

pub use config::{
    NameValue, RequestBody, RequestMethod, ResourceBudget, RunConfig, RunConfirmation, RunError,
};
pub use metrics::{Counters, LatencySnapshot, MetricsSnapshot};
pub use runner::{RunManager, RunSnapshot, RunState, RunUpdate};

/// Returns the engine's build identity without executing network requests.
///
/// # Returns
/// The engine name and Cargo package version. No resources are acquired.
///
/// # Examples
/// ```
/// let (name, version) = velo_engine::identity();
/// assert_eq!(name, "velo-engine");
/// assert!(!version.is_empty());
/// ```
pub fn identity() -> (&'static str, &'static str) {
    ("velo-engine", env!("CARGO_PKG_VERSION"))
}
