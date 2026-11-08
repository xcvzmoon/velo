//! Desktop-independent HTTP load engine with bounded virtual-user execution.

mod admission;
mod assertions;
mod authoring;
mod config;
mod dataset;
mod http_options;
mod imports;
mod metrics;
mod preview;
mod runner;
mod scenario;
mod scenario_metrics;
mod scenario_runtime;
mod threshold;
mod transport;
mod workload;

#[doc(inline)]
pub use admission::{AbortEvidence, AbortPolicy, MeasurementWindow};
#[doc(inline)]
pub use assertions::{
    Assertion, AssertionRule, AssertionTotals, CheckOutcome, FailureGroup, InspectionLimits,
    ResponseDiagnostics, ResponseSample,
};
#[doc(inline)]
pub use authoring::TestDefinition;
pub use config::{
    NameValue, RequestBody, RequestMethod, ResourceBudget, RunConfig, RunConfirmation, RunError,
};
#[doc(inline)]
pub use dataset::{
    DataAllocation, DataDefinition, DataExhaustion, DataField, DataGenerator, DataLifetime,
    DataRequest, DataSource, DataSummary, PreparedData,
};
pub use http_options::{HttpProtocol, MultipartPart, RequestAuth};
#[doc(inline)]
pub use imports::{ImportFormat, ImportedRequest, import_requests};
pub use metrics::{Counters, LatencySnapshot, MetricsSnapshot};
#[doc(inline)]
pub use preview::{PreviewResponse, preview_request};
pub use runner::{ArrivalSnapshot, RunManager, RunSnapshot, RunState, RunUpdate};
#[doc(inline)]
pub use scenario::{
    Extraction, ExtractionSource, Scenario, ScenarioPlan, ScenarioRequest, ScenarioStep,
    StepFailure,
};
#[doc(inline)]
pub use scenario_metrics::{
    PhaseOutcome, ScenarioPhase, ScenarioReport, ScenarioTotals, StepTotals,
};
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
