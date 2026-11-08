# Run tests in CI

The `velo-cli` executable runs version 1 desktop-exported definitions through the shared Rust engine. It needs no webview, SQLite database, OS credential session, or desktop application. Workloads, warm-up, budgets, assertions, scenario setup/load/cleanup, redirects, and drain behavior use the same native validation and execution paths as desktop.

## Build and validate

Install the Rust toolchain pinned in `rust-toolchain.toml`, then build only the CLI:

```sh
cargo build --locked --release -p velo-cli
./target/release/velo-cli --help
```

On Windows, use `target/release/velo-cli.exe`. Tauri system packages are not required for this build. Export a definition from desktop or start with [the example definition](examples/velo.test.json).

Supply secrets through environment variables already injected by your CI secret manager. Commands name the variable, never its value:

```sh
./target/release/velo-cli docs/examples/velo.test.json \
  --prepare \
  --secret local-api-token=VELO_CI_TOKEN
```

A successful preparation prints `Prepared successfully; no traffic sent.` It parses the strict versioned format, resolves variables and secrets, checks supplied data, opens required upload files, and validates engine budgets. It does not send HTTP requests, run scenario setup/cleanup, create report files, or print resolved settings. Unsupported fields, missing inputs, unreadable files, and unsupported versions fail before execution. Preparation does not prove the target is reachable.

## Authorize and execute

Use the exact normalized origin, including a nondefault port, to acknowledge authorization for the target and the configured load. The origin has no path or trailing slash. All dataset and scenario requests must remain on this origin, as enforced by the engine.

```sh
./target/release/velo-cli docs/examples/velo.test.json \
  --authorize http://127.0.0.1:8080 \
  --secret local-api-token=VELO_CI_TOKEN \
  --json result.json \
  --junit result.xml \
  --html result.html
```

No authorization is inferred from a successful preparation. The CLI executes the resolved workload unchanged; it does not silently lower concurrency or override invalid settings.

For another environment, use `--environment staging`. Override a non-secret variable with `--variable baseUrl=VELO_CI_ORIGIN`. Overrides apply after the selected environment's values. Keep sensitive values behind `{{secret:name}}` references and supply them with `--secret name=ENVIRONMENT_VARIABLE`.

Secret bindings may also arrive as a JSON object on stdin with `--secrets-stdin`. The caller must close the stream. Environment bindings and stdin bindings must have distinct names. Values must be nonempty UTF-8 strings of at most 16 KiB. Missing environment variables fail; the CLI never prompts, accesses a graphical credential store, or writes a plaintext fallback. Desktop `saved.*` references can be supplied explicitly through these same bindings.

Request content, assertion expectations, dataset values, local filenames, and underlying OS/network error text are omitted from output. Known secret values are redacted from report text fields without changing counters, statuses, or schema keys. Non-secret target origins and configured assertion/scenario identifiers remain visible. Preparation rejects any supplied secret whose value occurs in the target origin (`SECRET_TARGET_UNSUPPORTED`); use a public origin and put credentials in request fields. This conservative check also rejects accidental overlaps with short secret values. Treat baseline reports as trusted, content-free Velo exports.

## Prepare datasets and upload bindings

Dataset recipes use the same TypeScript preparation function as desktop. Install Node.js 26 and run `pnpm install` for this optional step. Faker uses the pinned desktop version, seed, locale, and field order; CSV and JSON use desktop parsers and mappings.

```sh
node scripts/prepare-data.ts test.velo.json source.csv > rows.json
./target/release/velo-cli test.velo.json --data-rows rows.json --prepare
```

For JSON sources, pass the JSON source path instead of the CSV path. For Faker, omit the source path:

```sh
node scripts/prepare-data.ts test.velo.json > rows.json
```

Pass the same `--data-rows rows.json` to the authorized run. The file is an array of normalized string records keyed by recipe field names. The Rust boundary checks count, keys, size, interpolation, origin, request budgets, and native allocation rules before traffic. It does not regenerate Faker rows or verify their provenance. Missing rows fail with `DATASET_INPUT_REQUIRED`; rows without a recipe are rejected. Recipes, source files, and row inputs are bounded, and no generator runs during measured traffic.

Row files contain source data. Keep them out of build logs and uploaded artifacts and remove them after the run. Do not put credentials in datasets. The CLI does not persist resolved requests or rows itself.

Desktop exports clear multipart upload paths. Bind each file part's `path` to a local readable file in the CI definition before preparation. Paths cannot contain variable substitutions. Unbound or unavailable uploads fail explicitly; files are streamed through the existing engine transport.

## Reports and baseline comparisons

`--json`, `--junit`, and `--html` use the shared version 1 report schema and exporters. JSON retains integer counters as decimal strings and metrics at native precision. HTML is a standalone readable report. JUnit contains aggregate run/rule/assertion cases, including a run outcome case for scenario and completeness failures. A run without a baseline has an unevaluable regression summary; this does not fail the CLI by itself.

Artifacts contain the final authoritative snapshot after worker cleanup. The CLI retains no interval timeline (`points` is empty); aggregate measurements and generator telemetry remain available. Existing destinations are replaced atomically, one file at a time, using a temporary file in the destination directory. Parent directories must exist. Outputs may not alias an input or each other. If a write fails, other requested formats are still attempted and the command exits with status 3. No artifact is promised before the engine has a snapshot or after an uncatchable process kill.

Use a prior JSON export as a baseline. Its `candidate.id` is the `baseline_id` in the shared comparison settings file:

```json
{
  "baseline_id": "COPY_CANDIDATE_ID_FROM_BASELINE_REPORT",
  "configuration_reviewed": true,
  "budgets": [{ "metric": "http_failure_percent", "limit": 1, "percentage": false }]
}
```

```sh
./target/release/velo-cli test.velo.json \
  --authorize http://127.0.0.1:8080 \
  --baseline baseline.json \
  --comparison comparison.json \
  --json candidate.json
```

Review target, workload, configuration differences, and redacted request content before setting `configuration_reviewed` to true. The CLI uses the shared compatibility, precision, zero-baseline, and regression-direction rules. A supplied baseline must match the requested ID and report version. It must yield a passing comparison with at least one evaluable budget for the CLI to succeed. Missing review, incompatible or incomplete evidence, failed budgets, and unevaluable budgets fail closed. Absolute thresholds and assertions remain separate from regression budgets.

## Exit statuses and interruption

| Status | Meaning                                                                                                                                                                         |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`    | Preparation succeeded, or a complete run passed all configured checks and any requested regression comparison                                                                   |
| `1`    | Failed/unevaluable criteria, aborted admission, scenario-step failure, or incomplete/empty measurement evidence                                                                 |
| `2`    | Invalid arguments, authorization mismatch, definition/input/secret errors, invalid baseline settings, or colliding output paths                                                 |
| `3`    | Engine failure, no evaluated HTTP responses despite transport failures/timeouts, signal setup failure, report/terminal write failure, or other execution infrastructure failure |
| `130`  | SIGINT/SIGTERM on Unix or Ctrl+C/Ctrl+Break on Windows; retained measurements are partial                                                                                       |

HTTP error responses are measured outcomes. Configure assertions or HTTP-failure thresholds to make them fail CI. A run with some successful responses and transport failures follows its configured criteria; all evaluated attempts failing before a response produces status 3.

Catchable interruption stops admission and waits for the engine's configured bounded drain, cancellation, and scenario cleanup. When a snapshot exists, requested reports retain partial results and zero owned VU workers. Interruption during input preparation exits without starting traffic. Output failures take precedence and produce status 3 even after a criteria failure or interruption. Programming panics terminate the process; they are not treated as successful results. SIGKILL, forced runner termination, and Windows console/window closure do not guarantee artifacts.

## GitHub Actions

Copy [the workflow example](examples/headless-ci.yaml) into `.github/workflows/` in the repository that owns your definition. Configure the `load-testing` GitHub environment, an authorized `VELO_CI_ORIGIN` environment variable, and the `VELO_CI_TOKEN` environment secret. The manual workflow validates first, runs only with explicit origin authorization, and uploads report artifacts with `always()` so failed criteria do not discard evidence. It uses source from this repository; pin the checkout ref to a reviewed commit when adapting it elsewhere.

The repository's own [headless workflow](../.github/workflows/headless.yaml) builds and tests the CLI on Linux without installing desktop system packages. It sends test traffic only to local ephemeral test servers.
