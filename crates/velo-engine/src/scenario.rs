//! Ordered HTTP journeys, bounded extraction, and isolated session state.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Weighted journeys and once-per-run lifecycle actions, all on the reviewed origin.
///
/// Selection mixes the seed with the global admission ordinal using SplitMix64.
/// Scheduling admits iterations; each selected journey owns one concurrency slot
/// until its sequential steps finish. VU sessions persist until worker retirement;
/// arrival iterations each receive a fresh isolated session. Relative weights are
/// probabilities, so small runs need not match their ratios.
///
/// # Examples
/// ```
/// let plan = velo_engine::ScenarioPlan::default();
/// assert!(plan.scenarios.is_empty());
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ScenarioPlan {
    /// Seed for deterministic selection in global admission order; default zero.
    #[serde(default)]
    pub seed: u32,
    /// Setup runs once before scheduling, with a 30-second total deadline.
    /// Its extracted values and cookies seed each isolated session. With datasets,
    /// lifecycle actions use the first prepared record.
    #[serde(default)]
    #[ts(type = "ScenarioStep[]")]
    pub setup: Vec<ScenarioStep>,
    /// Cleanup runs once after workers drain, using setup state only.
    /// Normal stop does not cancel cleanup; a separate 30-second total deadline applies.
    #[serde(default)]
    #[ts(type = "ScenarioStep[]")]
    pub cleanup: Vec<ScenarioStep>,
    /// One to 16 weighted journeys. Weights are relative probabilities, not quotas.
    #[ts(type = "Scenario[]")]
    pub scenarios: Vec<Scenario>,
}

/// A named journey selected once per admitted iteration.
///
/// # Examples
/// ```
/// # fn example(scenario: velo_engine::Scenario) {
/// assert!(scenario.weight > 0);
/// # }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Stable identifier, unique in the plan; 1 to 64 ASCII letters, digits, underscores or hyphens.
    pub id: String,
    /// Relative selection weight, 1 to 10,000.
    pub weight: u32,
    /// One to 32 steps executed sequentially.
    #[ts(type = "ScenarioStep[]")]
    pub steps: Vec<ScenarioStep>,
}

/// A reusable request with explicit dependency failure and think-time behavior.
///
/// # Examples
/// ```
/// # fn example(step: velo_engine::ScenarioStep) {
/// assert!(step.think_ms <= 3_600_000);
/// # }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ScenarioStep {
    /// Stable identity unique within this phase or scenario.
    pub id: String,
    /// HTTP content. Transport, workload, timeout and inspection limits come from the run.
    pub request: ScenarioRequest,
    /// Extraction is atomic; failed or missing optional values remove stale state.
    #[serde(default)]
    #[ts(type = "Extraction[]")]
    pub extractions: Vec<Extraction>,
    /// Cancellable delay after this step, excluded from HTTP latency; default zero.
    #[serde(default)]
    pub think_ms: u32,
    /// Stop the iteration or continue independent steps; no implicit retries.
    pub on_failure: StepFailure,
}

/// HTTP content shared by visual step editing and native execution.
///
/// # Examples
/// ```
/// # fn example(request: velo_engine::ScenarioRequest) {
/// let _method = request.method;
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ScenarioRequest {
    /// Absolute URL. Session placeholders are allowed only in the path or query.
    pub url: String,
    /// Logical HTTP method.
    pub method: crate::RequestMethod,
    /// Ordered HTTP headers.
    #[ts(type = "NameValue[]")]
    pub headers: Vec<crate::NameValue>,
    /// Ordered query parameters.
    #[ts(type = "NameValue[]")]
    pub query: Vec<crate::NameValue>,
    /// Optional payload; JSON placeholders must be inside strings.
    pub body: crate::RequestBody,
    /// Authentication resolved separately for each user.
    pub auth: crate::RequestAuth,
    /// Ordered bounded response checks for this step.
    #[serde(default)]
    #[ts(type = "Assertion[]")]
    pub assertions: Vec<crate::Assertion>,
}

/// Action after HTTP, assertion, binding, or required extraction failure.
///
/// # Examples
/// ```
/// let _policy = velo_engine::StepFailure::Stop;
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepFailure {
    /// Skip remaining steps in the iteration or setup phase.
    Stop,
    /// Continue; dependent unresolved requests still fail locally before sending.
    Continue,
}

/// Bounded scalar response extraction into the explicit `session:` namespace.
///
/// # Examples
/// ```
/// # fn example(extraction: velo_engine::Extraction) {
/// assert!(!extraction.name.is_empty());
/// # }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    /// Session variable name, 1 to 64 ASCII letters, digits, underscores or hyphens.
    pub name: String,
    /// Source in the final response.
    pub source: ExtractionSource,
    /// Missing or invalid values fail the step when true.
    pub required: bool,
    /// Allow replacing an existing session value; otherwise collision fails the step.
    pub overwrite: bool,
}

/// Supported response selectors; scripts and recursive JSONPath are unsupported.
///
/// # Examples
/// ```
/// let _source = velo_engine::ExtractionSource::Json { pointer: "/token".into() };
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtractionSource {
    /// RFC 6901 JSON pointer to a string, number or boolean, within the inspection body limit.
    Json {
        /// Empty selects the root; otherwise begins with `/`.
        pointer: String,
    },
    /// First matching response header, at most 4 KiB.
    Header {
        /// Case-insensitive HTTP header name.
        name: String,
    },
    /// Cookie from this final response's Set-Cookie fields, at most 4 KiB.
    Cookie {
        /// Case-sensitive cookie name.
        name: String,
    },
}

impl std::fmt::Debug for ScenarioRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ScenarioRequest([redacted])")
    }
}
