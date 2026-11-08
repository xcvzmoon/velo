# Compare a release against a baseline

Use Results → Run history to pin a reference run, select a candidate, review configuration differences, and export the evidence. Runs must share an origin, HTTP method, and workload kind. Matching these fields does not establish identical request content or target conditions.

## Create a comparison

1. Finish a run and choose **Pin baseline** in history. Pinning another run replaces the reference; **Remove baseline** removes the pin without deleting either run.
2. Select a candidate. The last selected run owns the release context and comparison settings. The two existing history selections still show their summaries and retained charts.
3. Add a Git SHA, release label, comma-separated tags, or notes, then save the release context. These fields do not change measurements. They are stored locally and included in exports; enter only information safe to share.
4. Review the configuration differences and measurement limitations. Add regression budgets and choose absolute metric units or relative percent for each limit.
5. Change **Configuration review** to the reviewed option, then choose **Save and evaluate budgets**.

A saved comparison retains its original baseline ID. To replace it with a newly pinned reference, choose **Use pinned baseline**. This resets the review acknowledgement while retaining the budgets. Removing a pin does not rewrite saved comparisons. Deleting the source baseline makes dependent comparisons unevaluable; Velo never substitutes another run. Refresh history to reload retained evidence after changes.

## Interpret the measurements

Comparison charts show labeled baseline and candidate values on a shared zero-based scale within each metric. Each metric has its own scale and unit; bar lengths across different metrics are not comparable. Zero has a zero-width bar; missing observations remain unavailable. Charts and tables retain unrounded measurements.

All deltas use the evaluated measurement population after warm-up, including its drain. Reports preserve the full-run population separately in the original snapshot. Latency describes completed HTTP responses, including status failures. Transport, timeout, and cancellation durations do not enter that response population. HDR percentiles remain approximate; subtracting them does not imply statistical significance.

| Metric                 | Absolute unit                           | Worsening direction |
| ---------------------- | --------------------------------------- | ------------------- |
| Response p50, p95, p99 | Milliseconds                            | Increase            |
| Request throughput     | Completed non-cancelled requests/second | Decrease            |
| HTTP failure           | Percentage points                       | Increase            |
| Assertion failure      | Percentage points                       | Increase            |

Absolute change is `candidate − baseline`. Relative percent is `100 × absolute change / baseline`. A zero or missing baseline produces an unavailable relative percentage, including when both values are zero. Absolute comparisons against zero remain available when both measurements exist. Velo uses the retained unrounded measurements for calculations and budget verdicts. Large counters remain decimal strings; converting them to JavaScript `number` can lose precision.

A budget limits worsening. For throughput, Velo negates the change; for latency and failure rates, it uses the change directly. Improvements can therefore have negative worsening. Equality passes. For example, throughput falling from 100 to 90 requests/s is a 10% worsening and passes a 10% budget. An HTTP failure increase from 1% to 2% is one percentage point in absolute units and 100% in relative units.

Each budget returns `pass`, `fail`, or `unevaluable`. The combined regression verdict fails if any evaluable budget fails; otherwise it is unevaluable if any budget is unevaluable or no budgets exist. It passes only when every configured budget is evaluable and satisfied. Absolute thresholds retain their original verdicts, even when a regression budget passes. Comparison settings and regression results are stored separately from measured snapshots and threshold results.

## Review configuration and limitations

Reports compare retained origin, workload, duration, warm-up, resource budgets, dataset settings, live abort policies, and allowlisted execution settings. Those settings include HTTP protocol, redirect limit, timeout, pacing, drain grace, thresholds, inspection limits, assertion IDs, and scenario IDs, ordering, weights, seed, and think times. Dataset allocation counts, cycles, and exhaustion are outcome evidence, rather than configuration differences.

New runs have a `safe-v1:` identity: SHA-256 of the retained safe execution settings. This identifies the retained configuration, not the complete authored document. URL paths and queries, headers, credentials, payloads, assertion expectations, extraction bindings, and dataset rows are omitted from this identity and from reports. Matching identities cannot prove identical omitted content. Review that context yourself before acknowledging the comparison. Earlier runs without this evidence remain identifiable but cannot produce a clean regression pass.

Interrupted, cancelled, aborted, failed, active, incomplete-history, or incomplete-measurement runs cannot pass regression budgets. Counter overflow, aggregation interference, out-of-range latency samples, arrival capacity drops, missed deadlines, and dataset exhaustion also prevent a clean pass. Missing populations leave their metrics unavailable. Optional scenario, diagnostic, and native CPU/RAM evidence stays absent or explicitly unavailable when its source is absent.

Summaries remain until deletion. Each run retains at most 600 sampled intervals; the exported `stride` records compaction. An `incomplete` flag records missing history. Retained points are samples, not merged buckets, and exports do not reconstruct omitted intervals. Generator telemetry has its own sample window and does not establish target-server health.

## Export the evidence

Choose an export action and a destination in the native save dialog. Cancelling creates no file. Writes use a temporary file in the destination directory and replace the destination only after writing and syncing the complete content. Save errors return `REPORT_EXPORT_FAILED` without exposing native paths.

- **Readable report** saves standalone HTML. It includes separately scaled baseline/candidate charts, measurement directionality, limits, configuration differences, release context, and retained evidence without remote resources or scripts. User-authored text is escaped.
- **JSON v1** saves the shared `ComparisonReport` format with `format_version: 1`. It contains the requested baseline ID, nullable baseline evidence, candidate evidence, review acknowledgement, differences, limitations, metric deltas, budget results, and combined regression verdict. Counts and native durations retain their decimal-string representation. Missing values are `null`, not invented zeroes.
- **JUnit** saves XML aggregate cases. There is one regression summary case, one case per budget, one per candidate absolute threshold, and one per configured standalone assertion in the evaluated diagnostics. Failures use `<failure>`; unevaluable cases use `<skipped>`. Assertion totals with known failures fail; zero observations or unavailable observations cannot pass. Incomplete or integrity-limited candidate evidence cannot produce clean threshold or assertion passes. Scenario checks remain in the retained scenario evidence and are not presented as per-request cases. Escaped JSON in `system-out` preserves the full report context. No exporter claims that individual HTTP requests are stored test cases.

JSON and HTML preserve both runs' threshold results, measurement windows, counts, latency populations, scenario outcomes, bounded diagnostics, generator evidence, and retention indicators. JUnit presents aggregate verdicts as cases and includes full evidence in `system-out`; JSON or HTML is easier to read directly.

The shared Rust API lives in `velo_contracts::reports`: `compare`, `export_json`, `export_html`, and `export_junit`. Desktop commands read authoritative SQLite history before exporting. Future CLI callers can reuse the same calculations and exporters without depending on Tauri. Exporters accept safe history evidence; request definitions and raw responses do not belong in that input.
