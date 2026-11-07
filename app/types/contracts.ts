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
export type Assertion = {
  /**
   * Unique ASCII identifier, 1–64 letters, digits, underscores or hyphens.
   */
  id: string;
  /**
   * Condition evaluated against the final response.
   */
  rule: AssertionRule;
};
export type AssertionRule =
  | { kind: 'status'; allowed: number[] }
  | { kind: 'header_present'; name: string }
  | { kind: 'header_equals'; name: string; value: string }
  | { kind: 'json_present'; pointer: string }
  | { kind: 'json_equals'; pointer: string; value: string }
  | { kind: 'text_contains'; value: string }
  | { kind: 'text_equals'; value: string };
export type InspectionLimits = {
  /**
   * Body inspection bytes per worker, 1–65,536. Excess bytes are drained and discarded.
   */
  body_bytes: number;
  /**
   * Retained representative samples per run, 0–32. Zero disables samples.
   */
  sample_count: number;
  /**
   * Total serialized sample bytes per run, 0–32,768. Zero disables samples.
   */
  sample_bytes: number;
};
export type CheckOutcome =
  | 'passed'
  | 'status_mismatch'
  | 'missing_header'
  | 'header_mismatch'
  | 'missing_field'
  | 'json_mismatch'
  | 'text_mismatch'
  | 'truncated'
  | 'invalid_json'
  | 'invalid_utf8'
  | 'encoded_body'
  | 'unavailable';
export type AssertionTotals = {
  /**
   * Stable configured identifier; expectations are never included.
   */
  id: string;
  /**
   * Matching complete responses.
   */
  passed: string;
  /**
   * Evaluated conditions that did not match.
   */
  failed: string;
  /**
   * Checks not evaluable, including invalid or truncated bodies and non-response attempts.
   */
  unavailable: string;
};
export type ResponseSample = {
  /**
   * Zero-based assertion index, or None for HTTP, transport, timeout or cancellation causes.
   */
  assertion: number | null;
  /**
   * Final response status when headers arrived; absent for connection failures.
   */
  status: number | null;
  /**
   * Stable cause, never an infrastructure message or actual/expected value.
   */
  cause: string;
};
export type FailureGroup = {
  /**
   * Safe representative metadata for this group.
   */
  example: ResponseSample;
  /**
   * Occurrences, including those whose sample could not be retained.
   */
  count: string;
};
export type ResponseDiagnostics = {
  /**
   * Per-assertion totals in configuration order.
   */
  assertions: AssertionTotals[];
  /**
   * Groups capped at 256; omitted group occurrences are counted separately.
   */
  groups: FailureGroup[];
  /**
   * First representative for each retained group, within count and serialized byte limits.
   */
  samples: ResponseSample[];
  /**
   * New representative samples omitted by retention limits.
   */
  dropped_samples: string;
  /**
   * Failure occurrences that could not fit in the group table.
   */
  dropped_groups: string;
  /**
   * Complete responses whose inspected body exceeded the cap.
   */
  truncated_responses: string;
  /**
   * Serialized bytes of the nonempty samples array, including delimiters; zero when empty.
   */
  retained_bytes: number;
  /**
   * Active run limits.
   */
  limits: InspectionLimits;
};
export type DataLifetime = 'run' | 'user' | 'iteration';
export type DataAllocation = 'sequential' | 'random' | 'unique';
export type DataExhaustion = 'stop' | 'repeat' | 'fail';
export type DataDefinition = {
  /**
   * Reuse boundary for selected records.
   */
  lifetime: DataLifetime;
  /**
   * Global selection order.
   */
  allocation: DataAllocation;
  /**
   * Explicit end-of-data behavior.
   */
  exhaustion: DataExhaustion;
  /**
   * Seed for generation and the native allocation shuffle.
   */
  seed: number;
  /**
   * Generator or import recipe. Imported files must be selected on this machine.
   */
  source: DataSource;
};
export type DataSource =
  | {
      kind: 'faker';
      /**
       * Exact supported package version, currently 10.6.0.
       */
      version: string;
      /**
       * Supported locale: en, de, or fr, with English fallback.
       */
      locale: string;
      /**
       * Number of prepared records, 1 through 10,000.
       */
      count: number;
      /**
       * Ordered fields; related fields may refer to earlier fields only.
       */
      fields: DataField[];
    }
  | {
      kind: 'csv';
      /**
       * Required source columns, mapped to request variables.
       */
      mapping: Record<string, string>;
    }
  | {
      kind: 'json';
      /**
       * Required source fields, mapped to request variables.
       */
      mapping: Record<string, string>;
    };
export type DataField = {
  /**
   * Variable name, using letters, digits, underscores, dots, or hyphens.
   */
  name: string;
  /**
   * Allowlisted generator and its supported arguments.
   */
  generator: DataGenerator;
};
export type DataGenerator =
  | { kind: 'first_name' }
  | { kind: 'last_name' }
  | { kind: 'uuid' }
  | {
      kind: 'email';
      /**
       * Earlier field containing the first name.
       */
      first: string;
      /**
       * Earlier field containing the last name.
       */
      last: string;
      /**
       * Email provider domain.
       */
      provider: string;
    }
  | {
      kind: 'integer';
      /**
       * Inclusive minimum.
       */
      min: number;
      /**
       * Inclusive maximum, at least min.
       */
      max: number;
    }
  | {
      kind: 'literal';
      /**
       * Literal string, at most 16 KiB.
       */
      value: string;
    }
  | {
      kind: 'copy';
      /**
       * Earlier field name.
       */
      field: string;
    };
export type DataRequest = {
  /**
   * Resolved HTTP URL.
   */
  url: string;
  /**
   * Resolved ordered headers.
   */
  headers: NameValue[];
  /**
   * Resolved ordered query pairs.
   */
  query: NameValue[];
  /**
   * Resolved body with JSON escaping preserved.
   */
  body: RequestBody;
  /**
   * Resolved authentication.
   */
  auth: RequestAuth;
};
export type PreparedData = {
  /**
   * Recipe that produced these requests.
   */
  definition: DataDefinition;
  /**
   * Resolved request records, limited to 10,000 and the configured byte budget.
   */
  requests: DataRequest[];
};
export type DataSummary = {
  /**
   * Source kind: faker, csv, or json.
   */
  source: string;
  /**
   * Faker package version, absent for imports.
   */
  version: string | null;
  /**
   * Faker locale, absent for imports.
   */
  locale: string | null;
  /**
   * Generation and shuffle seed.
   */
  seed: number;
  /**
   * Prepared record count.
   */
  records: number;
  /**
   * Reuse boundary.
   */
  lifetime: DataLifetime;
  /**
   * Selection order.
   */
  allocation: DataAllocation;
  /**
   * End-of-data behavior.
   */
  exhaustion: DataExhaustion;
  /**
   * Allocations made, excluding reuse within a lifetime.
   */
  allocated: string;
  /**
   * Repeat cycles entered after the initial pass.
   */
  cycles: string;
  /**
   * True once a stop or fail policy closed admission.
   */
  exhausted: boolean;
};
export type HistoryEntry = {
  /**
   * HTTP method used for comparison eligibility.
   */
  method: string;
  /**
   * Durable identifier independent of engine process identifiers.
   */
  id: string;
  /**
   * UTC creation time as Unix seconds.
   */
  created_at: string;
  /**
   * True when the application closed before a terminal snapshot was recorded.
   */
  interrupted: boolean;
  /**
   * True when some aggregate intervals could not be stored.
   */
  incomplete: boolean;
  /**
   * Sampling stride; one retains every interval, two every other interval.
   */
  stride: number;
  /**
   * Latest safe engine summary.
   */
  snapshot: RunSnapshot;
  /**
   * At most 600 aggregate intervals; empty in paginated list responses.
   */
  points: RunUpdate[];
};
export type SavedConfiguration = {
  /**
   * Durable configuration and credential reference identifier.
   */
  id: string;
  /**
   * User-provided display name, never inferred from request content.
   */
  name: string;
  /**
   * Whether loading requires the OS credential store.
   */
  remembered: boolean;
  /**
   * Redacted configuration on save/list; restored only during explicit load.
   */
  config: RunConfig;
};
export type TestDefinition = {
  /**
   * Format version; only 1 is supported.
   */
  version: number;
  /**
   * Optional dataset recipe; prepared rows are transient and must be rebound after loading.
   */
  dataset?: DataDefinition;
  /**
   * Editable request and workload, with optional `{{name}}` placeholders.
   */
  config: RunConfig;
  /**
   * Shared literal values or OS secret references, overridden by the selected environment.
   */
  variables: Record<string, string>;
  /**
   * Named values or `{{secret:name}}` references. Resolved secrets are never stored here.
   */
  environments: Record<string, Record<string, string>>;
};
export type ImportFormat = 'curl' | 'openapi' | 'postman';
export type ImportedRequest = {
  /**
   * Display name from the source; may contain private information.
   */
  name: string;
  /**
   * Converted request with default workload and budgets.
   */
  config: RunConfig;
  /**
   * Unsupported fields, security, scripts, or unresolved references.
   */
  warnings: string[];
};
export type PreviewResponse = {
  /**
   * Assertion outcomes in configuration order; expectations and actual values are omitted.
   */
  checks: CheckOutcome[];
  /**
   * Final HTTP status, including non-success responses.
   */
  status: number;
  /**
   * Elapsed time including redirects and bounded body consumption.
   */
  elapsed_ms: number;
  /**
   * Ordered response headers, with sensitive values masked.
   */
  headers: NameValue[];
  /**
   * Lossy UTF-8 body, with known request values masked.
   */
  body: string;
  /**
   * Whether body or header capture exceeded its limit.
   */
  truncated: boolean;
  /**
   * True when response content has been masked.
   */
  redacted: boolean;
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
export type RequestMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';
export type RequestAuth =
  | { kind: 'none' }
  | {
      kind: 'bearer';
      /**
       * Secret token, never returned in run snapshots.
       */
      token: string;
    }
  | {
      kind: 'basic';
      /**
       * User identifier; may be empty.
       */
      username: string;
      /**
       * Secret password; may be empty, but cannot contain control characters.
       */
      password: string;
    };
export type HttpProtocol = 'auto' | 'http1' | 'http2';
export type MultipartPart =
  | {
      kind: 'text';
      /**
       * Multipart field name without control characters.
       */
      name: string;
      /**
       * UTF-8 content.
       */
      value: string;
    }
  | {
      kind: 'file';
      /**
       * Multipart field name without control characters.
       */
      name: string;
      /**
       * Absolute local file path, never used as the transmitted filename.
       */
      path: string;
      /**
       * Transmitted filename; no path separators or control characters.
       */
      filename: string;
      /**
       * Valid MIME type; defaults in the editor to application/octet-stream.
       */
      content_type: string;
    };
export type RequestBody =
  | { kind: 'none' }
  | { kind: 'text'; value: string }
  | { kind: 'json'; value: string }
  | { kind: 'form'; value: NameValue[] }
  | { kind: 'multipart'; value: MultipartPart[] };
export type ResourceBudget = {
  /**
   * Maximum number of simultaneously owned VU tasks; must be positive.
   */
  max_vus: number;
  /**
   * Maximum normalized request bytes across the template and VU clones,
   * including pair overhead, the stage schedule, threshold rules, and upload buffers.
   * This is not a process-wide memory limit.
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
export type ThresholdMetric =
  | 'response_p95_ms'
  | 'http_failure_percent'
  | 'assertion_failure_percent'
  | 'request_rate';
export type ThresholdOperator = 'less_than' | 'greater_than';
export type Threshold = {
  /**
   * Population and unit to evaluate.
   */
  metric: ThresholdMetric;
  /**
   * Strict comparison against the limit.
   */
  operator: ThresholdOperator;
  /**
   * Maximum 100 for percentages and 3,600,000 for latency milliseconds.
   */
  limit: number;
};
export type ThresholdStatus = 'passed' | 'failed' | 'insufficient_data';
export type ThresholdResult = {
  /**
   * Configured rule in its original order.
   */
  rule: Threshold;
  /**
   * Unrounded observed value in the rule's unit, or None for insufficient data.
   */
  observed: number | null;
  /**
   * Final comparison outcome.
   */
  status: ThresholdStatus;
};
export type RunConfig = {
  /**
   * Absolute HTTP(S) URL without embedded credentials or a fragment.
   */
  url: string;
  /**
   * Logical HTTP method. GET and HEAD reject request bodies.
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
   * Optional body for methods other than GET and HEAD.
   */
  body: RequestBody;
  /**
   * Explicit authentication; custom Authorization headers cannot be combined with it.
   */
  auth: RequestAuth;
  /**
   * Negotiated HTTP by default; HTTP/2 forces h2, including prior knowledge for cleartext.
   */
  protocol: HttpProtocol;
  /**
   * Maximum redirects to follow; zero disables redirects. Cross-origin redirects and
   * HTTPS downgrades are rejected, keeping custom credentials and payloads on the target origin.
   */
  max_redirects: number;
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
   * Final cumulative metric rules; empty by default.
   */
  thresholds: Threshold[];
  /**
   * Ordered response checks; at most 32, validated before load starts.
   */
  assertions: Assertion[];
  /**
   * Bounded response inspection and safe diagnostic sample retention.
   */
  inspection: InspectionLimits;
  /**
   * Explicit task and retained request-data limits.
   */
  budget: ResourceBudget;
  /**
   * Transient prepared requests; omitted from portable definitions and saved configurations.
   */
  data?: PreparedData;
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
  /**
   * Safe dataset preparation metadata, absent for static requests.
   */
  dataset: DataSummary | null;
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
   * Assertion outcomes and bounded content-free diagnostic samples.
   */
  diagnostics: ResponseDiagnostics;
  /**
   * Failed assertion evaluations over all evaluations; absent if any check is unavailable.
   */
  assertion_failure_rate: number | null;
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
   * Slots rejected because a stop or fail dataset exhaustion policy closed admission.
   */
  dataset_drops: string;
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
   * Final threshold outcomes; empty while active or when no rules are configured.
   */
  thresholds: ThresholdResult[];
  /**
   * Stable internal failure code, without infrastructure or target details.
   */
  failure_code: string | null;
  /**
   * Safe preparation metadata and final allocation counts; never includes rows.
   */
  dataset: DataSummary | null;
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
