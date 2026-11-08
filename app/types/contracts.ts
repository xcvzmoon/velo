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
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | { [key: string]: JsonValue }
  | JsonValue[];
export type RunMetadata = {
  /**
   * Optional Git SHA, empty when unset.
   */
  git_sha: string;
  /**
   * Release label, empty when unset.
   */
  release: string;
  /**
   * At most 20 user labels.
   */
  tags: string[];
  /**
   * User notes; include only information safe to share.
   */
  notes: string;
};
export type RegressionMetric =
  | 'response_p50_ms'
  | 'response_p95_ms'
  | 'response_p99_ms'
  | 'request_rate'
  | 'http_failure_percent'
  | 'assertion_failure_percent';
export type RegressionBudget = {
  /**
   * Metric population and unit.
   */
  metric: RegressionMetric;
  /**
   * Finite nonnegative limit; equality passes.
   */
  limit: number;
  /**
   * Whether the limit uses relative percent instead of absolute units.
   */
  percentage: boolean;
};
export type ComparisonSettings = {
  /**
   * Durable reference; deletion does not select a replacement.
   */
  baseline_id: string;
  /**
   * At most 32 regression limits, independent of absolute thresholds.
   */
  budgets: RegressionBudget[];
  /**
   * Explicit acknowledgement of the displayed configuration differences.
   */
  configuration_reviewed: boolean;
};
export type ConfigurationDifference = {
  /**
   * Name of the changed configuration field.
   */
  field: string;
  /**
   * Original safe value.
   */
  baseline: JsonValue;
  /**
   * Candidate safe value.
   */
  candidate: JsonValue;
};
export type MetricDelta = {
  /**
   * Metric population and unit.
   */
  metric: RegressionMetric;
  /**
   * Baseline measurement, absent when unavailable.
   */
  baseline: number | null;
  /**
   * Candidate measurement, absent when unavailable.
   */
  candidate: number | null;
  /**
   * Candidate minus baseline in metric units.
   */
  absolute: number | null;
  /**
   * 100 times delta divided by baseline; absent for zero or missing baseline.
   */
  percentage: number | null;
};
export type RegressionResult = {
  /**
   * Configured limit.
   */
  budget: RegressionBudget;
  /**
   * Direction-adjusted change in the budget unit.
   */
  worsening: number | null;
  /**
   * Pass, fail, or unevaluable; never an absolute threshold verdict.
   */
  verdict: RegressionVerdict;
};
export type RegressionVerdict = 'pass' | 'fail' | 'unevaluable';
export type ComparisonReport = {
  /**
   * Report schema version, currently 1.
   */
  format_version: number;
  /**
   * Requested baseline identifier even after deletion.
   */
  baseline_id: string;
  /**
   * Baseline evidence, absent after deletion.
   */
  baseline: HistoryEntry | null;
  /**
   * Candidate measurements and release context.
   */
  candidate: HistoryEntry;
  /**
   * Whether the user explicitly reviewed retained and redacted configuration context.
   */
  configuration_reviewed: boolean;
  /**
   * Safe configuration differences requiring review.
   */
  differences: ConfigurationDifference[];
  /**
   * Compatibility, completeness, and integrity limitations.
   */
  limitations: string[];
  /**
   * Unrounded metric changes, independent of budgets.
   */
  deltas: MetricDelta[];
  /**
   * Budget verdicts, separate from thresholds.
   */
  regressions: RegressionResult[];
  /**
   * Combined regression outcome; no configured budgets is unevaluable.
   */
  verdict: RegressionVerdict;
};
export type ScenarioPlan = {
  /**
   * Seed for deterministic selection in global admission order; default zero.
   */
  seed: number;
  /**
   * Setup runs once before scheduling, with a 30-second total deadline.
   * Its extracted values and cookies seed each isolated session. With datasets,
   * lifecycle actions use the first prepared record.
   */
  setup: ScenarioStep[];
  /**
   * Cleanup runs once after workers drain, using setup state only.
   * Normal stop does not cancel cleanup; a separate 30-second total deadline applies.
   */
  cleanup: ScenarioStep[];
  /**
   * One to 16 weighted journeys. Weights are relative probabilities, not quotas.
   */
  scenarios: Scenario[];
};
export type Scenario = {
  /**
   * Stable identifier, unique in the plan; 1 to 64 ASCII letters, digits, underscores or hyphens.
   */
  id: string;
  /**
   * Relative selection weight, 1 to 10,000.
   */
  weight: number;
  /**
   * One to 32 steps executed sequentially.
   */
  steps: ScenarioStep[];
};
export type ScenarioStep = {
  /**
   * Stable identity unique within this phase or scenario.
   */
  id: string;
  /**
   * HTTP content. Transport, workload, timeout and inspection limits come from the run.
   */
  request: ScenarioRequest;
  /**
   * Extraction is atomic; failed or missing optional values remove stale state.
   */
  extractions: Extraction[];
  /**
   * Cancellable delay after this step, excluded from HTTP latency; default zero.
   */
  think_ms: number;
  /**
   * Stop the iteration or continue independent steps; no implicit retries.
   */
  on_failure: StepFailure;
};
export type ScenarioRequest = {
  /**
   * Absolute URL. Session placeholders are allowed only in the path or query.
   */
  url: string;
  /**
   * Logical HTTP method.
   */
  method: RequestMethod;
  /**
   * Ordered HTTP headers.
   */
  headers: NameValue[];
  /**
   * Ordered query parameters.
   */
  query: NameValue[];
  /**
   * Optional payload; JSON placeholders must be inside strings.
   */
  body: RequestBody;
  /**
   * Authentication resolved separately for each user.
   */
  auth: RequestAuth;
  /**
   * Ordered bounded response checks for this step.
   */
  assertions: Assertion[];
};
export type StepFailure = 'stop' | 'continue';
export type Extraction = {
  /**
   * Session variable name, 1 to 64 ASCII letters, digits, underscores or hyphens.
   */
  name: string;
  /**
   * Source in the final response.
   */
  source: ExtractionSource;
  /**
   * Missing or invalid values fail the step when true.
   */
  required: boolean;
  /**
   * Allow replacing an existing session value; otherwise collision fails the step.
   */
  overwrite: boolean;
};
export type ExtractionSource =
  | {
      kind: 'json';
      /**
       * Empty selects the root; otherwise begins with `/`.
       */
      pointer: string;
    }
  | {
      kind: 'header';
      /**
       * Case-insensitive HTTP header name.
       */
      name: string;
    }
  | {
      kind: 'cookie';
      /**
       * Case-sensitive cookie name.
       */
      name: string;
    };
export type ScenarioReport = {
  /**
   * Bounded configured iteration populations.
   */
  iterations: ScenarioTotals[];
  /**
   * At most 64 fixed step populations; dynamic URLs never create identities.
   */
  steps: StepTotals[];
  /**
   * Setup outcome; failures prevent load.
   */
  setup: PhaseOutcome;
  /**
   * Cleanup outcome; failure never implies successful removal.
   */
  cleanup: PhaseOutcome;
};
export type ScenarioTotals = {
  /**
   * Stable configured scenario identity.
   */
  id: string;
  /**
   * Admitted iterations.
   */
  started: string;
  /**
   * Non-cancelled iterations, including failures.
   */
  completed: string;
  /**
   * Completed iterations with any step failure.
   */
  failed: string;
  /**
   * Iterations interrupted before completion.
   */
  cancelled: string;
};
export type StepTotals = {
  /**
   * Configured scenario identity; empty for setup and cleanup.
   */
  scenario_id: string;
  /**
   * Configured step identity.
   */
  step_id: string;
  /**
   * Measurement population.
   */
  phase: ScenarioPhase;
  /**
   * HTTP attempts started; excludes local binding failures.
   */
  requests: string;
  /**
   * Step failures, including binding and extraction failures.
   */
  failed: string;
  /**
   * HTTP attempts interrupted during drain.
   */
  cancelled: string;
  /**
   * Requests rejected locally before sending.
   */
  binding_failures: string;
  /**
   * Responses with failed required extraction or state limits.
   */
  extraction_failures: string;
  /**
   * Latest stable failure code; never response content.
   */
  last_error: string | null;
  /**
   * Complete HTTP response latency, excluding think time.
   */
  response_latency: LatencySnapshot;
};
export type PhaseOutcome = 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'skipped';
export type ScenarioPhase = 'setup' | 'load' | 'cleanup';
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
   * Dataset-resolved journeys, retaining runtime session placeholders.
   */
  scenarios?: ScenarioPlan;
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
   * Editable release context, separate from measurements.
   */
  metadata: RunMetadata;
  /**
   * Safe definition identity, absent in older history.
   */
  definition_identity: string | null;
  /**
   * Allowlisted execution settings, with request content omitted.
   */
  configuration: JsonValue;
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
export type ApiSessionView = {
  /**
   * Extracted variable names; values remain in memory only.
   */
  variables: string[];
  /**
   * Cookie metadata, with masked values and domain/path scope.
   */
  cookies: NameValue[];
};
export type FunctionalState = 'passed' | 'failed' | 'cancelled' | 'skipped';
export type FunctionalResult = {
  /**
   * Saved request identity.
   */
  request_id: string;
  /**
   * Authored display name, never a resolved target URL.
   */
  name: string;
  /**
   * One-based iteration number.
   */
  iteration: number;
  /**
   * Final step state.
   */
  state: FunctionalState;
  /**
   * Bounded masked response, absent before headers or for cancellation.
   */
  response: PreviewResponse | null;
  /**
   * Stable safe failure code, without response values.
   */
  error: string | null;
};
export type FunctionalRun = {
  /**
   * Always `functional`, including exported evidence.
   */
  kind: FunctionalKind;
  /**
   * Operation identity, also used for cancellation.
   */
  id: string;
  /**
   * Collection identity used to reopen a saved request without sending it.
   */
  collection_id: string;
  /**
   * Selected environment, or shared values when absent.
   */
  environment: string | null;
  /**
   * `running`, `completed`, or `cancelled`.
   */
  state: FunctionalRunState;
  /**
   * Planned step count, including skipped outcomes.
   */
  total: number;
  /**
   * Portable authored snapshots for history restoration, without resolved secrets or local file paths.
   */
  requests: CollectionRequest[];
  /**
   * Outcomes in displayed execution order.
   */
  results: FunctionalResult[];
  /**
   * Safe final session view; never a persisted cookie jar.
   */
  session: ApiSessionView;
  /**
   * Local persistence failure, if execution completed but history could not be retained.
   */
  storage_error: string | null;
};
export type FunctionalKind = 'functional';
export type FunctionalRunState = 'running' | 'completed' | 'cancelled';
export type BodyFormat = 'text' | 'binary' | 'unsupported_encoding' | 'invalid_utf8';
export type ApiCollection = {
  /**
   * Portable format version, currently 1.
   */
  version: number;
  /**
   * Stable local identity, 1 to 80 ASCII letters, digits, underscores or hyphens.
   */
  id: string;
  /**
   * Display name, at most 160 bytes.
   */
  name: string;
  /**
   * Purpose and context, at most 16 KiB.
   */
  description: string;
  /**
   * Usage notes, at most 16 KiB.
   */
  notes: string;
  /**
   * Root inherited scope.
   */
  defaults: CollectionDefaults;
  /**
   * Named overrides applied after inherited and request variables.
   */
  environments: Record<string, Record<string, string>>;
  /**
   * JSON pointers masked after assertions and extraction, before display, history or export.
   */
  sensitive_response_fields: string[];
  /**
   * Ordered folders and saved requests.
   */
  entries: CollectionEntry[];
};
export type CollectionDefaults = {
  /**
   * Ordered headers overridden case-insensitively by an inner scope.
   */
  headers: NameValue[];
  /**
   * Explicit authentication override; absence inherits the parent.
   */
  auth: RequestAuth | null;
  /**
   * Shared values or secret references, overridden by inner scopes and environments.
   */
  variables: Record<string, string>;
};
export type CollectionEntry =
  | {
      kind: 'folder';
      /**
       * Identity unique across the entire collection.
       */
      id: string;
      /**
       * Display name, at most 160 bytes.
       */
      name: string;
      /**
       * Purpose and context, at most 16 KiB.
       */
      description: string;
      /**
       * Usage notes, at most 16 KiB.
       */
      notes: string;
      /**
       * Defaults applied after the parent scope.
       */
      defaults: CollectionDefaults;
      /**
       * Ordered folders and saved requests.
       */
      entries: CollectionEntry[];
    }
  | {
      kind: 'request';
      /**
       * Identity unique across the entire collection.
       */
      id: string;
      /**
       * Display name, at most 160 bytes.
       */
      name: string;
      /**
       * Purpose and context, at most 16 KiB.
       */
      description: string;
      /**
       * Usage notes, at most 16 KiB.
       */
      notes: string;
      /**
       * Canonical shared request, variables, environment overrides and dataset recipe.
       */
      definition: TestDefinition;
      /**
       * Inherit defaults when request authentication is None; false explicitly disables auth.
       */
      inherit_auth: boolean;
      /**
       * Response values made available to later requests in the session namespace.
       */
      extractions: Extraction[];
    };
export type CollectionRequest = {
  /**
   * Original saved request identity.
   */
  id: string;
  /**
   * Original saved request name.
   */
  name: string;
  /**
   * Shared request with inherited settings applied.
   */
  definition: TestDefinition;
  /**
   * Ordered response extraction rules.
   */
  extractions: Extraction[];
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
   * Bytes received across response hops before bounded capture stopped; a lower bound when truncated.
   */
  received_bytes: bigint;
  /**
   * Maximum retained body bytes, independent of the assertion inspection limit.
   */
  body_limit: number;
  /**
   * Body representation; non-text and unsupported encodings are withheld.
   */
  body_format: BodyFormat;
  /**
   * True for live capture or opted-in history; false when retention omitted response content.
   */
  content_retained: boolean;
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
      kind: 'api_key';
      /**
       * Header or query parameter name; must not conflict with an explicit request field.
       * Header placement excludes framing, Authorization, Proxy-Authorization, and Cookie headers.
       */
      name: string;
      /**
       * Secret value, omitted from reports and Debug output.
       */
      value: string;
      /**
       * Explicit placement; query values can appear in server access logs.
       */
      placement: ApiKeyPlacement;
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
export type ApiKeyPlacement = 'header' | 'query';
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
export type AbortPolicy = {
  /**
   * Acceptable strict comparison; failure can close admission after sustained evidence.
   */
  rule: Threshold;
  /**
   * Complete trailing window, 100–60,000 ms in multiples of 100 ms.
   */
  window_ms: number;
  /**
   * Required terminal requests, response latencies, or assertion evaluations.
   */
  min_samples: number;
  /**
   * Consecutive breached observation duration, 100–60,000 ms in 100 ms steps.
   */
  sustained_ms: number;
};
export type AbortEvidence = {
  /**
   * Policy that closed admission; the first configured simultaneous breach wins.
   */
  policy: AbortPolicy;
  /**
   * Last unrounded observation in the configured metric's unit.
   */
  observed: number | null;
  /**
   * Metric-specific population in the triggering complete window.
   */
  samples: string;
  /**
   * Monotonic time from load start when admission closed, including warm-up.
   */
  at_us: string;
  /**
   * Continuous observed breach duration, excluding initial window coverage.
   */
  breached_us: string;
};
export type MeasurementWindow = {
  /**
   * Scheduling offset where evaluated admissions begin; zero disables warm-up.
   */
  warmup_ms: number;
  /**
   * Observed duration after warm-up through drain; zero before the boundary.
   */
  evaluated_us: string;
  /**
   * True only after all configured scheduling time and load cleanup completed.
   */
  complete: boolean;
};
export type RunConfig = {
  /**
   * Optional multi-step journeys; absent runs the standalone request.
   */
  scenarios?: ScenarioPlan;
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
   * Warm-up scheduling time excluded from evaluated metrics; zero by default.
   */
  warmup_ms: number;
  /**
   * Live admission rules; at most eight, inactive during warm-up.
   */
  abort_policies: AbortPolicy[];
  /**
   * Final evaluated metric rules; empty by default.
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
   * Warm-up boundary in scheduling milliseconds.
   */
  warmup_ms: number;
  /**
   * Reviewed live admission rules.
   */
  abort_policies: AbortPolicy[];
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
export type RunState =
  | 'starting'
  | 'running'
  | 'stopping'
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'aborted';
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
   * Completed non-cancelled iterations; scenarios may contain several requests.
   */
  completed_iterations: string;
  /**
   * Iterations with HTTP failures; scenarios also include assertion, binding and extraction failures.
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
   * Completed non-cancelled iterations per actual second; independent of requests per iteration.
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
export type TelemetryStatus = 'warming_up' | 'available' | 'unavailable' | 'unsupported';
export type HostTelemetry = {
  /**
   * Sampler availability; a short run may finish while still warming up.
   */
  status: TelemetryStatus;
  /**
   * Workload-relative end of the native sample, as decimal microseconds.
   */
  sampled_at_us: string | null;
  /**
   * Actual native refresh window, as decimal microseconds, excluding initialization.
   */
  interval_us: string | null;
  /**
   * Logical CPUs used to normalize process CPU; null when unavailable.
   */
  logical_cpus: number | null;
  /**
   * Native process CPU divided by logical CPU count, in percent.
   */
  process_cpu_percent: number | null;
  /**
   * Native process resident memory in decimal bytes.
   */
  process_memory_bytes: string | null;
  /**
   * Host-wide CPU usage across logical CPUs, in percent.
   */
  system_cpu_percent: number | null;
  /**
   * Host RAM available for allocation, in decimal bytes; OS accounting varies.
   */
  system_available_memory_bytes: string | null;
  /**
   * Total host physical RAM, in decimal bytes.
   */
  system_total_memory_bytes: string | null;
};
export type RunSnapshot = {
  /**
   * Latest native resource sample; its window is independent of aggregate publishing.
   */
  telemetry: HostTelemetry;
  /**
   * Separate scenario iterations, fixed step populations and lifecycle outcomes.
   */
  scenarios: ScenarioReport | null;
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
   * Admission-selected evaluated traffic, including its drain and cancellations.
   */
  evaluated: MetricsSnapshot;
  /**
   * Explicit monotonic warm-up boundary and measurement completeness.
   */
  measurement: MeasurementWindow;
  /**
   * Sustained live policy trigger, separate from final threshold verdicts.
   */
  automatic_stop: AbortEvidence | null;
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
