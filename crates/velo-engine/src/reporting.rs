//! Content-free configuration context shared by desktop and headless reports.

use crate::{RunConfig, ScenarioStep};

fn step_identity(step: &ScenarioStep) -> serde_json::Value {
    serde_json::json!({ "id": step.id, "method": step.request.method, "think_ms": step.think_ms, "on_failure": step.on_failure })
}

/// Retains execution settings while omitting request content and assertion expectations.
///
/// # Arguments
/// * `config` - Resolved settings, including transient data that must remain private.
/// # Returns
/// An allowlisted report context. No traffic or storage operations occur.
/// # Examples
/// ```
/// let context = velo_engine::report_configuration(&velo_engine::RunConfig::default());
/// assert!(context.get("url").is_none());
/// ```
pub fn report_configuration(config: &RunConfig) -> serde_json::Value {
    let scenario_identity = config.scenarios.as_ref().map(|plan| {
        serde_json::json!({
            "seed": plan.seed,
            "setup": plan.setup.iter().map(step_identity).collect::<Vec<_>>(),
            "cleanup": plan.cleanup.iter().map(step_identity).collect::<Vec<_>>(),
            "scenarios": plan.scenarios.iter().map(|scenario| serde_json::json!({
                "id": scenario.id, "weight": scenario.weight,
                "steps": scenario.steps.iter().map(step_identity).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    });
    serde_json::json!({
        "scenarios": scenario_identity,
        "method": config.method, "protocol": config.protocol, "max_redirects": config.max_redirects,
        "timeout_ms": config.timeout_ms, "pacing_ms": config.pacing_ms, "grace_ms": config.grace_ms,
        "workload": config.workload, "warmup_ms": config.warmup_ms, "thresholds": config.thresholds,
        "abort_policies": config.abort_policies, "inspection": config.inspection,
        "assertion_ids": config.assertions.iter().map(|assertion| &assertion.id).collect::<Vec<_>>(),
    })
}
