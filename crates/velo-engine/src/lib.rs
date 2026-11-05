//! Desktop-independent HTTP load engine with bounded virtual-user execution.

mod config;
mod metrics;
mod runner;
mod workload;

pub use config::{
    NameValue, RequestBody, RequestMethod, ResourceBudget, RunConfig, RunConfirmation, RunError,
};
pub use metrics::{Counters, LatencySnapshot, MetricsSnapshot};
pub use runner::{RunManager, RunSnapshot, RunState, RunUpdate};
pub use workload::{RampStage, Workload};

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
