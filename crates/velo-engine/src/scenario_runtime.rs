//! Preparation and bounded per-user state for ordered HTTP journeys.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use reqwest::{Url, header};

use crate::{
    Extraction, ExtractionSource, RunConfig, RunError, ScenarioPlan, ScenarioRequest, ScenarioStep,
};

pub(crate) const SESSION_BYTES: usize = 32_768;
const VALUE_BYTES: usize = 4_096;
const VARIABLES_BYTES: usize = 16_384;
const COOKIE_BYTES: usize = 16_384;

pub(crate) struct PreparedPlan {
    pub definition: ScenarioPlan,
    pub base: RunConfig,
    pub multipart: Vec<Option<Vec<crate::http_options::PreparedPart>>>,
}

pub(crate) fn invalid(message: &str) -> RunError {
    RunError::new("INVALID_SCENARIO", message, Some("scenarios"))
}

fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(crate) fn request_config(base: &RunConfig, request: ScenarioRequest) -> RunConfig {
    RunConfig {
        url: request.url,
        method: request.method,
        headers: request.headers,
        query: request.query,
        body: request.body,
        auth: request.auth,
        assertions: request.assertions,
        data: None,
        scenarios: None,
        protocol: base.protocol,
        max_redirects: base.max_redirects,
        workload: base.workload.clone(),
        timeout_ms: base.timeout_ms,
        pacing_ms: base.pacing_ms,
        grace_ms: base.grace_ms,
        thresholds: Vec::new(),
        warmup_ms: 0,
        abort_policies: Vec::new(),
        inspection: base.inspection.clone(),
        budget: base.budget.clone(),
    }
}

fn placeholder_values(request: &ScenarioRequest) -> Result<BTreeMap<String, String>, RunError> {
    let encoded =
        serde_json::to_string(request).map_err(|_| invalid("Request encoding failed."))?;
    let mut rest = encoded.as_str();
    let mut values = BTreeMap::new();
    while let Some((_, tail)) = rest.split_once("{{") {
        let (reference, tail) = tail
            .split_once("}}")
            .ok_or_else(|| invalid("Close every variable reference."))?;
        let name = reference.strip_prefix("session:").ok_or_else(|| {
            invalid("Resolve environment and dataset variables before reviewing load.")
        })?;
        if !identifier(name) {
            return Err(invalid(
                "Session references require a valid session variable name.",
            ));
        }
        values.insert(name.into(), "velo".into());
        rest = tail;
    }
    Ok(values)
}

pub(crate) fn prepare(
    plan: ScenarioPlan,
    base: &RunConfig,
    client: &reqwest::Client,
    origin: &str,
) -> Result<PreparedPlan, RunError> {
    if plan.scenarios.is_empty() || plan.scenarios.len() > 16 {
        return Err(invalid("Configure 1 to 16 scenarios."));
    }
    let count = plan.setup.len()
        + plan.cleanup.len()
        + plan
            .scenarios
            .iter()
            .map(|scenario| scenario.steps.len())
            .sum::<usize>();
    if count > 64 {
        return Err(invalid(
            "Configure at most 64 steps across setup, scenarios and cleanup.",
        ));
    }
    let mut identities = BTreeSet::new();
    for scenario in &plan.scenarios {
        if !identifier(&scenario.id)
            || !identities.insert(&scenario.id)
            || !(1..=10_000).contains(&scenario.weight)
            || scenario.steps.is_empty()
        {
            return Err(invalid(
                "Use unique scenario identifiers, positive weights up to 10,000 and nonempty steps.",
            ));
        }
    }
    let mut multipart = Vec::with_capacity(count);
    for steps in std::iter::once(&plan.setup)
        .chain(plan.scenarios.iter().map(|scenario| &scenario.steps))
        .chain(std::iter::once(&plan.cleanup))
    {
        if steps.len() > 32 {
            return Err(invalid("Each phase or scenario supports at most 32 steps."));
        }
        let mut identities = BTreeSet::new();
        for step in steps {
            if !identifier(&step.id)
                || !identities.insert(&step.id)
                || step.think_ms > 3_600_000
                || step.extractions.len() > 32
            {
                return Err(invalid(
                    "Use unique step identifiers, at most 32 extractions and think time up to one hour.",
                ));
            }
            let mut names = BTreeSet::new();
            for extraction in &step.extractions {
                if !identifier(&extraction.name) || !names.insert(&extraction.name) {
                    return Err(invalid(
                        "Extraction names must be valid and unique within each step.",
                    ));
                }
                match &extraction.source {
                    ExtractionSource::Json { pointer } => {
                        if pointer.len() > 1024
                            || (!pointer.is_empty() && !pointer.starts_with('/'))
                            || pointer.contains("{{")
                        {
                            return Err(invalid(
                                "Use a literal JSON pointer of at most 1,024 bytes.",
                            ));
                        }
                        let bytes = pointer.as_bytes();
                        for (index, byte) in bytes.iter().enumerate() {
                            if *byte == b'~' && !matches!(bytes.get(index + 1), Some(b'0' | b'1')) {
                                return Err(invalid("JSON pointer escapes must be ~0 or ~1."));
                            }
                        }
                    }
                    ExtractionSource::Header { name } => {
                        header::HeaderName::from_bytes(name.as_bytes())
                            .map_err(|_| invalid("Use a valid response header name."))?;
                    }
                    ExtractionSource::Cookie { name } => {
                        if name.is_empty()
                            || name.len() > 256
                            || !name.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
                            })
                        {
                            return Err(invalid("Use a valid cookie name of at most 256 bytes."));
                        }
                    }
                }
            }
            let variables = placeholder_values(&step.request)?;
            let resolved = crate::authoring::bind_scenario_request(&step.request, &variables)?;
            let config = request_config(base, resolved);
            let checked = crate::config::validate_request(config, Some(client.clone()))?;
            if checked.confirmation.origin != origin {
                return Err(invalid(
                    "All setup, load and cleanup requests must target the reviewed origin.",
                ));
            }
            multipart.push(checked.multipart);
        }
    }
    let base = RunConfig {
        protocol: base.protocol,
        max_redirects: base.max_redirects,
        workload: base.workload.clone(),
        timeout_ms: base.timeout_ms,
        pacing_ms: base.pacing_ms,
        grace_ms: base.grace_ms,
        inspection: base.inspection.clone(),
        budget: base.budget.clone(),
        ..RunConfig::default()
    };
    Ok(PreparedPlan {
        definition: plan,
        base,
        multipart,
    })
}

pub(crate) fn same_identities(left: &ScenarioPlan, right: &ScenarioPlan) -> bool {
    fn keys(plan: &ScenarioPlan) -> Vec<(&str, Vec<&str>)> {
        std::iter::once(("", plan.setup.iter().map(|step| step.id.as_str()).collect()))
            .chain(plan.scenarios.iter().map(|scenario| {
                (
                    scenario.id.as_str(),
                    scenario.steps.iter().map(|step| step.id.as_str()).collect(),
                )
            }))
            .chain(std::iter::once((
                "",
                plan.cleanup.iter().map(|step| step.id.as_str()).collect(),
            )))
            .collect()
    }
    keys(left) == keys(right)
        && left.seed == right.seed
        && left
            .scenarios
            .iter()
            .map(|scenario| scenario.weight)
            .eq(right.scenarios.iter().map(|scenario| scenario.weight))
}

/// Uses the SplitMix64 mixing function on seed plus global admission ordinal.
/// Multiplication maps the mixed value to a relative-weight ticket.
pub(crate) fn choose(plan: &ScenarioPlan, ordinal: u64) -> usize {
    let mut random = ordinal
        .wrapping_add(u64::from(plan.seed))
        .wrapping_add(0x9e3779b97f4a7c15);
    random = (random ^ (random >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    random = (random ^ (random >> 27)).wrapping_mul(0x94d049bb133111eb);
    random ^= random >> 31;
    let total: u64 = plan
        .scenarios
        .iter()
        .map(|scenario| u64::from(scenario.weight))
        .sum();
    let mut ticket = ((u128::from(random) * u128::from(total)) >> 64) as u64;
    for (index, scenario) in plan.scenarios.iter().enumerate() {
        if ticket < u64::from(scenario.weight) {
            return index;
        }
        ticket -= u64::from(scenario.weight);
    }
    unreachable!("validated positive scenario weights cover every selection ticket")
}

#[derive(Clone, Default)]
pub(crate) struct Session {
    pub values: BTreeMap<String, String>,
    cookies: cookie_store::CookieStore,
}

impl Session {
    pub(crate) fn request_cookies(
        &self,
        url: &Url,
    ) -> Result<Option<header::HeaderValue>, &'static str> {
        let value = self
            .cookies
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        if value.is_empty() {
            return Ok(None);
        }
        header::HeaderValue::from_str(&value)
            .map(Some)
            .map_err(|_| "SESSION_COOKIE_INVALID")
    }

    pub(crate) fn response_cookies(
        &mut self,
        headers: &header::HeaderMap,
        url: &Url,
    ) -> Result<(), &'static str> {
        let existing = self
            .cookies
            .iter_unexpired()
            .cloned()
            .map(Ok::<_, std::convert::Infallible>);
        let mut next = match cookie_store::CookieStore::from_cookies(existing, false) {
            Ok(store) => store,
            Err(error) => match error {},
        };
        for value in headers.get_all(header::SET_COOKIE) {
            let value = value.to_str().map_err(|_| "SESSION_COOKIE_INVALID")?;
            if value.len() > VALUE_BYTES {
                return Err("SESSION_BUDGET_EXCEEDED");
            }
            let _ = next.parse(value, url);
            let bytes = next
                .iter_any()
                .map(|cookie| {
                    cookie.to_string().len() + std::mem::size_of::<cookie_store::Cookie<'static>>()
                })
                .sum::<usize>();
            if next.iter_any().count() > 32 || bytes > COOKIE_BYTES {
                return Err("SESSION_BUDGET_EXCEEDED");
            }
        }
        self.cookies = next;
        Ok(())
    }

    pub(crate) fn invalidate(&mut self, extractions: &[Extraction]) {
        for extraction in extractions {
            self.values.remove(&extraction.name);
        }
    }

    pub(crate) fn extract(
        &mut self,
        extractions: &[Extraction],
        headers: &header::HeaderMap,
        body: &[u8],
        truncated: bool,
    ) -> Result<(), &'static str> {
        let json_required = extractions
            .iter()
            .any(|extraction| matches!(extraction.source, ExtractionSource::Json { .. }));
        let encoded = headers
            .get(header::CONTENT_ENCODING)
            .is_some_and(|value| !value.as_bytes().eq_ignore_ascii_case(b"identity"));
        let json = if json_required && !truncated && !encoded {
            serde_json::from_slice::<serde_json::Value>(body).ok()
        } else {
            None
        };
        let mut next = self.values.clone();
        let mut error = None;
        for extraction in extractions {
            let value = match &extraction.source {
                ExtractionSource::Json { pointer } => json
                    .as_ref()
                    .and_then(|json| json.pointer(pointer))
                    .and_then(|value| match value {
                        serde_json::Value::String(text) => Some(text.clone()),
                        serde_json::Value::Number(_) | serde_json::Value::Bool(_) => {
                            Some(value.to_string())
                        }
                        _ => None,
                    }),
                ExtractionSource::Header { name } => headers
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                ExtractionSource::Cookie { name } => headers
                    .get_all(header::SET_COOKIE)
                    .iter()
                    .find_map(|value| {
                        let value = value.to_str().ok()?;
                        let cookie = cookie_store::RawCookie::parse(value).ok()?;
                        (cookie.name() == name).then(|| cookie.value().to_owned())
                    }),
            };
            if value
                .as_ref()
                .is_some_and(|value| value.len() > VALUE_BYTES)
            {
                error.get_or_insert("EXTRACTION_VALUE_TOO_LARGE");
                continue;
            }
            if let Some(value) = value {
                if next.contains_key(&extraction.name) && !extraction.overwrite {
                    error.get_or_insert("SESSION_OVERWRITE_REQUIRED");
                } else {
                    next.insert(extraction.name.clone(), value);
                }
            } else {
                next.remove(&extraction.name);
                if extraction.required {
                    let cause = if matches!(extraction.source, ExtractionSource::Json { .. }) {
                        if truncated {
                            "EXTRACTION_BODY_TOO_LARGE"
                        } else if encoded {
                            "EXTRACTION_ENCODED_BODY"
                        } else if json.is_none() {
                            "EXTRACTION_INVALID_JSON"
                        } else {
                            "EXTRACTION_REQUIRED_MISSING"
                        }
                    } else {
                        "EXTRACTION_REQUIRED_MISSING"
                    };
                    error.get_or_insert(cause);
                }
            }
        }
        let bytes = next
            .iter()
            .map(|(name, value)| name.len() + value.len())
            .sum::<usize>();
        if next.len() > 32 || bytes > VARIABLES_BYTES {
            error.get_or_insert("SESSION_BUDGET_EXCEEDED");
        }
        if let Some(error) = error {
            self.invalidate(extractions);
            return Err(error);
        }
        self.values = next;
        Ok(())
    }
}

pub(crate) struct Exchange<'a> {
    pub session: &'a mut Session,
    pub step: &'a ScenarioStep,
    pub error: Option<&'static str>,
}

pub(crate) fn think_time(step: &ScenarioStep) -> Duration {
    Duration::from_millis(u64::from(step.think_ms))
}

pub(crate) fn retained_bytes(plan: &PreparedPlan) -> Result<usize, RunError> {
    let encoded =
        serde_json::to_vec(&plan.definition).map_err(|_| invalid("Scenario encoding failed."))?;
    Ok(encoded
        .len()
        .saturating_mul(2)
        .saturating_add(std::mem::size_of::<PreparedPlan>())
        .saturating_add(
            plan.multipart.len()
                * std::mem::size_of::<Option<Vec<crate::http_options::PreparedPart>>>(),
        ))
}

pub(crate) fn check_binding_budget(
    request: &ScenarioRequest,
    values: &BTreeMap<String, String>,
    limit: usize,
) -> Result<(), RunError> {
    let encoded =
        serde_json::to_string(request).map_err(|_| invalid("Request encoding failed."))?;
    let mut bytes = encoded.len();
    let mut rest = encoded.as_str();
    while let Some((_, tail)) = rest.split_once("{{session:") {
        let Some((name, tail)) = tail.split_once("}}") else {
            return Err(invalid("Close every session reference."));
        };
        if let Some(value) = values.get(name) {
            bytes = bytes.saturating_add(value.len().saturating_mul(6));
        }
        if bytes > limit {
            return Err(RunError::new(
                "SCENARIO_BINDING_BUDGET_EXCEEDED",
                "Resolved request exceeds its per-worker byte allowance.",
                Some("scenarios"),
            ));
        }
        rest = tail;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_seeded_in_admission_order_with_relative_weights() {
        let plan = ScenarioPlan {
            seed: 17,
            scenarios: vec![
                crate::Scenario {
                    id: "small".into(),
                    weight: 1,
                    steps: vec![],
                },
                crate::Scenario {
                    id: "large".into(),
                    weight: 3,
                    steps: vec![],
                },
            ],
            ..ScenarioPlan::default()
        };
        let first: Vec<usize> = (0..12).map(|ordinal| choose(&plan, ordinal)).collect();
        assert_eq!(first, vec![1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1]);
        let small = (0..10_000)
            .filter(|ordinal| choose(&plan, *ordinal) == 0)
            .count();
        assert!((2_300..2_700).contains(&small));
        let changed = ScenarioPlan { seed: 42, ..plan };
        assert_ne!(
            first,
            (0..12)
                .map(|ordinal| choose(&changed, ordinal))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn cookie_scope_secure_expiry_and_state_budgets_are_enforced() {
        let secure = Url::parse("https://example.test/private/login").unwrap();
        let insecure = Url::parse("http://example.test/private/data").unwrap();
        let other = Url::parse("https://other.test/private/data").unwrap();
        let public = Url::parse("https://example.test/public").unwrap();
        let mut session = Session::default();
        let mut headers = header::HeaderMap::new();
        headers.append(
            header::SET_COOKIE,
            header::HeaderValue::from_static("sid=secret; Path=/private; Secure; HttpOnly"),
        );
        session.response_cookies(&headers, &secure).unwrap();
        assert!(session.request_cookies(&insecure).unwrap().is_none());
        assert!(session.request_cookies(&other).unwrap().is_none());
        assert!(session.request_cookies(&public).unwrap().is_none());
        assert_eq!(
            session.request_cookies(&secure).unwrap().unwrap(),
            "sid=secret"
        );
        headers.insert(
            header::SET_COOKIE,
            header::HeaderValue::from_static("sid=expired; Path=/private; Max-Age=0; Secure"),
        );
        session.response_cookies(&headers, &secure).unwrap();
        assert!(session.request_cookies(&secure).unwrap().is_none());
        let request = ScenarioRequest {
            url: "https://example.test/{{session:value}}".into(),
            method: crate::RequestMethod::GET,
            headers: vec![],
            query: vec![],
            body: crate::RequestBody::None,
            auth: crate::RequestAuth::None,
            assertions: vec![],
        };
        let values = BTreeMap::from([("value".into(), "a".repeat(4096))]);
        assert_eq!(
            check_binding_budget(&request, &values, 1024)
                .unwrap_err()
                .code,
            "SCENARIO_BINDING_BUDGET_EXCEEDED"
        );
    }
}
