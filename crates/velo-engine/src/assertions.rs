//! Response checks and bounded, content-free diagnostic evidence.
//!
//! Checks inspect one final response after redirects. Body bytes are retained only
//! up to the configured cap, while transport still drains the complete response.
//! No response content or assertion expectations are retained in diagnostics.

use reqwest::header::{HeaderMap, HeaderName};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::RunError;

/// A named response check, validated before any load traffic is admitted.
/// Expectations may contain private data and are omitted from Debug output.
///
/// # Examples
/// ```
/// let check = velo_engine::Assertion {
///     id: "healthy".into(),
///     rule: velo_engine::AssertionRule::Status { allowed: vec![200, 204] },
/// };
/// assert_eq!(check.id, "healthy");
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Assertion {
    /// Unique ASCII identifier, 1–64 letters, digits, underscores or hyphens.
    pub id: String,
    /// Condition evaluated against the final response.
    pub rule: AssertionRule,
}

impl std::fmt::Debug for Assertion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Assertion([redacted])")
    }
}

/// Supported conditions; JSON equality compares parsed JSON values, including types.
/// JSON pointers use RFC 6901, with an empty pointer selecting the entire document.
/// Text checks require a complete UTF-8 body. Expectations are omitted from Debug.
///
/// # Examples
/// ```
/// let rule = velo_engine::AssertionRule::JsonEquals {
///     pointer: "/ok".into(), value: "true".into(),
/// };
/// assert!(!format!("{rule:?}").contains("true"));
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssertionRule {
    /// At least one allowed status must match; 1–100 statuses in 100–599.
    Status {
        #[ts(type = "number[]")]
        allowed: Vec<u16>,
    },
    /// Header names are case insensitive; an empty header value still counts as present.
    HeaderPresent { name: String },
    /// Any occurrence of the named header must exactly match the configured value.
    HeaderEquals { name: String, value: String },
    /// The JSON pointer must exist; a JSON null value is present.
    JsonPresent { pointer: String },
    /// The pointer must exist and equal the valid JSON expectation.
    JsonEquals { pointer: String, value: String },
    /// The complete UTF-8 body must contain the nonempty literal.
    TextContains { value: String },
    /// The complete UTF-8 body must exactly equal the literal.
    TextEquals { value: String },
}

impl std::fmt::Debug for AssertionRule {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AssertionRule([redacted])")
    }
}

/// Inspection and diagnostic retention limits with hard maxima enforced at preparation.
/// Samples contain only status, check index and stable cause, never response values.
///
/// # Examples
/// ```
/// let limits = velo_engine::InspectionLimits::default();
/// assert_eq!(limits.body_bytes, 16_384);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InspectionLimits {
    /// Body inspection bytes per worker, 1–65,536. Excess bytes are drained and discarded.
    pub body_bytes: u32,
    /// Retained representative samples per run, 0–32. Zero disables samples.
    pub sample_count: u32,
    /// Total serialized sample bytes per run, 0–32,768. Zero disables samples.
    pub sample_bytes: u32,
}

impl Default for InspectionLimits {
    fn default() -> Self {
        Self {
            body_bytes: 16_384,
            sample_count: 16,
            sample_bytes: 8_192,
        }
    }
}

/// Stable evaluation outcome or request failure cause; no target text is included.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::CheckOutcome::Truncated)?, "truncated");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// Condition matched.
    Passed,
    /// Allowed status did not match.
    StatusMismatch,
    /// Required header was absent.
    MissingHeader,
    /// Header value did not match.
    HeaderMismatch,
    /// JSON pointer was absent.
    MissingField,
    /// Parsed JSON value did not match.
    JsonMismatch,
    /// Text condition did not match.
    TextMismatch,
    /// Inspection cap was exceeded; body checks cannot pass on prefixes.
    Truncated,
    /// Complete body was not valid JSON, including excessive JSON nesting.
    InvalidJson,
    /// Complete body was not valid UTF-8.
    InvalidUtf8,
    /// Content encoding is unsupported; encoded payloads are not decoded by the engine.
    EncodedBody,
    /// A complete response was not obtained due to transport, timeout or cancellation.
    Unavailable,
}

impl CheckOutcome {
    fn failed(self) -> bool {
        matches!(
            self,
            Self::StatusMismatch
                | Self::MissingHeader
                | Self::HeaderMismatch
                | Self::MissingField
                | Self::JsonMismatch
                | Self::TextMismatch
        )
    }
}

/// Totals for one configured assertion; every terminal attempt contributes exactly once.
/// `passed + failed + unavailable` equals terminal attempts, including cancellations.
///
/// # Examples
/// ```
/// # fn example(total: velo_engine::AssertionTotals) {
/// assert!(total.id.len() <= 64);
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct AssertionTotals {
    /// Stable configured identifier; expectations are never included.
    pub id: String,
    /// Matching complete responses.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub passed: u64,
    /// Evaluated conditions that did not match.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub failed: u64,
    /// Checks not evaluable, including invalid or truncated bodies and non-response attempts.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub unavailable: u64,
}

/// A safe representative failure, with response content withheld before IPC or persistence.
///
/// # Examples
/// ```
/// # fn example(sample: velo_engine::ResponseSample) {
/// assert!(sample.cause.len() <= 32);
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct ResponseSample {
    /// Zero-based assertion index, or None for HTTP, transport, timeout or cancellation causes.
    pub assertion: Option<u32>,
    /// Final response status when headers arrived; absent for connection failures.
    pub status: Option<u16>,
    /// Stable cause, never an infrastructure message or actual/expected value.
    pub cause: String,
}

/// Aggregate occurrences of one assertion/cause/status combination.
///
/// # Examples
/// ```
/// # fn example(group: velo_engine::FailureGroup) {
/// assert!(group.count > 0);
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct FailureGroup {
    /// Safe representative metadata for this group.
    pub example: ResponseSample,
    /// Occurrences, including those whose sample could not be retained.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub count: u64,
}

/// Bounded cumulative assertion results and redacted diagnostic evidence.
/// Samples live with their run and are removed when that run is deleted or evicted.
///
/// # Examples
/// ```
/// # fn example(diagnostics: velo_engine::ResponseDiagnostics) {
/// assert!(diagnostics.samples.len() <= diagnostics.limits.sample_count as usize);
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct ResponseDiagnostics {
    /// Per-assertion totals in configuration order.
    #[ts(type = "AssertionTotals[]")]
    pub assertions: Vec<AssertionTotals>,
    /// Groups capped at 256; omitted group occurrences are counted separately.
    #[ts(type = "FailureGroup[]")]
    pub groups: Vec<FailureGroup>,
    /// First representative for each retained group, within count and serialized byte limits.
    #[ts(type = "ResponseSample[]")]
    pub samples: Vec<ResponseSample>,
    /// New representative samples omitted by retention limits.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub dropped_samples: u64,
    /// Failure occurrences that could not fit in the group table.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub dropped_groups: u64,
    /// Complete responses whose inspected body exceeded the cap.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub truncated_responses: u64,
    /// Serialized bytes of the nonempty samples array, including delimiters; zero when empty.
    pub retained_bytes: u32,
    /// Active run limits.
    pub limits: InspectionLimits,
}

#[derive(Default)]
pub(crate) struct Observation {
    pub status: Option<u16>,
    pub checks: Vec<CheckOutcome>,
    pub truncated: bool,
    pub cause: Option<&'static str>,
}

pub(crate) struct PreparedAssertions {
    pub definitions: Vec<Assertion>,
    expected: Vec<Option<serde_json::Value>>,
}

pub(crate) fn prepare(
    definitions: Vec<Assertion>,
    limits: &InspectionLimits,
) -> Result<PreparedAssertions, RunError> {
    let invalid = || {
        RunError::new(
            "INVALID_ASSERTION",
            "Use at most 32 unique assertions with valid status, header, JSON pointer and bounded expectations.",
            Some("assertions"),
        )
    };
    if !(1..=65_536).contains(&limits.body_bytes)
        || limits.sample_count > 32
        || limits.sample_bytes > 32_768
    {
        return Err(RunError::new(
            "INVALID_INSPECTION_LIMITS",
            "Inspection is limited to 1–65,536 body bytes, 32 samples and 32,768 sample bytes.",
            Some("inspection"),
        ));
    }
    if definitions.len() > 32 {
        return Err(invalid());
    }
    let mut expected = Vec::with_capacity(definitions.len());
    for (index, assertion) in definitions.iter().enumerate() {
        if assertion.id.is_empty()
            || assertion.id.len() > 64
            || !assertion
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || definitions[..index]
                .iter()
                .any(|previous| previous.id == assertion.id)
        {
            return Err(invalid());
        }
        let mut parsed = None;
        match &assertion.rule {
            AssertionRule::Status { allowed } => {
                if allowed.is_empty()
                    || allowed.len() > 100
                    || allowed.iter().any(|status| !(100..=599).contains(status))
                {
                    return Err(invalid());
                }
            }
            AssertionRule::HeaderPresent { name } | AssertionRule::HeaderEquals { name, .. } => {
                if name.len() > 256 || HeaderName::from_bytes(name.as_bytes()).is_err() {
                    return Err(invalid());
                }
            }
            AssertionRule::JsonPresent { pointer } | AssertionRule::JsonEquals { pointer, .. } => {
                if pointer.len() > 1024 || (!pointer.is_empty() && !pointer.starts_with('/')) {
                    return Err(invalid());
                }
                let mut chars = pointer.chars();
                while let Some(character) = chars.next() {
                    if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                        return Err(invalid());
                    }
                }
            }
            _ => {}
        }
        match &assertion.rule {
            AssertionRule::HeaderEquals { value, .. }
            | AssertionRule::JsonEquals { value, .. }
            | AssertionRule::TextContains { value }
            | AssertionRule::TextEquals { value } => {
                if value.len() > 4096 {
                    return Err(invalid());
                }
                if matches!(assertion.rule, AssertionRule::TextContains { .. }) && value.is_empty()
                {
                    return Err(invalid());
                }
                if matches!(assertion.rule, AssertionRule::HeaderEquals { .. })
                    && reqwest::header::HeaderValue::from_str(value).is_err()
                {
                    return Err(invalid());
                }
                if matches!(assertion.rule, AssertionRule::JsonEquals { .. }) {
                    parsed = Some(serde_json::from_str(value).map_err(|_| invalid())?);
                }
            }
            _ => {}
        }
        expected.push(parsed);
    }
    Ok(PreparedAssertions {
        definitions,
        expected,
    })
}

impl PreparedAssertions {
    pub(crate) fn needs_body(&self) -> bool {
        self.definitions.iter().any(|assertion| {
            matches!(
                assertion.rule,
                AssertionRule::JsonPresent { .. }
                    | AssertionRule::JsonEquals { .. }
                    | AssertionRule::TextContains { .. }
                    | AssertionRule::TextEquals { .. }
            )
        })
    }

    pub(crate) fn evaluate(
        &self,
        status: u16,
        headers: &HeaderMap,
        body: &[u8],
        truncated: bool,
    ) -> Vec<CheckOutcome> {
        let encoded = headers
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| !value.as_bytes().eq_ignore_ascii_case(b"identity"));
        let json = if !truncated
            && !encoded
            && self.definitions.iter().any(|assertion| {
                matches!(
                    assertion.rule,
                    AssertionRule::JsonPresent { .. } | AssertionRule::JsonEquals { .. }
                )
            }) {
            serde_json::from_slice::<serde_json::Value>(body).ok()
        } else {
            None
        };
        let text = std::str::from_utf8(body).ok();
        self.definitions
            .iter()
            .enumerate()
            .map(|(index, assertion)| {
                let matched = match &assertion.rule {
                    AssertionRule::Status { allowed } => {
                        return if allowed.contains(&status) {
                            CheckOutcome::Passed
                        } else {
                            CheckOutcome::StatusMismatch
                        };
                    }
                    AssertionRule::HeaderPresent { name } => {
                        return if headers.contains_key(name) {
                            CheckOutcome::Passed
                        } else {
                            CheckOutcome::MissingHeader
                        };
                    }
                    AssertionRule::HeaderEquals { name, value } => {
                        return if !headers.contains_key(name) {
                            CheckOutcome::MissingHeader
                        } else if headers
                            .get_all(name)
                            .iter()
                            .any(|actual| actual.as_bytes() == value.as_bytes())
                        {
                            CheckOutcome::Passed
                        } else {
                            CheckOutcome::HeaderMismatch
                        };
                    }
                    _ if truncated => return CheckOutcome::Truncated,
                    _ if encoded => return CheckOutcome::EncodedBody,
                    AssertionRule::JsonPresent { pointer }
                    | AssertionRule::JsonEquals { pointer, .. } => {
                        let Some(json) = &json else {
                            return CheckOutcome::InvalidJson;
                        };
                        let Some(actual) = json.pointer(pointer) else {
                            return CheckOutcome::MissingField;
                        };
                        if matches!(assertion.rule, AssertionRule::JsonPresent { .. }) {
                            true
                        } else {
                            Some(actual) == self.expected[index].as_ref()
                        }
                    }
                    AssertionRule::TextContains { value } => {
                        let Some(text) = text else {
                            return CheckOutcome::InvalidUtf8;
                        };
                        text.contains(value)
                    }
                    AssertionRule::TextEquals { value } => {
                        let Some(text) = text else {
                            return CheckOutcome::InvalidUtf8;
                        };
                        text == value
                    }
                };
                if matched {
                    CheckOutcome::Passed
                } else if matches!(assertion.rule, AssertionRule::JsonEquals { .. }) {
                    CheckOutcome::JsonMismatch
                } else {
                    CheckOutcome::TextMismatch
                }
            })
            .collect()
    }
}

impl ResponseDiagnostics {
    pub(crate) fn new(assertions: &[Assertion], limits: InspectionLimits) -> Self {
        Self {
            assertions: assertions
                .iter()
                .map(|assertion| AssertionTotals {
                    id: assertion.id.clone(),
                    passed: 0,
                    failed: 0,
                    unavailable: 0,
                })
                .collect(),
            groups: Vec::new(),
            samples: Vec::new(),
            dropped_samples: 0,
            dropped_groups: 0,
            truncated_responses: 0,
            retained_bytes: 0,
            limits,
        }
    }

    pub(crate) fn observe(&mut self, observation: &Observation, complete: bool) {
        if complete && observation.truncated {
            self.truncated_responses = self.truncated_responses.saturating_add(1);
        }
        for index in 0..self.assertions.len() {
            let outcome = if complete {
                observation
                    .checks
                    .get(index)
                    .copied()
                    .unwrap_or(CheckOutcome::Unavailable)
            } else {
                CheckOutcome::Unavailable
            };
            let totals = &mut self.assertions[index];
            let counter = if outcome == CheckOutcome::Passed {
                &mut totals.passed
            } else if outcome.failed() {
                &mut totals.failed
            } else {
                &mut totals.unavailable
            };
            *counter = counter.saturating_add(1);
            if complete && outcome != CheckOutcome::Passed {
                let cause = match outcome {
                    CheckOutcome::Passed => continue,
                    CheckOutcome::StatusMismatch => "status_mismatch",
                    CheckOutcome::MissingHeader => "missing_header",
                    CheckOutcome::HeaderMismatch => "header_mismatch",
                    CheckOutcome::MissingField => "missing_field",
                    CheckOutcome::JsonMismatch => "json_mismatch",
                    CheckOutcome::TextMismatch => "text_mismatch",
                    CheckOutcome::Truncated => "truncated",
                    CheckOutcome::InvalidJson => "invalid_json",
                    CheckOutcome::InvalidUtf8 => "invalid_utf8",
                    CheckOutcome::EncodedBody => "encoded_body",
                    CheckOutcome::Unavailable => "unavailable",
                };
                self.group(ResponseSample {
                    assertion: Some(index as u32),
                    status: observation.status,
                    cause: cause.into(),
                });
            }
        }
        if let Some(cause) = observation.cause {
            self.group(ResponseSample {
                assertion: None,
                status: observation.status,
                cause: cause.into(),
            });
        } else if complete
            && observation
                .status
                .is_some_and(|status| !(200..400).contains(&status))
        {
            self.group(ResponseSample {
                assertion: None,
                status: observation.status,
                cause: "http_status".into(),
            });
        }
    }

    fn group(&mut self, example: ResponseSample) {
        if let Some(group) = self
            .groups
            .iter_mut()
            .find(|group| group.example == example)
        {
            group.count = group.count.saturating_add(1);
            return;
        }
        if self.groups.len() >= 256 {
            self.dropped_groups = self.dropped_groups.saturating_add(1);
            self.dropped_samples = self.dropped_samples.saturating_add(1);
            return;
        }
        let size = serde_json::to_vec(&example).map_or(u32::MAX, |bytes| bytes.len() as u32);
        let retained = if self.samples.is_empty() {
            size.saturating_add(2)
        } else {
            self.retained_bytes.saturating_add(size).saturating_add(1)
        };
        if self.samples.len() < self.limits.sample_count as usize
            && retained <= self.limits.sample_bytes
        {
            self.samples.push(example.clone());
            self.retained_bytes = retained;
        } else {
            self.dropped_samples = self.dropped_samples.saturating_add(1);
        }
        self.groups.push(FailureGroup { example, count: 1 });
    }

    pub(crate) fn failure_rate(&self) -> Option<f64> {
        let mut passed = 0.0;
        let mut failed = 0.0;
        for total in &self.assertions {
            if total.unavailable != 0 {
                return None;
            }
            passed += total.passed as f64;
            failed += total.failed as f64;
        }
        (passed + failed > 0.0).then_some(failed / (passed + failed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assertion(rule: AssertionRule) -> Assertion {
        Assertion {
            id: "check".into(),
            rule,
        }
    }

    #[test]
    fn definitions_and_limits_reject_invalid_input_before_execution() {
        for rule in [
            AssertionRule::Status { allowed: vec![] },
            AssertionRule::Status { allowed: vec![600] },
            AssertionRule::HeaderPresent {
                name: "bad\nheader".into(),
            },
            AssertionRule::HeaderEquals {
                name: "ok".into(),
                value: "bad\nvalue".into(),
            },
            AssertionRule::JsonPresent {
                pointer: "missing-slash".into(),
            },
            AssertionRule::JsonPresent {
                pointer: "/bad~2escape".into(),
            },
            AssertionRule::JsonEquals {
                pointer: "/ok".into(),
                value: "not json".into(),
            },
            AssertionRule::TextContains {
                value: String::new(),
            },
            AssertionRule::TextEquals {
                value: "x".repeat(4097),
            },
        ] {
            assert_eq!(
                prepare(vec![assertion(rule)], &InspectionLimits::default())
                    .err()
                    .unwrap()
                    .code,
                "INVALID_ASSERTION"
            );
        }
        let valid = assertion(AssertionRule::Status { allowed: vec![200] });
        for definitions in [
            vec![valid.clone(), valid.clone()],
            vec![valid.clone(); 33],
            vec![Assertion {
                id: " ".into(),
                ..valid
            }],
        ] {
            assert!(prepare(definitions, &InspectionLimits::default()).is_err());
        }
        for limits in [
            InspectionLimits {
                body_bytes: 0,
                ..Default::default()
            },
            InspectionLimits {
                body_bytes: 65_537,
                ..Default::default()
            },
            InspectionLimits {
                sample_count: 33,
                ..Default::default()
            },
            InspectionLimits {
                sample_bytes: 32_769,
                ..Default::default()
            },
        ] {
            assert_eq!(
                prepare(vec![], &limits).err().unwrap().code,
                "INVALID_INSPECTION_LIMITS"
            );
        }
    }

    #[test]
    fn text_json_and_header_rules_handle_absence_types_and_invalid_bytes() {
        for (rule, body, expected) in [
            (
                AssertionRule::TextEquals {
                    value: "hello".into(),
                },
                b"hello".as_slice(),
                CheckOutcome::Passed,
            ),
            (
                AssertionRule::TextEquals {
                    value: "hello".into(),
                },
                b"hello!".as_slice(),
                CheckOutcome::TextMismatch,
            ),
            (
                AssertionRule::TextContains {
                    value: "hello".into(),
                },
                b"\xffhello".as_slice(),
                CheckOutcome::InvalidUtf8,
            ),
            (
                AssertionRule::JsonPresent {
                    pointer: "/0/a~0b".into(),
                },
                b"[{\"a~b\":null}]".as_slice(),
                CheckOutcome::Passed,
            ),
            (
                AssertionRule::JsonEquals {
                    pointer: "".into(),
                    value: "null".into(),
                },
                b"null".as_slice(),
                CheckOutcome::Passed,
            ),
            (
                AssertionRule::HeaderPresent {
                    name: "missing".into(),
                },
                b"".as_slice(),
                CheckOutcome::MissingHeader,
            ),
            (
                AssertionRule::HeaderEquals {
                    name: "missing".into(),
                    value: "".into(),
                },
                b"".as_slice(),
                CheckOutcome::MissingHeader,
            ),
        ] {
            let checks = prepare(vec![assertion(rule)], &InspectionLimits::default()).unwrap();
            assert_eq!(
                checks.evaluate(200, &HeaderMap::new(), body, false),
                vec![expected]
            );
        }
        let checks = prepare(
            vec![assertion(AssertionRule::TextContains {
                value: "prefix".into(),
            })],
            &InspectionLimits::default(),
        )
        .unwrap();
        assert_eq!(
            checks.evaluate(200, &HeaderMap::new(), b"prefix", true),
            vec![CheckOutcome::Truncated]
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_ENCODING,
            reqwest::header::HeaderValue::from_static("gzip"),
        );
        assert_eq!(
            checks.evaluate(200, &headers, b"prefix", false),
            vec![CheckOutcome::EncodedBody]
        );
    }

    #[test]
    fn representative_retention_remains_bounded_under_sustained_failures() {
        let mut diagnostics = ResponseDiagnostics::new(
            &[],
            InspectionLimits {
                sample_count: 2,
                sample_bytes: 256,
                ..Default::default()
            },
        );
        for _ in 0..100 {
            for status in 400..599 {
                diagnostics.observe(
                    &Observation {
                        status: Some(status),
                        ..Default::default()
                    },
                    true,
                );
            }
        }
        assert_eq!(diagnostics.samples.len(), 2);
        assert_eq!(diagnostics.dropped_samples, 197);
        assert_eq!(diagnostics.groups.len(), 199);
        assert!(diagnostics.groups.iter().all(|group| group.count == 100));
        assert_eq!(
            serde_json::to_vec(&diagnostics.samples).unwrap().len(),
            diagnostics.retained_bytes as usize
        );
        for status in 100..399 {
            diagnostics.group(ResponseSample {
                assertion: Some(0),
                status: Some(status),
                cause: "missing_field".into(),
            });
        }
        assert_eq!(diagnostics.groups.len(), 256);
        assert_eq!(diagnostics.dropped_groups, 242);
        assert!(diagnostics.retained_bytes <= 256);
        let mut disabled = ResponseDiagnostics::new(
            &[],
            InspectionLimits {
                sample_bytes: 0,
                ..Default::default()
            },
        );
        disabled.observe(
            &Observation {
                cause: Some("timeout"),
                ..Default::default()
            },
            false,
        );
        assert_eq!(disabled.samples.len(), 0);
        assert_eq!(disabled.dropped_samples, 1);
        assert_eq!(disabled.groups[0].count, 1);
    }
}
