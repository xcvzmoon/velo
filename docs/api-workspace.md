# API workspace

Choose a collection in **API workspace**, then use **Requests** to edit it, **Run collection** to authorize execution and inspect responses, **History** for stored functional evidence, and **Secrets** for OS-store references. Collection controls sit beside request editing at wide desktop sizes and above it in smaller windows.

Use **API workspace** for individual HTTP requests and ordered functional checks. Use **Tests** for load generation. API sends and collection runs never start load, and their evidence does not become a performance baseline.

## Organize requests

Create a collection, then add requests or nested folders. Open requests in tabs. Move entries up or down to change the displayed execution order; duplicate an entry to give its copy fresh identities. Search matches request and folder names. Collection buttons also support duplication and ordering.

The dot beside a collection or request tab indicates unsaved work. Save the collection to retain changes after restarting Velo. Navigation keeps the workspace and its form drafts in memory. Closing a dirty tab or deleting content requires confirmation. Unapplied JSON edits must be applied or explicitly discarded before switching collections.

Use **Edit collection JSON, defaults, environments, and extractions** to rename collections and folders, edit usage notes, configure defaults, and attach extraction rules. Applying JSON validates the complete document before replacing the current draft. Keep IDs stable when editing an existing collection.

## Inheritance and environments

Collection and folder defaults provide ordered headers, authentication, and variables. Inner folders override outer folders. Request headers override inherited headers by name, and a request's explicit authentication takes precedence. When a request uses `None`, the **Inherit authentication** setting controls whether defaults supply authentication. An explicit Authorization header suppresses inherited authentication.

Variables resolve in this order, with later scopes taking precedence:

1. Collection defaults.
2. Outer folders, then inner folders.
3. Request variables.
4. The selected environment.
5. Prepared dataset values.

Use `{{name}}` for variables, `{{secret:name}}` for OS credential references, and `{{session:name}}` for extracted session values. Missing variables, unavailable attachments, and invalid credentials fail before the affected request sends traffic. The masked resolved preview shows effective request settings without sending traffic.

## Authentication

The shared HTTP editor supports Basic, Bearer, and API-key authentication. API keys require an explicit header or query placement. Store sensitive values with **Secrets**, then reference them in the request. Collection persistence and portable export require secret references in known sensitive fields rather than literal credentials.

Authentication controls, explicit Authorization/Proxy-Authorization/Cookie headers, and recognized credential fields such as `x-api-key` or `access_token` require HTTPS for remote targets. Cleartext authentication is allowed only on literal loopback IP addresses, such as `127.0.0.1` or `[::1]`, for local testing. OAuth tokens sent as Bearer credentials follow the same target policy. This protection covers recognized authentication inputs; arbitrary custom data fields can still contain sensitive information. Query API keys can appear in server logs and recorded URLs even over HTTPS, so prefer header placement when the API supports it.

The OAuth control supports client credentials and authorization code with PKCE. Acquire a token explicitly before sending a request. Authorization code opens the system browser and uses a temporary loopback callback with state verification. Cancel pending acquisition when it is no longer needed. Refresh and forgetting local tokens are explicit actions; sending a request does not silently acquire or refresh tokens. Tokens stay in the OS credential store, and the editor receives a Bearer secret reference.

Token and authorization endpoints require HTTPS without URL credentials or fragments. Static endpoint query parameters, such as tenant settings, remain intact; parameters that conflict with OAuth protocol fields are rejected. PKCE providers must support the native `http://127.0.0.1:<random-port>/oauth/callback` redirect. Confidential clients use HTTP Basic client authentication; public PKCE clients can omit the client secret. Stored refresh credentials bind to the token endpoint, issuer, and client ID.

Authorization-code flows require the provider's exact issuer identifier and an `iss` callback parameter as specified by RFC 9207. Velo checks the issuer before exchanging an authorization code. Providers that omit this parameter are rejected. Closing a request tab, switching away from it, or leaving its cached workspace cancels pending OAuth work. A delayed acquisition or completion cannot apply a token to an inactive or reopened tab.

If an OS credential dialog is already open, cancellation waits for that operation and its cleanup to finish. Dismiss the dialog to proceed. A cancelled local write restores the previous record or removes the new record; if the OS refuses rollback, Velo reports a cleanup failure and the local token must be forgotten explicitly. Provider-side effects cannot be rolled back locally. Cancelling a refresh after the provider rotates its token may require acquiring a new token.

## Send and inspect

Choose an environment, confirm authorization, and select **Send active request**. Editing or opening a request never sends it. Each send performs one logical request with the configured same-origin redirect policy and no automatic retries. Sending again requires another action.

Cancel API execution to stop owned work and prevent subsequent requests. API operations share desktop execution admission with load testing. Stop active load before sending API traffic, so functional requests do not silently disturb a performance measurement.

The response view shows HTTP status, measured total elapsed time, received bytes, headers, cookies, and bounded response content. Total elapsed time includes redirects and bounded body consumption. DNS, TLS, and connection timings are unavailable. Received bytes may be a lower bound when capture stops at its limit. The viewer identifies truncated bodies, binary content, invalid UTF-8, and unsupported encodings.

Response content appears as escaped text. HTML and scripts returned by a target do not execute. Use formatted JSON or raw text, search the captured body, and explicitly copy or export the redacted body.

## Assertions, extraction, and sessions

Attach status, header, JSON-field, or text assertions with the shared assertion editor. Results list each check separately. Functional assertions do not evaluate load thresholds or throughput.

Configure each request's `extractions` in collection JSON. For example:

```json
{
  "name": "order_id",
  "source": { "kind": "json", "pointer": "/id" },
  "required": true,
  "overwrite": false
}
```

Later requests can reference `{{session:order_id}}` in supported fields. Extraction uses the same bounded JSON, header, and cookie rules as scenarios. Missing required fields, invalid JSON, inspection limits, and attempted overwrites produce explicit errors. Required extraction failure does not leave stale values available to dependent requests.

Individual sends share an in-memory session isolated by collection and environment. Inspect masked cookies and extracted variable names in **Interactive session**, or clear both explicitly. Collection runs use independent sessions. Sessions disappear when Velo exits. Velo retains at most 32 interactive collection/environment sessions; clear an existing session before adding another when the limit is reached. Deleting a collection clears its interactive sessions.

## Run a collection

Select an entire collection or toggle folders and requests with **Select for run**. Selected entries execute in the collection's displayed order. Configure the environment, iteration count, delay between requests, and stop-on-failure policy before authorizing execution. The initial runner is sequential and bounded. It does not retry failures.

Use **Shared test data** on a request to configure CSV, JSON, or Faker data with the existing recipe controls. Import CSV/JSON files or prepare Faker records, then run with the prepared rows. Prepared data stays in memory. Recipe allocation, lifetime, and exhaustion settings use shared test-data behavior. Functional execution has one sequential worker: run/user lifetime keeps one row, and iteration lifetime allocates the next row per collection iteration. A stop-on-exhaustion recipe records later steps as skipped; fail-on-exhaustion fails preflight before traffic.

Progress shows completed, failed, cancelled, and skipped outcomes. Stopping prevents later requests from starting. Exported evidence retains the `functional` run kind, request identities, per-request checks, cancellation state, and redacted responses. It is distinct from performance reports.

## History and redaction

Functional API history has separate retention and deletion controls. Choose a maximum retained run count, or zero to disable retention. Response content persistence is opt-in. Enabling it retains only bounded redacted content. Deleting history does not delete saved collections or OS credentials.

Known sensitive headers, resolved secrets, and known request values are redacted before responses reach persistence or export. Configure collection `sensitive_response_fields` with JSON pointers for additional response fields, such as `/access_token` or `/user/email`. Copy and export use the redacted response shown in the viewer. Portable collections retain variable and secret references, clear machine-local file paths, and do not embed resolved credentials. Rebind attachments after import.

Restore a history request to open its authored snapshot in a new unsaved collection. Existing drafts remain intact, and restoration never sends traffic. Snapshots retain variable and secret references, not resolved credentials. Rebind local attachments before sending a restored request. Inspecting history evidence never resends a request.

## Reuse in load tests

Select **Open in load-test editor** to reuse the request's shared definition. It opens Tests without starting load. Review workload, budgets, target authorization, and any machine-local attachments before starting.

Select **Review as load scenario** to convert the selected ordered flow for review. Conversion reports settings the scenario contract cannot represent instead of silently dropping them. Review the resulting scenario and authorize load separately. Functional sessions and performance virtual-user sessions remain independent.

The [Rust audit](api-rust-audit.md) records the guidelines applied to this feature and the resulting fixes.

The [authentication security audit](authentication-security-audit.md) records the protocol checks, findings, fixes, and verification limits.
