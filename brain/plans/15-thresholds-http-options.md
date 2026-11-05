# Thresholds and remaining HTTP options

- Milestone 6 follows [[plans/14-arrival-rate]]. Implement separate working increments in this order; assertions and persistence remain excluded.
- [x] Final thresholds: optional rules for response p95 (ms), HTTP failure rate (percent), and terminal request throughput (requests/s), with explicit strict less-than/greater-than operators. Validate finite nonnegative limits, failure percentages up to 100, and latency limits up to one hour. Charge rules to the request-data budget.
- Evaluate only terminal cumulative snapshots after worker cleanup, without changing HTTP counters or execution state. Preserve cancelled/failed run context. Empty required populations, counter overflow, and incomplete response histograms produce insufficient data, never a pass. Throughput requires at least one non-cancelled terminal attempt and positive elapsed time.
- Expose Formisch controls, metric populations, units, operators, observed values, and individual verdicts. No configured rules means no threshold verdict. Native configuration remains authoritative; regenerate bindings.
- [x] Add PUT, PATCH, DELETE, HEAD, OPTIONS with explicit body semantics and local-server coverage.
- [x] Add Bearer/Basic authentication controls, safe errors and secret-free reports; reject conflicting authorization sources.
- [x] Add URL-encoded form bodies with duplicate field support and local-server coverage.
- [x] Add streaming multipart/file uploads, native file selection, bounded buffers, and cleanup tests for success/cancellation.
- [x] Add HTTP negotiation/forced protocol and bounded redirect controls; test redirect limits and negotiated/forced protocols locally.
- Threshold verification: exact strict boundaries, missing populations, partial runs, invalid limits, integrity warnings, live versus terminal snapshots, local-server outcomes, frontend validation, binding drift. Then check → typecheck → test → build; lint and rustdoc.
- HDR values remain approximate; comparisons use unrounded native values. Reference: [HDR implementation](https://github.com/HdrHistogram/HdrHistogram_rust/blob/main/README.md).

## Evidence

- All milestone 6 increments are implemented and verified locally on macOS arm64.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Built the macOS application and `Velo_0.0.0_aarch64.dmg`.
- Passed `pnpm lint`, `pnpm bindings:check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `git diff --check`.
- Passing tests: 78 frontend tests, 15 engine unit tests, 24 local-server integration tests, 32 engine doctests, three native command tests, and four desktop/shared-contract doctests.
- Targeted tests pass for thresholds, methods, auth validation and secrecy, form encoding, duplicate fields, multipart streaming/replay, source changes, redirect method rewrites/limits/bytes, total deadlines, and HTTP/1.1 versus HTTP/2.
- The cancellation test initially exposed a retained upload handle. Readers now have owned tasks, bounded channels, cancellation, and joins; the macOS descriptor check passes after a stalled 128-MiB upload is cancelled.
- No interactive packaged-app request-options check, Windows/Linux run, remote CI, signing, notarization, or capacity benchmark is claimed.

## HTTP implementation decisions

- GET/HEAD reject bodies; POST/PUT/PATCH/DELETE/OPTIONS support text, JSON, URL-encoded pairs, and multipart. Duplicate pair/part order is preserved.
- Basic/Bearer credentials are validated once, Authorization conflicts rejected, and auth debug output redacted. Secrets, paths, and body content never enter summaries/events/errors.
- Auto protocol retains negotiation; forced HTTP/1.1 and HTTP/2 use Reqwest policies. Cleartext h2 requires prior knowledge. Redirects default off, remain same-origin, reject credential URLs/downgrades, and follow HTTP method/body rewrite rules. Intermediate response bytes count; a single timeout spans all hops and source setup.
- Native file selection uses the existing dialog dependency without granting generic frontend filesystem access. Preparation checks regular readable files off the desktop async/UI thread. Iterations reopen files, check length/modification metadata, and require unchanged sources.
- Multipart streams use bounded one-chunk channels and 16-KiB chunks. Two buffers per file per VU are charged to the existing byte budget. Owned upload tasks close files after pending disk operations, are joined between attempts/hops, and are cancelled/joined before final publication. No full-file retention, request backlog, or new persistence.
- Enabled Reqwest form/multipart/stream and Tokio filesystem features; added tokio-stream for the bounded receiver adapter and tempfile for tests. Existing Tokio, Reqwest, and Tauri dialog provide the rest.
- Official references: [Reqwest multipart](https://docs.rs/reqwest/latest/reqwest/multipart/struct.Part.html), [Tauri file dialogs](https://v2.tauri.app/plugin/dialog/).
