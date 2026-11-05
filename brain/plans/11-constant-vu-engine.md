# Milestone 2: constant-VU engine

## Scope

- Deliver GET/POST execution and native start, stop, and current/final state commands. Request editing, charts, and events remain milestone 3.
- Keep all HTTP execution, validation, metrics, and worker ownership in `velo-engine`, independently testable without Tauri.
- Use one pooled client per run, disabled redirects, no retries, no cookie store, system certificate validation, and streamed encoded response bodies. Count bytes actually consumed, including partial bodies; exclude headers and transport overhead. Redirect configuration remains milestone 6.
- Require authorization acknowledgement on every start. Return a safe confirmation summary containing target origin, VUs, duration, budgets, and high-load warnings before execution. Never return credentials, query values, headers, or request bodies in diagnostics.
- Default to a 30-second timeout, zero pacing, five-second drain, a 256-VU budget, and an 8-MiB normalized request-data budget across the template and VU clones. Budgets are explicit and adjustable; exceeding them rejects the run before worker creation. Warn at 100 VUs.
- Bound workers by the configured VU budget, normalized request data and VU clones by the request-data budget, and metrics by three fixed HDR histograms. No observation queue, per-request history, or chart storage.
- Serialize u64 counters and microsecond durations as decimal strings. Histogram precision is three significant digits over 1 μs–1 hour. Report out-of-range observations and aggregation interference.

## Implementation tasks

1. [x] Add configuration validation, safe errors, confirmation, and explicit resource budgets.
2. [x] Add fixed-memory outcome metrics, separate latency populations, rates, and integrity reporting.
3. [x] Add pooled sequential VUs, pacing, duration, stop/drain cancellation, joined ownership, and one-run lifecycle.
4. [x] Prove construction, classification, streaming, limits, stop races, repeated runs, and stalled-target cleanup with local-server tests.
5. [x] Add thin desktop commands and generated execution contracts without exposing runnable UI controls.
6. [x] Run the required checks and lint, binding drift, independent engine tests, and rustdoc checks. Record evidence and limitations.

## Decisions

- Reqwest 0.13.5 uses rustls with the platform certificate verifier and HTTP/1.1 + HTTP/2 negotiation. Tokio 1.53.1, tokio-util 0.7.19, and HDR Histogram 7.6.0 implement execution, stop signalling, and distributions. Axum 0.8.9 is test-only, with HTTP/1 and Tokio features. Versions are locked in the workspace lockfile.
- Engine-owned contracts derive Serde and ts-rs without a Tauri dependency. Explicit string overrides match the decimal u64 serializer. ts-rs's `no-serde-warnings` feature handles that intentionally custom serialization; serialization fixtures and binding drift checks verify the wire contract.
- Admission and observation use a short shared critical section. There is no observation queue to drop or back up. Latency stops before observation aggregation. Lock wait totals and a >1-ms interference flag expose contention. Cumulative snapshots are bounded; interval snapshots and events remain milestone 3.
- A JoinSet owns workers. Attempt guards reconcile cancelled and partially read requests; worker guards track ownership. The supervisor aborts at grace expiry and joins every worker before publishing terminal state. Explicit shutdown also joins the supervisor.
- The native exit path blocks admission and repeated exit requests until shutdown completes. Interactive confirmation remains milestone 3, before runnable UI controls are added.
- Request-data budgets count normalized URL/header/body bytes and pair overhead across the template and possible VU clones. This conservatively counts shared bodies per VU and does not claim to bound total resident memory.
- Official references: [Reqwest client controls](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html), [Tokio JoinSet](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html), [HDR Histogram](https://docs.rs/hdrhistogram/7.6.0/hdrhistogram/struct.Histogram.html), and [cancellation tokens](https://docs.rs/tokio-util/0.7.19/tokio_util/sync/struct.CancellationToken.html).

## Verification evidence, 2026-11-05

- macOS arm64: `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed. `pnpm lint` passes without warnings after aligning generated array notation with repository style.
- Workspace tests cover ten local-server integration cases, two engine metric tests, native IPC dispatch, existing diagnostics, nineteen engine doctests, the desktop doctest, and four existing frontend tests. `cargo test -p velo-engine --locked` also passes independently. No tests contact external targets.
- `pnpm bindings:check`, `git diff --check`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, and engine docs with missing-doc warnings enabled pass. The engine's normal dependency tree contains no Tauri dependency.
- The application and DMG were generated under `target/release/bundle/`. The packaged executable launched and exited with status zero after a native quit request. This is an idle launch/exit smoke check; active execution is verified through local-server engine tests and native IPC dispatch tests.
- No remote CI result, sustained-load benchmark, signing, notarization, or Windows/Linux verification is claimed. Editor controls, interactive authorization/exit confirmation, aggregate events, and charts remain milestone 3.
