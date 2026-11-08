# Execution and measurements

The independent engine validates GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS requests, custom headers, query pairs, and text, JSON, URL-encoded, or multipart bodies. GET and HEAD reject request bodies. Each VU executes sequential requests through a shared keep-alive pool. Timeout defaults to 30 seconds, pacing to zero, and drain grace to five seconds. Redirects default to disabled; automatic retries, proxies, and automatic response decompression are disabled. Standalone requests do not retain cookies; [scenarios](authoring.md#build-a-multi-step-scenario) keep isolated per-user cookie state. TLS uses system certificate validation.

Bearer and Basic credentials are excluded from run reports. They stay in memory unless you explicitly remember a request in the OS credential store. Basic authentication is encoded, not encrypted; use HTTPS to protect credentials in transit. Dedicated authentication cannot be combined with a custom Authorization header. HTTP defaults to normal negotiation, with explicit HTTP/1.1-only and HTTP/2-only modes. Cleartext HTTP/2 requires prior knowledge; forced HTTP/2 never falls back to HTTP/1.1.

The redirect limit defaults to zero. Enabled redirects follow only the original origin and reject HTTPS downgrades. This keeps credentials, custom headers, and request bodies on the authorized target. POST changes to GET on 301/302; 303 changes to GET except for HEAD; 307/308 preserve method and body. Redirect bodies contribute to response-byte totals, while counters still count one logical request. The full-request timeout covers all hops and upload setup together.

URL-encoded and multipart fields retain duplicate names and order. Choose multipart files through the native selector; edit the transmitted filename and MIME type separately. Files must remain readable and unchanged during a run. Uploads stream through bounded channels with two 16-KiB buffers per file per concurrent VU charged to the request-data budget. Disk content is not loaded into memory as a whole. Readers are cancelled and joined before a terminal state is published; body-preserving redirects reopen files. File paths, credentials, and body content are excluded from run reports. Body-byte metrics count received response payloads, including redirects, and exclude uploaded bytes. File contents are never persisted; explicitly remembered requests retain file paths in the OS credential store, and the files must still exist when loaded.

Configure final thresholds in the request editor using strict less-than or greater-than comparisons. Limits use milliseconds for response p95, percent for HTTP failures, and requests per second for throughput. Evaluation uses final cumulative measurements after cleanup, including load drain in throughput duration and excluding scenario setup and cleanup time. Equality fails. Empty populations, counter overflow, and incomplete latency histograms cannot pass. Cancelled and failed runs retain their partial context; threshold failures do not alter HTTP outcomes or execution state. Response percentiles remain approximate HDR measurements, and comparisons use unrounded native values.

Native commands are `prepare_run`, `start_run`, `stop_run`, and `current_run`. Prepare returns a safe target origin, requested workload, explicit budgets, and high-load warning codes without HTTP traffic. Start requires `authorized: true`, returns a run ID immediately, and rejects overlapping runs. Stop requires that ID and closes admission before drain. Current returns `null` before the first run, then the latest current/final summary. A new accepted run replaces the previous summary. The editor prepares a safe workload confirmation before starting. Application exit asks for confirmation during an active run, blocks new starts, and awaits cleanup before closing.

Workloads use a tagged `workload` configuration. Constant workloads contain `kind: 'constant'`, `vus`, and `duration_ms`. Ramping workloads contain `kind: 'ramping'`, `start_vus`, and ordered `stages` with `duration_ms` and `target_vus`. Each stage duration is positive; ramp counts may be zero. Total scheduling duration must fit 4,294,967,295 milliseconds.

Stages interpolate whole VUs from the preceding count toward the target using monotonic deadlines. Counts round toward the starting count until the next whole-VU transition; stage endpoints are exact. A repeated target holds that level. Runtime scheduling can delay transitions; missed transitions are reconciled to the current desired count without replaying them. Zero-VU stages keep the schedule alive without sending requests.

Ramp-down retires users after their current request or complete scenario iteration finishes. Retirement interrupts pacing and prevents another request, without cancelling the response. Retiring workers occupy slots until joined, so a later ramp-up may wait for slow retirees. Desired, actual, and retiring VUs are reported separately. Actual VUs never exceed the workload's peak allocation. Stop and duration expiry still apply the configured drain grace, then abort and join remaining work.

Constant arrival rate schedules one iteration per evenly spaced monotonic deadline, beginning at zero and excluding the duration endpoint. Set a positive integer rate, duration, and concurrent request capacity within the worker budget. Pacing must be zero. Each admitted task performs one standalone request or a complete selected scenario through the shared pool. A multi-step iteration owns one capacity slot until it finishes; its steps execute sequentially. Completed tasks are joined before admitting more work; the scheduler retains no pending-request queue.

Delayed scheduler wakes skip expired slots in one calculation and offer at most the current slot. There are no catch-up batches. Offered slots include started iterations, capacity drops, dataset drops, and missed deadlines. The live view and final summary show each count and maximum scheduler lateness in microseconds. Drops indicate that the generator could not offer the requested load; they do not enter HTTP failure rate. Offered slot rate uses scheduling time, while achieved request throughput includes drain. Tokio timers and OS scheduling are best effort; sub-millisecond spacing is not a timing guarantee. No measured rate capacity is claimed. Rates at or above 1,000 iterations/s prompt a high-load warning.

The default budgets allow 256 worker tasks and 8 MiB of request data across the normalized template and VU clones. Request-body sharing is conservatively budgeted as a copy per VU. Stage configuration and the compiled schedule also count against the byte budget. Both budgets are adjustable; exceeding either rejects the run before worker creation. A warning applies at 100 VUs. These defaults are allocation safeguards, not capacity claims or a process-wide memory limit. Transport, TLS, runtime, and allocator overhead consume additional memory.

Responses are streamed and discarded. Body-byte totals count encoded application bytes actually consumed, including partial bodies from failures and cancellations. They exclude headers, TLS, retransmissions, and unread payloads. Standalone iterations have one logical request; scenario iterations can have several. Redirect hops remain part of one logical request. Final HTTP status 200–399 succeeds only after the body is fully consumed. Status failures, transport failures, timeouts, and cancellations remain separate.

Three fixed cumulative HDR histograms measure completed responses, transport failures, and timeouts. A fourth fixed histogram measures interval response latency and resets after each aggregate update. Response latency includes connection acquisition and body reading, excludes aggregation, and includes status failures. Histograms use microseconds and three significant digits over 1 μs–1 hour. Statistics are approximate; empty populations return `null`, and out-of-range samples are counted explicitly. There is no per-request history or observation queue. Metrics lock waits over one millisecond flag aggregation interference.

Counters reconcile as `started = responses + transport_failures + timeouts + cancelled + in_flight`. Terminal states have zero in-flight requests and zero owned workers. HTTP failure rate excludes cancellations; request throughput divides non-cancelled terminal attempts by scheduling plus drain time. Cancelled runs retain partial results. Counters and microsecond durations serialize as decimal strings, preserving u64 precision; percentile values fit safely in JavaScript numbers. Chart consumers check range before converting interval counters; cumulative counters remain decimal strings.

Local-server tests cover construction, connection reuse, streaming, encoded and partial bytes, status/transport/timeout classification, pacing, duration, stop races, resource validation, repeated runs, and stalled-target cleanup. A Tauri test runtime exercises actual command dispatch and serialization without a webview or external target. No sustained-load capacity benchmark or Windows/Linux verification is claimed.

## Run and monitor

1. Start a local target, such as `python3 -m http.server 8080 --bind 127.0.0.1` in a directory containing only test content.
2. Open Tests. Enter the target URL, method, optional headers/query pairs, and POST body. Choose constant VUs and duration, ramping VUs with ordered stages, or constant arrival rate with iterations/second and concurrent request capacity. Arrival workloads require zero pacing. Set timeout, pacing, grace, and local resource budgets.
3. Select Review run. Check the target origin, load, budgets, and high-load warnings, then acknowledge authorization to start.
4. Monitor interval request rate, response p95, HTTP failure rate, and desired/actual VUs in Tests, Dashboard, or Results. Stop closes admission and drains requests before publishing partial results.
5. Open Results for the final counters, duration breakdown, throughput, failure fraction, and separate response/transport/timeout distributions.

Aggregate `run-update` events carry a run ID, decimal sequence, monotonic interval duration, cumulative state, interval outcome counts, desired/actual/retiring VUs, and interval response histogram. `start_run` accepts optional `updateIntervalMs` (250–10,000 ms; default 1,000). The UI offers 500 ms, 1 s, and 2 s. One native publisher owns each run; missed timer ticks do not produce catch-up bursts. Emission failures do not fail HTTP execution.

An application-scoped subscription preserves monitoring across navigation. Recovery reads run every two seconds to restore cumulative totals and final state after missed events. Charts retain at most 600 points, show gaps for missing intervals, and do not reconstruct lost percentiles. Scenario runs also plot cumulative iteration throughput separately from interval request throughput. Scenario assertion charts use the aggregate step check rate even when there are no standalone assertions. Stored charts omit zero-duration lifecycle updates and duplicate elapsed timestamps. Empty response populations have no latency value. Charts and resize observers are destroyed on route teardown; the event listener is removed on application teardown.

This milestone adds [uPlot](https://github.com/leeoniya/uPlot) for charts and the Rust [Tauri dialog plugin](https://v2.tauri.app/plugin/dialog/) for native exit confirmation. Dialogs are called from Rust; no frontend dialog permissions are granted. macOS uses the existing Objective-C runtime crates directly to route AppKit termination requests, including Dock Quit and Command-Q, through the same confirmation and cleanup path. This is required because the current Tauri macOS delegate does not intercept AppKit termination.

## Generated contracts

```sh
pnpm bindings:generate
pnpm bindings:check
```

The generator uses `ts-rs` declarations from Rust and runs through `cargo run --locked -p velo-contracts --example export-bindings`. Commit generated contract updates alongside the Rust types that require them. Do not edit `app/types/contracts.ts` manually. CI checks for drift. Nuxt exposes native contract types through `#imports`.

TypeScript and Vue component tests live under `tests/`, mirroring the application directories.

## Response assertions and failure diagnostics

Add checks under **Response assertions and diagnostics** in the visual editor, or set `config.assertions` in a portable definition. Preview sends one logical request and shows each check's result in configuration order. Review validates all definitions before load admission.

```json
{
  "assertions": [
    { "id": "status", "rule": { "kind": "status", "allowed": [200, 201] } },
    { "id": "content", "rule": { "kind": "header_present", "name": "content-type" } },
    {
      "id": "application-ok",
      "rule": { "kind": "json_equals", "pointer": "/ok", "value": "true" }
    },
    { "id": "message", "rule": { "kind": "text_contains", "value": "ready" } }
  ],
  "inspection": { "body_bytes": 16384, "sample_count": 16, "sample_bytes": 8192 },
  "thresholds": [{ "metric": "assertion_failure_percent", "operator": "less_than", "limit": 1 }]
}
```

The fragment belongs inside `config`; keep the request and workload fields from your definition. Assertions use literal expectations. Environment and dataset substitution applies to request fields, not assertion definitions.

| Rule             | Condition                                                                                               |
| ---------------- | ------------------------------------------------------------------------------------------------------- |
| `status`         | Final status matches one of 1–100 allowed codes in 100–599.                                             |
| `header_present` | Case-insensitive `name` exists, including an empty value.                                               |
| `header_equals`  | Any occurrence of `name` exactly equals `value`.                                                        |
| `json_present`   | RFC 6901 `pointer` exists; JSON null counts as present.                                                 |
| `json_equals`    | Field equals the parsed JSON in `value`, including its type. Use `"\"ready\""` to expect a JSON string. |
| `text_contains`  | Complete UTF-8 body contains the nonempty literal `value`.                                              |
| `text_equals`    | Complete UTF-8 body exactly matches `value`, including whitespace.                                      |

Use at most 32 assertions with unique IDs of 1–64 ASCII letters, digits, underscores or hyphens. Header names are limited to 256 bytes, JSON pointers to 1,024 bytes, and expectations to 4,096 bytes. An empty JSON pointer selects the whole document. Escape `~` as `~0` and `/` as `~1`. Equality expectations are parsed once during preparation; each response JSON body is parsed at most once, using serde_json's default nesting limit.

### Accounting

A complete 200 response with an incorrect payload increments HTTP successes and the failed assertion's counter. It does not increment HTTP status failures. Multiple failed assertions on that response each contribute one failed check. HTTP failure percentage retains its status/transport/timeout meaning. Scenario failed iterations also include assertion, binding, and required extraction failures. Standalone failed-iteration accounting keeps its HTTP meaning.

For each assertion, `passed + failed + unavailable` equals terminal logical attempts, including cancellations. Missing headers or fields and mismatched values fail. Invalid JSON, invalid UTF-8, unsupported content encoding and truncated input are unavailable/error outcomes. A transport failure, timeout or cancellation makes every check unavailable, even when response headers arrived. In-flight attempts contribute no check totals yet.

Live results and stored history plot cumulative assertion failure percentage separately from interval HTTP failure percentage. Assertion charts use a 0–100% axis. Unavailable checks, counter overflow and missed live events leave gaps; incomplete stored histories hide charts.

`assertion_failure_percent` is `100 × failed checks / (passed checks + failed checks)` across configured assertions. If any check is unavailable, or no checks have been evaluated, the threshold reports `insufficient_data`. Equality with a strict threshold limit fails, as with other thresholds. HTTP failure percentage is unchanged.

### Bounds, latency and retention

Body inspection defaults to 16 KiB per worker and accepts 1–65,536 bytes. Body checks share the transport reader used by preview; there is no observation queue or second request. Load execution retains a bounded prefix, drains and discards excess bytes, and marks all body checks unavailable on truncation, including text matches found in that prefix. Status and header checks still run on a fully consumed oversized response. Without body assertions, load execution does not retain a body buffer. Across peak workers, configured inspection buffers must fit the 16 MiB run limit; reduce peak workers or the byte cap when preparation reports `INSPECTION_BUDGET_EXCEEDED`. Parsing and transport allocations are additional to these raw capture buffers and remain bounded by the response prefix and existing transport limits.

The existing deadline covers request setup, redirects, body consumption and assertion evaluation. Streaming reads remain cancellable. Parsing and literal comparisons execute synchronously on the bounded input before the next cancellation point. Response latency includes this processing and upload cleanup; enabling checks can reduce measured throughput and increase latency. JSON parsing costs scale with inspected body size; literal checks cost up to 32 scans of the bounded body. Assertions do not decompress encoded responses. Preview still captures at most 64 KiB of body and 16 KiB of headers, independently of the smaller assertion inspection cap.

Failure groups use stable cause, assertion index and HTTP status, capped at 256 groups. Excess group occurrences remain visible in `dropped_groups`. Samples keep the first representative metadata for each new group, with defaults of 16 samples and 8 KiB of serialized sample data per run. Hard maxima are 32 samples and 32 KiB. Setting either sample limit to zero disables samples; aggregate counts and grouping continue. `dropped_samples`, retained bytes and truncated-response counts are visible during and after execution.

Samples contain only HTTP status, assertion index and a stable cause. All response bodies, header values, URLs, credentials, configured sensitive fields, and actual/expected assertion values are withheld before native IPC or persistence. This conservative policy also prevents partial or encoded secrets from escaping. IDs are user-authored labels and should not contain secrets. Single-request preview remains a separate, explicitly authorized inspection tool with its existing redaction policy.

The latest run summary stores assertion aggregates, groups and retained samples. Interval points keep assertion aggregates without duplicating groups or samples. Samples follow the run's retention: they remain until the run is deleted; deletion removes the summary and cascades to interval points. There is no automatic expiry. See [history](history.md) for storage failure and deletion behavior.
