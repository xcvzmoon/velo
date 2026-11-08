use serde_json::{Value, json};
use velo_contracts::analysis::redact_evidence;
use velo_engine::TestDefinition;

use super::{AiReview, AiTask, MAX_CONTEXT_BYTES, failure};

fn description(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "authorization",
                "password",
                "api_key",
                "api-key",
                "apikey",
                "secret",
                "token",
                "cookie",
                "sk-",
                "aiza",
                "{{secret:",
            ]
            .iter()
            .any(|term| lower.contains(term))
            {
                "[REDACTED: credential-like input]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn prepare(
    task: AiTask,
    source: &str,
    definition: Option<TestDefinition>,
    selected_evidence: Option<Value>,
) -> Result<AiReview, velo_engine::RunError> {
    if source.len() > 32_768
        || selected_evidence
            .as_ref()
            .is_some_and(|value| value.to_string().len() > MAX_CONTEXT_BYTES)
    {
        return Err(failure(
            "AI_CONTEXT_TOO_LARGE",
            "Select less context; descriptions are limited to 32 KiB and evidence to 128 KiB.",
        ));
    }
    let template = definition.unwrap_or_default();
    let selected = json!({
        "method": template.config.method,
        "workload": template.config.workload,
        "timeout_ms": template.config.timeout_ms,
        "pacing_ms": template.config.pacing_ms,
        "grace_ms": template.config.grace_ms,
        "warmup_ms": template.config.warmup_ms,
        "budget": template.config.budget,
    });
    let sanitized_evidence = redact_evidence(&selected_evidence.unwrap_or(Value::Null));
    let content = json!({
        "task": task,
        "description": description(source),
        "selected_definition": selected,
        "evidence": sanitized_evidence,
    })
    .to_string();
    if content.len() > MAX_CONTEXT_BYTES {
        return Err(failure(
            "AI_CONTEXT_TOO_LARGE",
            "Select less context before reviewing.",
        ));
    }
    Ok(AiReview {
        id: uuid::Uuid::new_v4().to_string(),
        config: super::AiProviderConfig::default(),
        content,
        redactions: vec![
            "Request URLs, paths, headers, query pairs, authentication, bodies, variables, environments, assertions, scenarios, and dataset values are excluded from selected definitions.".into(),
            "Evidence keeps allowlisted numeric measurements and completeness flags. Names, response content, labels, and unsupported fields are excluded.".into(),
            "Credential-like description lines are redacted. Review all remaining free text before sending; arbitrary secrets cannot be identified reliably.".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_secrets_and_untrusted_evidence_keys_never_enter_review() {
        let mut definition = TestDefinition::default();
        definition.config.url = "https://private.example/PRIVATE?token=PRIVATE".into();
        definition
            .variables
            .insert("PRIVATE".into(), "PRIVATE".into());
        let review = prepare(AiTask::Draft, "Build a test\nAuthorization: PRIVATE\n{{secret:PRIVATE}}", Some(definition), Some(json!({"candidate":{"metrics":{"request_rate":12.0,"PRIVATE":123,"counters":{"responses":"42"}}},"PRIVATE":true}))).unwrap();
        assert!(!review.content.contains("PRIVATE"));
        let value: Value = serde_json::from_str(&review.content).unwrap();
        assert_eq!(
            value.pointer("/evidence/candidate/metrics/counters/responses"),
            Some(&json!(42))
        );
        assert_eq!(
            value.pointer("/evidence/candidate/metrics/request_rate"),
            Some(&json!(12.0))
        );
    }

    #[test]
    fn bounded_inputs_are_rejected_before_context_storage() {
        assert_eq!(
            prepare(AiTask::Draft, &"a".repeat(32_769), None, None)
                .unwrap_err()
                .code,
            "AI_CONTEXT_TOO_LARGE"
        );
    }
}
