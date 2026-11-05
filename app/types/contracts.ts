export type DiagnosticInput = {
  /**
   * Label containing 1 to 64 characters after trimming whitespace.
   */
  label: string;
};
export type DiagnosticReport = {
  /**
   * Name of the engine handling the diagnostic.
   */
  engine: string;
  /**
   * Engine package version.
   */
  version: string;
};
export type CommandError = {
  /**
   * Stable code identifying the failure, such as `INVALID_DIAGNOSTIC_LABEL`.
   */
  code: string;
  /**
   * Message safe to display to the user.
   */
  message: string;
  /**
   * Input field associated with the failure, or `None` for command failures.
   */
  field: string | null;
};
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
   * including pair overhead and the stage schedule. This is not a process-wide memory limit.
   */
  max_request_bytes: number;
};
export type RampStage = {
  /**
   * Positive stage duration in milliseconds.
   */
  duration_ms: number;
  /**
   * Desired workers at the stage endpoint; zero is allowed.
   */
  target_vus: number;
};
export type Workload =
  | {
      kind: 'constant';
      /**
       * Sequential worker count.
       */
      vus: number;
      /**
       * Scheduling duration in milliseconds.
       */
      duration_ms: number;
    }
  | {
      kind: 'arrival';
      /**
       * Positive target iterations per second; independent of response speed.
       */
      rate: number;
      /**
       * Maximum simultaneously owned single-request tasks; must be positive.
       */
      capacity: number;
      /**
       * Positive scheduling duration in milliseconds.
       */
      duration_ms: number;
    }
  | {
      kind: 'ramping';
      /**
       * Initial worker count; zero is allowed.
       */
      start_vus: number;
      /**
       * Nonempty stage schedule; total duration must fit a positive u32 in milliseconds.
       */
      stages: RampStage[];
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
   * Constant, staged, or arrival schedule, validated before worker allocation.
   */
  workload: Workload;
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
   * Peak requested workers, including the starting count.
   */
  vus: number;
  /**
   * Requested scheduling duration in milliseconds.
   */
  duration_ms: number;
  /**
   * Native-validated schedule shown during authorization review.
   */
  workload: Workload;
  /**
   * Explicit allocation budgets.
   */
  budget: ResourceBudget;
  /**
   * HIGH_VU_COUNT warns at 100 workers; HIGH_ARRIVAL_RATE at 1,000 iterations/s.
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
export type ArrivalSnapshot = {
  /**
   * Deadlines reached while scheduling was open, including dropped iterations.
   */
  offered: string;
  /**
   * Current slots rejected because all concurrency capacity was owned.
   */
  capacity_drops: string;
  /**
   * Expired slots skipped after a delayed scheduler wake; never replayed.
   */
  missed_deadlines: string;
  /**
   * Maximum wake delay against the next unprocessed deadline, in microseconds.
   */
  max_lateness_us: string;
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
   * Desired workers at the last scheduler reconciliation; zero during drain.
   */
  desired_vus: number;
  /**
   * Workers finishing their current request before retirement.
   */
  retiring_vus: number;
  /**
   * Currently owned VU workers; zero after cleanup.
   */
  active_vus: number;
  /**
   * Maximum concurrently owned VU workers.
   */
  max_vus: number;
  /**
   * Arrival scheduling counters; zero for VU workloads. Drops exclude HTTP failures.
   */
  arrival: ArrivalSnapshot;
  /**
   * Cumulative counters, rates, and separate duration populations.
   */
  metrics: MetricsSnapshot;
  /**
   * Stable internal failure code, without infrastructure or target details.
   */
  failure_code: string | null;
};
export type RunUpdate = {
  /**
   * Sequence starting at one within each run, encoded as decimal.
   */
  sequence: string;
  /**
   * Authoritative cumulative state at this sample.
   */
  snapshot: RunSnapshot;
  /**
   * Monotonic time since the preceding sample or execution start, in microseconds.
   */
  interval_us: string;
  /**
   * Completed non-cancelled attempts in this interval.
   */
  interval_requests: string;
  /**
   * Status, transport, and timeout failures in this interval.
   */
  interval_failures: string;
  /**
   * Bounded response-latency population for this interval.
   */
  response_latency: LatencySnapshot;
};
