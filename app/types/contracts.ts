export type DiagnosticInput = { label: string };
export type DiagnosticReport = { engine: string; version: string };
export type CommandError = { code: string; message: string; field: string | null };
export type NameValue = {
  /**
   * Header or query parameter name.
   */
  name: string;
  /**
   * Header or query parameter value; never included in run reports.
   */
  value: string;
};
export type RequestMethod = 'GET' | 'POST';
export type RequestBody =
  | { kind: 'none' }
  | { kind: 'text'; value: string }
  | { kind: 'json'; value: string };
export type ResourceBudget = {
  /**
   * Maximum number of simultaneously owned VU tasks; must be positive.
   */
  max_vus: number;
  /**
   * Maximum normalized request bytes across the template and VU clones,
   * including per-pair overhead. This is not a process-wide memory limit.
   */
  max_request_bytes: number;
};
export type RunConfig = {
  /**
   * Absolute HTTP(S) URL without embedded credentials or a fragment.
   */
  url: string;
  /**
   * GET or POST.
   */
  method: RequestMethod;
  /**
   * Custom headers; Host, Content-Length, and Transfer-Encoding are reserved.
   */
  headers: NameValue[];
  /**
   * Additional query pairs appended to the URL, including duplicate names.
   */
  query: NameValue[];
  /**
   * Optional POST body.
   */
  body: RequestBody;
  /**
   * Sequential workers sharing one connection pool; must be positive.
   */
  vus: number;
  /**
   * Scheduling duration; must be positive.
   */
  duration_ms: number;
  /**
   * Full response timeout, including streamed body; 1–3,600,000 ms.
   */
  timeout_ms: number;
  /**
   * Delay after an iteration finishes, before another starts; zero by default.
   */
  pacing_ms: number;
  /**
   * Maximum drain time after stop or duration expiry; zero aborts immediately.
   */
  grace_ms: number;
  /**
   * Explicit task and retained request-data limits.
   */
  budget: ResourceBudget;
};
export type RunError = {
  /**
   * Stable machine-readable code.
   */
  code: string;
  /**
   * Safe explanation suitable for display.
   */
  message: string;
  /**
   * Invalid configuration field, when applicable.
   */
  field: string | null;
};
export type RunConfirmation = {
  /**
   * Target scheme, host, and port; excludes path, credentials, and query.
   */
  origin: string;
  /**
   * Requested workers.
   */
  vus: number;
  /**
   * Requested scheduling duration in milliseconds.
   */
  duration_ms: number;
  /**
   * Explicit allocation budgets.
   */
  budget: ResourceBudget;
  /**
   * Stable warning codes. HIGH_VU_COUNT applies at 100 or more VUs.
   */
  warnings: string[];
};
export type RunState = 'starting' | 'running' | 'stopping' | 'completed' | 'cancelled' | 'failed';
export type Counters = {
  /**
   * Admitted logical requests.
   */
  started: string;
  /**
   * Fully consumed HTTP responses, including status failures.
   */
  responses: string;
  /**
   * Fully consumed responses with status 200–399.
   */
  successes: string;
  /**
   * Fully consumed responses with other statuses.
   */
  status_failures: string;
  /**
   * Failed connection, protocol, or response body operations, excluding timeouts.
   */
  transport_failures: string;
  /**
   * Requests whose complete-response deadline expired.
   */
  timeouts: string;
  /**
   * Requests cancelled after the drain grace expired.
   */
  cancelled: string;
  /**
   * Requests still executing; zero in terminal states.
   */
  in_flight: string;
  /**
   * Encoded response body bytes consumed, including bytes from failed attempts.
   */
  response_body_bytes: string;
  /**
   * Completed non-cancelled iterations; one request per iteration.
   */
  completed_iterations: string;
  /**
   * Iterations with status, transport, or timeout failures.
   */
  failed_iterations: string;
};
export type LatencySnapshot = {
  /**
   * Decimal significant digits used by the HDR histogram.
   */
  significant_digits: number;
  /**
   * Lowest supported sample in microseconds.
   */
  lowest_us: number;
  /**
   * Highest supported sample in microseconds.
   */
  highest_us: number;
  /**
   * Number of recorded samples, encoded as a decimal string.
   */
  samples: string;
  /**
   * Samples outside the supported range, encoded as a decimal string.
   */
  out_of_range: string;
  /**
   * Approximate arithmetic mean in microseconds.
   */
  mean_us: number | null;
  /**
   * Minimum recorded latency in microseconds.
   */
  min_us: number | null;
  /**
   * Maximum recorded latency in microseconds.
   */
  max_us: number | null;
  /**
   * Approximate median in microseconds.
   */
  median_us: number | null;
  /**
   * Approximate 90th percentile in microseconds.
   */
  p90_us: number | null;
  /**
   * Approximate 95th percentile in microseconds.
   */
  p95_us: number | null;
  /**
   * Approximate 99th percentile in microseconds.
   */
  p99_us: number | null;
};
export type MetricsSnapshot = {
  /**
   * Reconciled request outcomes and received payload bytes.
   */
  counters: Counters;
  /**
   * Completed HTTP response latencies, including status failures.
   */
  response_latency: LatencySnapshot;
  /**
   * Transport failure durations, excluded from response latency.
   */
  transport_latency: LatencySnapshot;
  /**
   * Timeout durations, excluded from response latency.
   */
  timeout_latency: LatencySnapshot;
  /**
   * Failure fraction over responses, transport failures, and timeouts; None when empty.
   */
  http_failure_rate: number | null;
  /**
   * Non-cancelled terminal requests per actual elapsed second.
   */
  request_rate: number;
  /**
   * Completed non-cancelled iterations per actual second; currently one request per iteration.
   */
  iteration_rate: number;
  /**
   * Successful responses per actual elapsed second.
   */
  success_rate: number;
  /**
   * Total time waiting to enter the metrics critical section, in microseconds.
   */
  aggregation_wait_us: string;
  /**
   * At least one metrics lock acquisition waited over one millisecond.
   */
  aggregation_interference: boolean;
  /**
   * At least one counter exhausted its representation; totals are then incomplete.
   */
  counter_overflow: boolean;
  /**
   * Maximum concurrently in-flight requests.
   */
  max_in_flight: string;
};
export type RunSnapshot = {
  /**
   * Monotonically increasing application-instance identifier, encoded as decimal.
   */
  run_id: string;
  /**
   * Lifecycle state, terminal only after cleanup.
   */
  state: RunState;
  /**
   * Safe target and workload details.
   */
  confirmation: RunConfirmation;
  /**
   * Scheduling time observed so far, in microseconds as a decimal string.
   */
  scheduling_us: string;
  /**
   * Time since admission closed, in microseconds as a decimal string.
   */
  drain_us: string;
  /**
   * Scheduling plus drain duration, in microseconds as a decimal string.
   */
  actual_us: string;
  /**
   * Currently owned VU workers; zero after cleanup.
   */
  active_vus: number;
  /**
   * Maximum concurrently owned VU workers.
   */
  max_vus: number;
  /**
   * Cumulative counters, rates, and separate duration populations.
   */
  metrics: MetricsSnapshot;
  /**
   * Stable internal failure code, without infrastructure or target details.
   */
  failure_code: string | null;
};
