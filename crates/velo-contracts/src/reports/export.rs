//! Standalone, escaped exporters shared by native and future CLI callers.

use serde_json::Value;

use super::{ComparisonReport, RegressionVerdict, limitations};
use crate::CommandError;

/// Renders a versioned report as precision-preserving JSON.
///
/// # Arguments
/// * `report` - Evidence created from redacted history by [`crate::reports::compare`].
/// # Returns
/// Pretty JSON; all native counters remain decimal strings.
/// # Errors
/// Returns `REPORT_ENCODING_FAILED` if serialization fails.
/// # Examples
/// ```
/// # fn example(report: &velo_contracts::reports::ComparisonReport) -> Result<(), velo_contracts::CommandError> {
/// let json = velo_contracts::reports::export_json(report)?;
/// assert!(json.contains("format_version"));
/// # Ok(()) }
/// ```
pub fn export_json(report: &ComparisonReport) -> Result<String, CommandError> {
    serde_json::to_string_pretty(report).map_err(|_| CommandError {
        code: "REPORT_ENCODING_FAILED".into(),
        message: "The report could not be encoded.".into(),
        field: None,
    })
}

fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|character| matches!(character, '\n' | '\r' | '\t' | ' '..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))
        .fold(String::new(), |mut output, character| {
            match character {
                '&' => output.push_str("&amp;"),
                '<' => output.push_str("&lt;"),
                '>' => output.push_str("&gt;"),
                '"' => output.push_str("&quot;"),
                '\'' => output.push_str("&apos;"),
                _ => output.push(character),
            }
            output
        })
}

fn metric_label(metric: super::RegressionMetric) -> &'static str {
    use super::RegressionMetric;
    match metric {
        RegressionMetric::ResponseP50Ms => "Response p50 (ms)",
        RegressionMetric::ResponseP95Ms => "Response p95 (ms)",
        RegressionMetric::ResponseP99Ms => "Response p99 (ms)",
        RegressionMetric::RequestRate => "Throughput (requests/s)",
        RegressionMetric::HttpFailurePercent => "HTTP failure (%)",
        RegressionMetric::AssertionFailurePercent => "Assertion failure (%)",
    }
}

fn measurement_charts(report: &ComparisonReport) -> String {
    let mut charts = String::new();
    for delta in &report.deltas {
        let baseline = delta
            .baseline
            .filter(|value| value.is_finite() && *value >= 0.0);
        let candidate = delta
            .candidate
            .filter(|value| value.is_finite() && *value >= 0.0);
        let maximum = baseline.unwrap_or(0.0).max(candidate.unwrap_or(0.0));
        charts.push_str(&format!(
            "<figure><figcaption>{}</figcaption>",
            metric_label(delta.metric)
        ));
        for (label, value, color) in [
            ("Baseline", baseline, "#0284c7"),
            ("Candidate", candidate, "#d97706"),
        ] {
            let width = value.map(|value| {
                if maximum == 0.0 {
                    0.0
                } else {
                    value / maximum * 100.0
                }
            });
            let bar = width
                .map(|width| {
                    format!("<div style=\"width:{width}%;height:100%;background:{color}\"></div>")
                })
                .unwrap_or_default();
            charts.push_str(&format!("<div class=\"bar-label\"><span>{label}</span><span>{}</span></div><div class=\"bar-track\" role=\"img\" aria-label=\"{label}: {}; {}\">{bar}</div>",
                display(value), display(value), metric_label(delta.metric)));
        }
        let maximum_label = if baseline.is_none() && candidate.is_none() {
            "No observations".into()
        } else {
            maximum.to_string()
        };
        charts.push_str(&format!(
            "<div class=\"bar-label\"><span>0</span><span>{maximum_label}</span></div></figure>"
        ));
    }
    charts
}

fn run_context(entry: &crate::HistoryEntry, label: &str) -> Result<String, CommandError> {
    let context = serde_json::json!({
        "id": entry.id, "created_at_unix_seconds": entry.created_at, "method": entry.method,
        "definition_identity": entry.definition_identity, "release": entry.metadata,
        "state": entry.snapshot["state"], "configuration": entry.configuration,
        "target_and_workload": entry.snapshot["confirmation"], "measurement": entry.snapshot["measurement"],
        "interrupted": entry.interrupted, "incomplete": entry.incomplete, "sampled_interval_stride": entry.stride,
        "retained_intervals": entry.points.len(),
    });
    let source = serde_json::to_string_pretty(&context).map_err(|_| CommandError {
        code: "REPORT_ENCODING_FAILED".into(),
        message: "The report could not be encoded.".into(),
        field: None,
    })?;
    Ok(format!(
        "<h2>{label} context</h2><pre>{}</pre>",
        escape(&source)
    ))
}

/// Renders a standalone HTML report with embedded readable evidence and no external resources.
///
/// # Arguments
/// * `report` - Redacted comparison including optional scenario, diagnostic, and host evidence.
/// # Returns
/// Escaped HTML containing separately scaled charts, deltas, budgets, limitations,
/// and complete safe JSON evidence. Charts distinguish missing observations from measured zero.
/// # Errors
/// Returns `REPORT_ENCODING_FAILED` if evidence serialization fails.
/// # Examples
/// ```
/// # fn example(report: &velo_contracts::reports::ComparisonReport) -> Result<(), velo_contracts::CommandError> {
/// assert!(velo_contracts::reports::export_html(report)?.starts_with("<!doctype html>"));
/// # Ok(()) }
/// ```
pub fn export_html(report: &ComparisonReport) -> Result<String, CommandError> {
    let evidence = escape(&export_json(report)?);
    let charts = measurement_charts(report);
    let mut rows = String::new();
    for delta in &report.deltas {
        rows.push_str(&format!(
            "<tr><th>{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            metric_label(delta.metric),
            display(delta.baseline),
            display(delta.candidate),
            display(delta.absolute),
            display(delta.percentage)
        ));
    }
    let mut warnings = String::new();
    for limitation in &report.limitations {
        warnings.push_str(&format!("<li>{}</li>", escape(limitation)));
    }
    let mut budgets = String::new();
    for result in &report.regressions {
        budgets.push_str(&format!(
            "<tr><th>{}</th><td>{}</td><td>{}</td><td>{}</td><td>{:?}</td></tr>",
            metric_label(result.budget.metric),
            result.budget.limit,
            if result.budget.percentage {
                "relative %"
            } else {
                "absolute metric units"
            },
            display(result.worsening),
            result.verdict
        ));
    }
    let mut differences = String::new();
    for difference in &report.differences {
        differences.push_str(&format!(
            "<tr><th>{}</th><td><pre>{}</pre></td><td><pre>{}</pre></td></tr>",
            escape(&difference.field),
            escape(&difference.baseline.to_string()),
            escape(&difference.candidate.to_string())
        ));
    }
    let baseline_context = report
        .baseline
        .as_ref()
        .map(|entry| run_context(entry, "Baseline"))
        .transpose()?
        .unwrap_or_else(|| "<h2>Baseline unavailable</h2>".into());
    let candidate_context = run_context(&report.candidate, "Candidate")?;
    Ok(format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'"><title>Velo regression report</title><style>body{{font:16px system-ui;max-width:1000px;margin:3rem auto;padding:1rem;color:#18202b;background:#fff}}table{{border-collapse:collapse;width:100%}}th,td{{text-align:left;padding:.6rem;border:1px solid #ccc}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;background:#f3f4f6;padding:1rem}}h1,h2{{line-height:1.2}}.charts{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,280px),1fr));gap:1rem}}figure{{min-width:0;margin:0;padding:1rem;border:1px solid #ccc;border-radius:6px}}figcaption{{font-weight:600;margin-bottom:1rem}}.bar-label{{display:flex;flex-wrap:wrap;justify-content:space-between;gap:.5rem;font-size:13px;overflow-wrap:anywhere;margin:.5rem 0}}.bar-track{{height:12px;background:#e2e8f0;border-radius:2px;overflow:hidden}}</style></head>
<body><h1>Velo regression report</h1><p>Format 1 · regression verdict: {:?}</p><p>Baseline: {} · candidate: {}</p>
<p>Latency is milliseconds; throughput is requests/s; failure metrics are percent. Absolute change is candidate minus baseline (percentage points for failure rates). Relative change is 100 × absolute / baseline. Unavailable percentages include zero baselines. Lower latency and failures are better; higher throughput is better. Budgets use worsening, including negative changes for improvements. No statistical significance is claimed.</p>
<h2>Limitations</h2><ul>{warnings}</ul><p>Request paths, credentials, payloads, and dataset rows are withheld. Safe definition identity cannot prove identical omitted request content. Review release notes and configuration before accepting conclusions. Summaries remain until deletion; intervals retain at most 600 samples and the reported stride. Missing intervals are never reconstructed. Optional evidence remains absent when unavailable.</p>
{baseline_context}{candidate_context}
<h2>Configuration differences</h2><table><thead><tr><th>Field</th><th>Baseline</th><th>Candidate</th></tr></thead><tbody>{differences}</tbody></table>
<h2>Regression budgets</h2><p>Configuration review acknowledged: {}. No budgets means unevaluable.</p><table><thead><tr><th>Metric</th><th>Limit</th><th>Unit</th><th>Worsening</th><th>Verdict</th></tr></thead><tbody>{budgets}</tbody></table>
<h2>Evaluated measurement charts</h2><p>Each metric uses its own zero-based scale. Baseline is blue; candidate is amber. Values are labeled without rounding. Zero has no visible bar; missing observations are unavailable. Bars show measured values, not signed percentage changes.</p><div class="charts">{charts}</div>
<h2>Evaluated measurements</h2><table><thead><tr><th>Metric</th><th>Baseline</th><th>Candidate</th><th>Absolute change</th><th>Relative change %</th></tr></thead><tbody>{rows}</tbody></table>
<h2>Configuration, verdicts, and retained evidence</h2><p>The evidence below includes release metadata, definition identity, configuration differences, measurement windows, counts, separate latency populations, thresholds, regression budgets, scenario outcomes, diagnostics, and generator health. Large counters remain decimal strings. JUnit cases represent aggregate rules and configured assertions, never individual HTTP requests.</p><pre>{evidence}</pre></body></html>"#,
        report.verdict,
        escape(&report.baseline_id),
        escape(&report.candidate.id),
        report.configuration_reviewed
    ))
}

fn display(value: Option<f64>) -> String {
    value.map_or_else(|| "unavailable".into(), |value| value.to_string())
}

fn case(name: &str, verdict: RegressionVerdict, evidence: &str) -> String {
    let outcome = match verdict {
        RegressionVerdict::Pass => String::new(),
        RegressionVerdict::Fail => format!(
            "<failure message=\"Limit exceeded\">{}</failure>",
            escape(evidence)
        ),
        RegressionVerdict::Unevaluable => format!(
            "<skipped message=\"Unevaluable\">{}</skipped>",
            escape(evidence)
        ),
    };
    format!(
        "<testcase classname=\"velo.aggregate\" name=\"{}\">{outcome}</testcase>",
        escape(name)
    )
}

/// Exports aggregate rule cases as JUnit XML, never one case per HTTP request.
///
/// # Arguments
/// * `report` - Comparison evidence with separate absolute and regression verdicts.
/// # Returns
/// XML with run and regression summary cases, budgets, candidate thresholds, and assertions.
/// Unevaluable or incomplete evidence becomes a skipped case; exceeded limits become failures.
/// Full safe evidence is included as escaped JSON in `system-out`.
/// # Errors
/// Returns `REPORT_ENCODING_FAILED` if evidence serialization fails.
/// # Examples
/// ```
/// # fn example(report: &velo_contracts::reports::ComparisonReport) -> Result<(), velo_contracts::CommandError> {
/// assert!(velo_contracts::reports::export_junit(report)?.contains("testsuite"));
/// # Ok(()) }
/// ```
pub fn export_junit(report: &ComparisonReport) -> Result<String, CommandError> {
    let evidence = escape(&export_json(report)?);
    let mut cases = vec![("run outcome".to_owned(), super::run_verdict(&report.candidate), "Native completion, measurement integrity, thresholds, assertions, and scenario outcomes.".to_owned()), (
        "regression summary".to_owned(),
        report.verdict,
        report.limitations.join("; "),
    )];
    for (index, result) in report.regressions.iter().enumerate() {
        cases.push((
            format!("regression {index}: {:?}", result.budget.metric),
            result.verdict,
            format!(
                "worsening={} limit={} percentage={}",
                display(result.worsening),
                result.budget.limit,
                result.budget.percentage
            ),
        ));
    }
    let mut incomplete = Vec::new();
    limitations(&report.candidate, "Candidate", &mut incomplete);
    if let Some(thresholds) = report.candidate.snapshot["thresholds"].as_array() {
        for (index, threshold) in thresholds.iter().enumerate() {
            let verdict = if !incomplete.is_empty() {
                RegressionVerdict::Unevaluable
            } else {
                match threshold["status"].as_str() {
                    Some("passed") => RegressionVerdict::Pass,
                    Some("failed") => RegressionVerdict::Fail,
                    _ => RegressionVerdict::Unevaluable,
                }
            };
            cases.push((format!("threshold {index}"), verdict, threshold.to_string()));
        }
    }
    if let Some(assertions) = report
        .candidate
        .snapshot
        .pointer("/evaluated/diagnostics/assertions")
        .and_then(Value::as_array)
    {
        for (index, assertion) in assertions.iter().enumerate() {
            let parsed = ["passed", "failed", "unavailable"].map(|field| {
                assertion[field]
                    .as_str()
                    .and_then(|value| value.parse::<u64>().ok())
            });
            let verdict = match parsed {
                [Some(_), Some(failed), Some(_)] if failed > 0 => RegressionVerdict::Fail,
                [Some(passed), Some(0), Some(0)] if passed > 0 && incomplete.is_empty() => {
                    RegressionVerdict::Pass
                }
                _ => RegressionVerdict::Unevaluable,
            };
            cases.push((
                format!(
                    "assertion {index}: {}",
                    assertion["id"].as_str().unwrap_or("unnamed")
                ),
                verdict,
                assertion.to_string(),
            ));
        }
    }
    let failures = cases
        .iter()
        .filter(|(_, verdict, _)| *verdict == RegressionVerdict::Fail)
        .count();
    let skipped = cases
        .iter()
        .filter(|(_, verdict, _)| *verdict == RegressionVerdict::Unevaluable)
        .count();
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuites><testsuite name=\"Velo aggregate report v1\" tests=\"{}\" failures=\"{failures}\" skipped=\"{skipped}\">",
        cases.len()
    );
    for (name, verdict, evidence) in cases {
        xml.push_str(&case(&name, verdict, &evidence));
    }
    xml.push_str(&format!(
        "<system-out>{evidence}</system-out></testsuite></testsuites>"
    ));
    Ok(xml)
}
