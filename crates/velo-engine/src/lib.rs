//! Desktop-independent HTTP load engine with bounded virtual-user execution.

mod config;
mod http_options;
mod metrics;
mod runner;
mod threshold;
mod workload;

pub use config::{
    NameValue, RequestBody, RequestMethod, ResourceBudget, RunConfig, RunConfirmation, RunError,
};
pub use http_options::RequestAuth;
pub use metrics::{Counters, LatencySnapshot, MetricsSnapshot};
pub use runner::{ArrivalSnapshot, RunManager, RunSnapshot, RunState, RunUpdate};
pub use threshold::{
    Threshold, ThresholdMetric, ThresholdOperator, ThresholdResult, ThresholdStatus,
};
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
