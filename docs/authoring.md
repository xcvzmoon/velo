# Author and share a test

Open **Tests** in the desktop app. The visual editor and **JSON and environments** edit the same version 1 definition. A valid visual change updates the definition; **Apply JSON** replaces it only when parsing succeeds. Unapplied JSON and invalid visual fields block preview, export, save, and load review. Changing a definition or environment invalidates an earlier authorization review.

## Save, reopen, and share

1. Configure the request and workload in the visual editor, or import [the example definition](examples/velo.test.json).
2. Put private values in **OS secrets** and use references in the definition. For the example, set `local-api-token` before previewing.
3. Enter a name under **Saved tests** and select **Save test**. Saving an existing name replaces that test.
4. Choose a saved test and select **Load test** to reopen it. Select **Delete test** to remove its definition.
5. Select **Export velo.test.json** to choose a destination. **Import velo.test.json** opens a local JSON file.

Definitions retain literal request data, shared variables, and named environments in local SQLite. SQLite is not encrypted. Store private values in the OS credential store, with references in the document. Exports contain the editable definition, never resolved secret values. Exporting environments preserves their secret references and does not retrieve secret values. Removing a test does not remove named secrets, which may be shared by other tests. Enter the secret name and an empty value under **OS secrets** to delete it.

Earlier saved configurations remain available in **Saved configurations**. Loading a remembered request creates references to its existing OS-store fields; exporting that definition does not copy the restored credentials. These references require the original saved configuration on the same machine. Replace them with named secret references before sharing the test. Deleting the original configuration makes its references unavailable.

## Version 1 format

A document requires `version`, `config`, `variables`, and `environments`, with an optional `dataset` recipe. See [generated and imported test data](datasets.md) for preparation and allocation rules. Transient `config.data` requests cannot be embedded in a portable definition. Unsupported versions, unknown configuration properties, wrong field types, and malformed JSON fail before execution. The file limit is 8 MiB. Local storage holds at most 1,000 named tests; replacing an existing test is still allowed at that limit. A resolved field is also limited to 8 MiB, and engine request-data budgets still apply.

`config` uses the Rust-owned `RunConfig` contract, including all existing methods, ordered header/query pairs, authentication, HTTP protocol, redirect limit, timeout, pacing, drain grace, workloads, budgets, and thresholds. See [execution and measurements](execution.md) and the generated [TypeScript contracts](../app/types/contracts.ts). Duplicate header/query names retain their order. JSON bodies are strings containing JSON; they are not embedded document objects.

| Body kind   | `value`                                                                           |
| ----------- | --------------------------------------------------------------------------------- |
| `none`      | Omitted                                                                           |
| `text`      | String                                                                            |
| `json`      | String containing valid JSON, or one whole-body reference resolving to valid JSON |
| `form`      | Ordered `{ "name": "...", "value": "..." }` pairs                                 |
| `multipart` | Ordered text/file parts from `MultipartPart`                                      |

Upload paths are local bindings. File import and export clear them while retaining part metadata. Select the file again in the visual editor before previewing or running. Pasting JSON can bind a path on the current machine; an absent, unreadable, or changed file fails preparation or transport. File paths do not accept variables.

## Environments and variables

Edit `variables` and `environments` in the JSON editor. Both maps contain strings; each environment is a named map of values. Select an environment to override shared variables. The **Shared variables only** selection applies no environment overrides.

Use `{{name}}` in the URL, header names/values, query names/values, authentication, text bodies, JSON strings/keys, form pairs, and multipart fields/metadata. A variable may contain a literal value or exactly `{{secret:name}}`. Secret references can also appear directly in request fields. Nested variable expressions are rejected; secret values are inserted literally and never evaluated as templates.

URL path/query substitutions are percent-encoded. A base URL or authority reference supplies its URL components directly and is subsequently checked by the engine. Query pairs and form pairs receive transport encoding after substitution. In JSON bodies, placeholders inside strings and keys are resolved through the JSON data model, preserving escaping. A whole-body reference must resolve to valid JSON. Duplicate JSON keys created by substitutions are rejected.

Missing variables, unavailable secrets, and missing environments identify the field requiring attention. Secret names use 1 to 80 letters, digits, dots, underscores, or hyphens; `saved.` is reserved for earlier saved configurations. Named values are limited to 16,384 bytes in the OS store. OS operations are serialized; a five-second response deadline reports a pending operation without starting more credential operations. There is no plaintext fallback.

## Import an API request

Under **Import a request**, choose a format, paste the source, and select **Convert requests**. Choose a candidate, review every limitation, then select **I reviewed the conversion. Apply request**. Import replaces request settings and supplies default workload settings; review the workload before applying load. Nothing sends traffic until explicit preview or run authorization.

| Format          | Supported conversion                                                                                                                                                                                | Reported limitations                                                                                                                                                                                                  |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| cURL            | Shell-quoted `curl`; separate `-X`/`--request`, `-H`/`--header`, `-d`/`--data`/`--data-raw`/`--data-binary`, `-u`/`--user`, `--url`, `--max-time`, `--max-redirs`, `-L`/`--location`, `-I`/`--head` | Unsupported options, multiple URLs/data arguments, file-backed bodies, and shell operators fail conversion. Redirects stay on the target origin. Compression is reported for review.                                  |
| OpenAPI 3 JSON  | Candidate per supported HTTP operation, first server with operation/path overrides, path placeholders, header/query parameters                                                                      | Body schemas/examples, references, security, alternate servers, serialization options, and unsupported fields require manual configuration. YAML is unsupported. Parameter values must be supplied through variables. |
| Postman v2 JSON | Nested folders, request selection, methods, raw URLs, ordered headers/query pairs, raw text/JSON and URL-encoded bodies                                                                             | Authentication inheritance, collection variables, scripts, unsupported body modes/uploads, URL variables, and unsupported fields are reported. Configure them manually.                                               |

Parsing is offline. cURL text never runs in a shell; Postman scripts and OpenAPI references are never executed or fetched. Limits are 8 MiB per source, 1,000 candidates, and 32 Postman folder levels. Imports with 64 or more unsupported-field warnings per request are rejected; simplify the source before retrying. Imported literal credentials remain literal until you replace them with OS secret references.

## Build a multi-step scenario

1. Configure the target origin and workload in the visual editor, then open **Scenarios** and enable a scenario plan.
2. Name the journey and add its ordered HTTP steps. The step editor reuses the existing request controls; transport, timeout, response inspection, workload, and resource settings come from the parent run.
3. For login, add a required JSON extraction named `token` with pointer `/token`. Use `{{session:token}}` in later authentication fields. Set **Overwrite** when a subsequent iteration should replace an earlier token.
4. Extract an order ID from the creation response, then use `{{session:order}}` in the fetch URL path. URL substitution encodes extracted values; JSON substitution escapes string values. A missing authored reference fails locally before sending HTTP.
5. Apply step settings, return to the visual editor to adjust the workload, and select **Review load**. Confirm authorization for the reviewed origin before starting. Live and saved results separate scenario iterations from individual requests.

The version 1 document stores journeys in optional `config.scenarios`. A plan has `seed` (default zero), `setup` and `cleanup` arrays (default empty), and a required `scenarios` array. Each scenario has a unique `id`, positive `weight`, and ordered `steps`. Each step has an `id`, `request`, `on_failure` (`stop` or `continue`), optional `think_ms` (default zero), and optional `extractions` (default empty). Step requests contain `url`, `method`, `headers`, `query`, `body`, `auth`, and optional `assertions`. Workload and transport settings remain on `config`.

This extraction object belongs in a step's `extractions` array:

```json
{
  "name": "token",
  "source": { "kind": "json", "pointer": "/token" },
  "required": true,
  "overwrite": true
}
```

A JSON source uses an RFC 6901 pointer and extracts a string, number, or boolean. Header sources use `{ "kind": "header", "name": "x-token" }`; cookie sources use `{ "kind": "cookie", "name": "sid" }` and read the final response's Set-Cookie fields. JSON extraction shares the bounded assertion response reader. Oversized, encoded, invalid, missing, or nonscalar required values fail visibly. Extraction commits atomically. Failed extraction removes its destination values; missing optional extraction also clears stale values. Replacing an existing value requires `overwrite: true`.

Shared variables resolve first, environment entries override them, and dataset fields override both. Extracted values occupy the separate `session:` namespace and cannot overwrite those static values. Colons are reserved in static variable names. Scenario static values cannot introduce deferred references. Session values are inserted literally, without recursive template expansion. Assertions keep literal expectations. Session references may change URL paths and queries, never the scheme or authority. Preparation validates every setup, load, cleanup, and dataset target against the reviewed origin.

Each VU keeps its own extracted values and RFC 6265 cookie state across iterations until retirement. Each arrival iteration starts with fresh state. Setup values and cookies are copied into each session; later user changes stay local. Cookies respect domain, path, expiry, and Secure rules, including across redirects. An explicit Cookie request header takes precedence over the session jar.

`on_failure: "stop"` skips the rest of that iteration after an HTTP, assertion, binding, or required extraction failure. `continue` attempts subsequent steps, but dependent unresolved requests still fail locally. Neither policy retries implicitly. Think time follows each executed step and is excluded from HTTP latency. Explicit stop interrupts think time; active HTTP requests drain within the configured grace, then remaining workers are aborted and joined.

Relative weights select a journey once per global admission ordinal, using the configured seed and SplitMix64 mixing. The selection sequence is reproducible for the same configuration and admitted ordinals; concurrent completion order and the number admitted remain timing-dependent. Weights express probabilities, not exact percentages or quotas for small runs.

Setup executes once before measured traffic. A failed or cancelled setup prevents load admission. Cleanup executes once after workers drain, including after stopping or setup failure where possible, using setup state rather than an arbitrary user's state. With datasets, lifecycle actions use the first prepared record. Put per-user removal in a journey step. Setup and cleanup each have a separate 30-second total deadline. Cleanup ignores ordinary stop until its deadline; failed or incomplete cleanup is visible and fails the run. Cleanup cannot be guaranteed after a crash or an unreachable target.

A plan supports 1–16 scenarios, 1–32 steps per scenario, and at most 64 configured steps across all phases. IDs use 1–64 ASCII letters, digits, underscores, or hyphens. Weights range from 1 to 10,000; think time ranges from zero to one hour, subject to lifecycle deadlines. Each user retains at most 32 extracted values and 32 cookies, with separate 16-KiB value and cookie budgets and a 4-KiB limit per extracted value or Set-Cookie field. Session reservations and response buffers together must fit 16 MiB across peak workers and setup state. Templates, prepared records, worker requests, and uploads also use the configured request-data budget. Runtime expansion can fail locally if it exceeds the per-worker allowance.

Scenario IDs and step IDs define bounded metric populations. Dynamic URLs, extracted values, cookies, and response bodies never enter reports. Setup and cleanup measurements are separate from evaluated load; their time does not dilute load rates or request-rate thresholds. A successful HTTP response can still belong to a failed iteration because of assertions or extraction. Single-request preview does not execute scenario plans; use a short authorized run to validate the complete journey.

## Preview before applying load

Select **I am authorized. Preview request** to send one logical request through the engine. It uses the configured timeout, TLS verification, protocol, upload preparation, same-origin redirects, and disabled retries. A preview occupies the same native admission gate as a load start. Active load runs reject preview; another preview is not queued for later execution.

The result shows final status, elapsed milliseconds, ordered headers, and a UTF-8 response body. Capture limits are 16 KiB of headers and 64 KiB of body. The UI reports truncation and redaction. Known request values and resolved secret values are masked; sensitive response headers such as `Set-Cookie` are withheld. Truncated bodies are withheld when secret references were resolved, since a cutoff could expose part of a secret. Redaction cannot identify arbitrary server-generated secrets or every transformation of a request value.

Closing the app cancels a pending preview. Preview results are temporary. They do not create load-test history, affect comparisons, or authorize a later load run. Select **Review load**, inspect the target and workload, and authorize the load separately.
