# Implementation history

I built Velo in working increments: typed native communication, a standalone HTTP engine, live monitoring, additional workloads, HTTP options, and local persistence. The following dated decisions and verification records describe those increments; current behavior is documented in the implementation guides. Earlier evidence predates later fixes and is not a release certification.

## Foundation and typed native communication

- Root Cargo workspace owns both crates and the lockfile. Engine has no dependencies; its identity API proves the crate boundary without fake execution.
- ts-rs 12.0.1 generates Serde-compatible DTOs through `pnpm bindings:generate`; `pnpm bindings:check` compares formatted output without rewriting tracked contracts.
- Runtime API is @tauri-apps/api 2.12.4. Vitest 5.1.0 runs injected transport tests without module mocking. Thiserror 2.0.21 and tracing 0.1.44 support native errors and diagnostics.
- Tracing owns native logging; the starter log plugin was removed to prevent competing global logger initialization during development launch.
- Diagnostic labels use 1–64 Unicode code points after trimming. No numeric counters cross IPC yet. Future u64 counters/durations will serialize as decimal strings with matching generated TypeScript strings; chart conversions must check numeric range.
- Runtime font assets and their OFL license are bundled locally. CSP permits local assets and Tauri IPC; the unused broad asset filesystem protocol was removed.
- Native metrics/events remain deferred to milestones 2–3 as agreed. Active CI targets macOS only; Windows/Linux verification is deferred to milestone 8.
- Official references: [ts-rs](https://docs.rs/ts-rs/12.0.1/ts_rs/) and [Tauri commands](https://v2.tauri.app/develop/calling-rust/).

### Foundation verification evidence, 2026-11-05

- `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed on macOS arm64. `pnpm lint` passed with two preserve-caught-error warnings where the UI adapter intentionally replaces infrastructure details with a safe error message.
- Four frontend contract tests, two native diagnostic tests (also compiled in the exporter target), and both public-API doctests pass. `cargo test -p velo-engine` passes independently.
- `pnpm bindings:check` passes. Formatting and `git diff --check` pass.
- Development launch passed after removing the duplicate global logger. Route templates have single roots for Nuxt transitions.
- Final application and DMG were generated in `target/release/bundle/`. An initial DMG attempt encountered a busy temporary disk image; detaching that generated image and rebuilding succeeded.
- Packaged app launched, Settings navigation worked through the native accessibility tree, and Check connection returned `Connected to velo-engine 0.0.0.`. Shell screenshot was inspected during development.
- Windows/Linux builds and interactive checks are deferred to milestone 8; active CI targets macOS only. No remote CI run, signing, or notarization is claimed.

## Milestone 2: constant-VU engine

### Decisions

- Reqwest 0.13.5 uses rustls with the platform certificate verifier and HTTP/1.1 + HTTP/2 negotiation. Tokio 1.53.1, tokio-util 0.7.19, and HDR Histogram 7.6.0 implement execution, stop signalling, and distributions. Axum 0.8.9 is test-only, with HTTP/1 and Tokio features. Versions are locked in the workspace lockfile.
- Engine-owned contracts derive Serde and ts-rs without a Tauri dependency. Explicit string overrides match the decimal u64 serializer. ts-rs's `no-serde-warnings` feature handles that intentionally custom serialization; serialization fixtures and binding drift checks verify the wire contract.
- Admission and observation use a short shared critical section. There is no observation queue to drop or back up. Latency stops before observation aggregation. Lock wait totals and a >1-ms interference flag expose contention. Cumulative snapshots are bounded; interval snapshots and events remain milestone 3.
- A JoinSet owns workers. Attempt guards reconcile cancelled and partially read requests; worker guards track ownership. The supervisor aborts at grace expiry and joins every worker before publishing terminal state. Explicit shutdown also joins the supervisor.
- The native exit path blocks admission and repeated exit requests until shutdown completes. Interactive confirmation remains milestone 3, before runnable UI controls are added.
- Request-data budgets count normalized URL/header/body bytes and pair overhead across the template and possible VU clones. This conservatively counts shared bodies per VU and does not claim to bound total resident memory.
- Official references: [Reqwest client controls](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html), [Tokio JoinSet](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html), [HDR Histogram](https://docs.rs/hdrhistogram/7.6.0/hdrhistogram/struct.Histogram.html), and [cancellation tokens](https://docs.rs/tokio-util/0.7.19/tokio_util/sync/struct.CancellationToken.html).

### Verification evidence, 2026-11-05

- macOS arm64: `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed. `pnpm lint` passes without warnings after aligning generated array notation with repository style.
- Workspace tests cover ten local-server integration cases, two engine metric tests, native IPC dispatch, existing diagnostics, nineteen engine doctests, the desktop doctest, and four existing frontend tests. `cargo test -p velo-engine --locked` also passes independently. No tests contact external targets.
- `pnpm bindings:check`, `git diff --check`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, and engine docs with missing-doc warnings enabled pass. The engine's normal dependency tree contains no Tauri dependency.
- The application and DMG were generated under `target/release/bundle/`. The packaged executable launched and exited with status zero after a native quit request. This is an idle launch/exit smoke check; active execution is verified through local-server engine tests and native IPC dispatch tests.
- No remote CI result, sustained-load benchmark, signing, notarization, or Windows/Linux verification is claimed. Editor controls, interactive authorization/exit confirmation, aggregate events, and charts remain milestone 3.

## Live monitoring and first usable release

### Decisions

- Events default to one second; the execution view offers 500 ms, 1 s, and 2 s updates. Native validation accepts only 250–10,000 ms.
- UI retains the latest run only. Request secrets remain in the in-memory editor; events and summaries contain safe origin/workload metadata.
- Exact counters stay decimal strings; chart rates use bounded interval deltas. Histograms have fixed allocation and reset after each aggregate sample.
- Native emission failures do not fail HTTP execution. Recovery reads restore totals and terminal state without reconstructing missed chart measurements.
- macOS AppKit termination is routed through the native exit handler using the existing Objective-C runtime dependencies. The delegate's optional method is installed once and its registration refreshed; startup fails with a stable code if the framework already supplies that method. Recheck this integration on Tauri upgrades. Background: [Tauri termination support](https://github.com/tauri-apps/tauri/issues/12978).
- Exit confirmation is attached to the main window. Cancelling restores admission; confirmed exit waits for workers and the aggregate publisher to be joined.

### Verification evidence, 2026-11-05

- Installed dependencies with `pnpm install` before typechecking. Added uPlot 1.6.32 and the Rust dialog plugin 2.8.0; macOS Objective-C dependencies reuse already locked runtime versions.
- Frontend tests cover request conversion/validation, subscription ordering, overlapping actions, exact counter strings, stale runs/sequences/lifecycle states, gaps, terminal recovery, subscription retry, and cleanup. A test with nearly 100,000 updates keeps at most 600 chart points.
- Engine local-server tests cover interval outcome reconciliation, histogram reset, monotonic duration accounting, stale publisher rejection, and terminal cleanup. Desktop tests exercise real command dispatch and aggregate delivery using Tauri's test runtime.
- Packaged macOS checks use an isolated HTTP server bound to 127.0.0.1:8080. The editor prepares and starts real requests; all four charts receive native measurements. Navigating to Dashboard and Results preserves the run and charts.
- A two-minute packaged run completed naturally and displayed the final summary. A subsequent stopped run displayed partial results with zero active VUs. Native/WebKit RSS samples at ten-second intervals showed no upward growth during the local run; this is a short observation, not a sustained-load benchmark.
- Packaged native Quit reaches the active-run confirmation. The Keep running choice dismisses the attached sheet and preserves the same run for both native Quit and the red window-close control. Confirmed exit logs cleanup and the packaged executable exits with status zero.
- DMG packaging intermittently encountered a busy temporary disk image on this host. Ejecting the task-created image and rerunning the original build succeeded; no packaging checks were bypassed.
- Windows/Linux builds and interaction checks, signing/notarization, and sustained-load capacity benchmarks remain milestone 8. Local observations do not establish a machine capacity guarantee.
- Passed the required sequence: `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Also passed `pnpm lint`, `pnpm bindings:check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Frontend suite: 33 tests; engine integration suite: 11 tests plus unit tests and workspace doctests.

## Milestone 4: ramping virtual users

### Decisions

- Use a tagged workload contract. Constant workloads define VUs and duration. Ramps define starting VUs and ordered stages with a positive duration and nonnegative target.
- Interpolate from each stage's starting count to its target using monotonic elapsed time. Round toward the starting count until the next whole-VU transition. Stage endpoints are exact, including zero-VU stages.
- Wake at the next whole-VU transition or stage boundary. Delayed scheduler wakes reconcile directly to the current target without replaying missed transitions.
- Retiring workers finish the current request and exit. Retirement interrupts pacing and prevents another request. Retirement does not cancel a response.
- Count retiring workers against the peak workload allocation. If slow retirees occupy available slots, defer new workers until slots are joined. Expose desired, actual, and retiring VUs separately.
- Native validation checks stage durations, summed duration representation, peak VUs, and stage configuration memory within the explicit request-data budget before starting.
- Reuse the existing Formisch editor and bounded uPlot charts. Review shows the full native-validated stage schedule before authorization.

### Verification evidence

- Deterministic tests cover ramp-up/down boundaries, zero/hold stages, delayed sampling, nanosecond rounding, and maximum duration/count representations. Paused Tokio time verifies zero-worker scheduling and cancellation before a future ramp.
- Local-server tests hold responses behind an explicit gate to prove graceful retirement, no readmission by retirees, deferred ramp-up within the peak allocation, and zero owned/in-flight work after stop. Slow-response tests cover natural completion and repeated runs; a pacing test proves retirement wakes idle workers without another iteration.
- Frontend tests cover stage conversion and validation, duration overflow, peak budgets, inactive field edits, and distinct desired/actual chart values with gaps. Native command tests check ramp confirmation serialization.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` on macOS arm64. Also passed `pnpm lint`, `pnpm bindings:check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Frontend: 44 tests; engine: seven unit tests, sixteen local-server integration tests, and twenty-three doctests; desktop command and diagnostic tests also pass.
- Packaged app verification used a temporary loopback server with 50-ms responses. The editor reviewed a zero-start ramp to four VUs over four seconds, then to zero over ten seconds. It completed 559 successful requests with zero cancellations and zero in flight, peak requested/actual VUs of 4/4, and 14.017 seconds actual duration. Dashboard/Results navigation preserved the run; both VU chart series and the final summary rendered correctly. Screenshots and native accessibility values were inspected locally. This is a functional smoke check, not a capacity measurement.
- macOS is the available platform; Windows/Linux gates remain milestone 8. No remote CI, signing/notarization, or capacity benchmark is claimed.

- Scheduling uses Tokio monotonic deadlines and cancellable task joins: [sleep_until](https://docs.rs/tokio/1.53.1/tokio/time/fn.sleep_until.html), [JoinSet](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html). Stage controls use the existing [Formisch FieldArray](https://formisch.dev/vue/api/FieldArray/). No dependencies were added.

## Constant arrival rate

### Verification evidence

- Completed on macOS 27.0 arm64. No dependencies added.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Also passed `pnpm lint`, `pnpm bindings:check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `git diff --check`.
- Frontend: 52 tests. Engine: nine unit tests, nineteen local-server integration tests, and twenty-four doctests. Desktop command tests cover arrival confirmation through actual IPC dispatch.
- Paused time proves fractional deadlines, duration endpoint exclusion, maximum rate representation, and skipping a 500-ms delayed wake without replay. A fast-target test caught a duration-end loop after all slots were processed; the exit condition was fixed and the test passes.
- A stalled-target test offers 250 slots at 1,000/s with capacity three. Exactly three requests start and are cancelled after zero grace; remaining slots reconcile as capacity drops or missed deadlines. Final owned tasks and in-flight requests are zero. State is bounded by capacity, fixed histograms, and scalar counters; no sustained-memory benchmark is claimed.
- Packaged `target/release/bundle/macos/Velo.app` launched. Using macOS accessibility controls, selected constant arrival rate, reviewed 10 iterations/s with capacity ten for two seconds, and ran against a temporary loopback target with 100-ms responses. Results showed twenty successful responses, forty payload bytes, zero drops, zero in flight, peak actual concurrency two, 2.008 seconds actual duration, and maximum scheduler lateness 2,067 μs. Native confirmation and Results navigation worked. This is a functional smoke check, not a capacity measurement.
- Built both the app and `target/release/bundle/dmg/Velo_0.0.0_aarch64.dmg`. Windows/Linux verification remains milestone 8. No remote CI, signing, notarization, or benchmark evidence is claimed.

## Thresholds and remaining HTTP options

### Evidence

- All milestone 6 increments are implemented and verified locally on macOS arm64.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Built the macOS application and `Velo_0.0.0_aarch64.dmg`.
- Passed `pnpm lint`, `pnpm bindings:check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `git diff --check`.
- Passing tests: 78 frontend tests, 15 engine unit tests, 24 local-server integration tests, 32 engine doctests, three native command tests, and four desktop/shared-contract doctests.
- Targeted tests pass for thresholds, methods, auth validation and secrecy, form encoding, duplicate fields, multipart streaming/replay, source changes, redirect method rewrites/limits/bytes, total deadlines, and HTTP/1.1 versus HTTP/2.
- The cancellation test initially exposed a retained upload handle. Readers now have owned tasks, bounded channels, cancellation, and joins; the macOS descriptor check passes after a stalled 128-MiB upload is cancelled.
- No interactive packaged-app request-options check, Windows/Linux run, remote CI, signing, notarization, or capacity benchmark is claimed.

### HTTP implementation decisions

- GET/HEAD reject bodies; POST/PUT/PATCH/DELETE/OPTIONS support text, JSON, URL-encoded pairs, and multipart. Duplicate pair/part order is preserved.
- Basic/Bearer credentials are validated once, Authorization conflicts rejected, and auth debug output redacted. Secrets, paths, and body content never enter summaries/events/errors.
- Auto protocol retains negotiation; forced HTTP/1.1 and HTTP/2 use Reqwest policies. Cleartext h2 requires prior knowledge. Redirects default off, remain same-origin, reject credential URLs/downgrades, and follow HTTP method/body rewrite rules. Intermediate response bytes count; a single timeout spans all hops and source setup.
- Native file selection uses the existing dialog dependency without granting generic frontend filesystem access. Preparation checks regular readable files off the desktop async/UI thread. Iterations reopen files, check length/modification metadata, and require unchanged sources.
- Multipart streams use bounded one-chunk channels and 16-KiB chunks. Two buffers per file per VU are charged to the existing byte budget. Owned upload tasks close files after pending disk operations, are joined between attempts/hops, and are cancelled/joined before final publication. No full-file retention, request backlog, or new persistence.
- Enabled Reqwest form/multipart/stream and Tokio filesystem features; added tokio-stream for the bounded receiver adapter and tempfile for tests. Existing Tokio, Reqwest, and Tauri dialog provide the rest.
- Official references: [Reqwest multipart](https://docs.rs/reqwest/latest/reqwest/multipart/struct.Part.html), [Tauri file dialogs](https://v2.tauri.app/plugin/dialog/).

## Persistence, credentials, history, and comparisons

### Policies

- SQLite through rusqlite with bundled SQLite, in the application data directory. An OS file lock prevents another instance from recovering live records. One connection owned by a bounded worker; no storage calls on the request measurement path.
- Saved configurations omit URL path/query/userinfo, headers, query pairs, authentication and bodies by default. Names are user-authored and must not contain secrets. Re-enter omitted fields before execution.
- Explicit remember stores the complete request configuration in the OS credential store through keyring; SQLite holds only the reference and redacted configuration. Store failure requires re-entry; never write plaintext as a fallback.
- Summaries remain until explicit deletion. History pages contain 25 rows; comparisons load at most two selected records and require matching origin, method, and workload kind. Partial/interrupted runs are identified.
- Each run retains at most 600 aggregate intervals. At the bound, retain every second point and double the sampling stride. Display stride and missing-interval status; never interpolate missing observations. No individual request records.
- Bounded writer queue of 32 aggregate updates; overflow marks history incomplete without delaying engine sampling. Finalization drains queued updates before exit. Write failures are visible independently of execution success.
- Each interval is one transaction; no batch spans multiple intervals. Startup marks unfinished records interrupted. SQLite transactions keep snapshot/series metadata consistent; deletes cascade to series. No automatic summary expiry.

### Implementation

- [x] Native storage, recovery, deletion, compaction, bounded writer, safe errors.
- [x] Explicit OS credentials, saved configuration save/load/delete and validation.
- [x] Nuxt saved configuration controls, paginated history, selected summaries/charts and comparisons.
- [x] Persistence/credential failure tests and full repository verification.

### Dependencies

- rusqlite (bundled SQLite): https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html
- keyring 3 (native stores, no plaintext fallback): https://docs.rs/crate/keyring/3.6.0

### Local verification

- 2026-11-06, macOS arm64: `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed. `pnpm lint`, `pnpm bindings:check`, and `git diff --check` passed.
- Tests passed: 55 Rust unit/integration tests, 38 Rust doctests, and 78 frontend tests. New coverage includes redaction, unavailable credentials, corruption, transaction rollback, read-only writes, bounded compaction, cascade deletion, pagination, database reopen, exclusive ownership, queue overflow, finalization, and native IPC save/load.
- Built `target/release/bundle/macos/Velo.app` and `target/release/bundle/dmg/Velo_0.0.0_aarch64.dmg`.
- Real OS credential prompts and interactive UI behavior were not exercised. Credential behavior uses injected stores. Windows/Linux and remote CI remain unverified; milestone 8 remains open.

## Release hardening and benchmarks

### Reproducible engine baseline

Run from a clean checkout, with no other tests competing for resources:

```sh
cargo test --locked --release -p velo-engine --test execution release_benchmark -- --ignored --exact --nocapture --test-threads=1
```

The opt-in harness starts an ephemeral loopback HTTP target and runs one five-second warmup followed by three thirty-second measurements at sixteen constant VUs. Each request is GET, with a seven-byte `payload` response, default HTTP negotiation, pooled connections, no pacing, and the default timeout/drain settings. Every repetition verifies terminal cleanup and accounting. The JSON snapshot includes durations, rates, latency populations, failures, and integrity indicators. Warmup output must be excluded from results.

Capture the full output, Git SHA, lockfile hash, Rust version, OS version, CPU model/core count, RAM, power mode, and background workload. Sample generator process RSS and CPU externally throughout the run. The target runs in the same process and records request details behind mutexes; these results measure a combined loopback baseline, not isolated generator capacity or remote target throughput. Use a separately provisioned target and report its resource use before making capacity claims. No comparison to another load engine is established by this harness.

### Evidence ledger

- Windows/Linux CI and installer retention are configured. No remote run or clean-machine installation has been performed in this session.
- Existing $0 Actions budget remains unchanged. Exhausted private-repository minutes can prevent CI from starting.
- Public release readiness remains pending platform execution, interactive acceptance, signing decisions, and sustained resource measurements.

### Local verification, 2026-11-06

`pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed on macOS 27.0 arm64 (Apple M1, 8 GiB RAM). `pnpm lint` and `pnpm bindings:check` passed. The benchmark is ignored in normal tests and must be invoked explicitly.

The build produced Velo.app and Velo_0.0.0_aarch64.dmg. Opening the packaged app succeeded and its native process remained running. This is launch evidence only; it does not establish interactive acceptance or clean-machine installation. DMG SHA-256: `11eeed3ac7d5abf5e7595759f02e8b7620b5445b2f5684a7e4538f0047a3db7c`.

The release benchmark passed its warmup and all three measured repetitions, with completed state, reconciled counters, zero active workers, and zero in-flight requests at each terminal snapshot. Rust was 1.99.1; the base commit was `5d54de6269494f05a9564d46374018f25974f64f` with the milestone changes in the working tree. Cargo.lock SHA-256 was `8206b08703ca12183d9db98e8714891c46e502beac78be1f9659d69caadf9317`. Raw local output is `/tmp/velo-release-benchmark.log` (temporary, not a published capacity report). Power mode and continuous resource sampling were not captured; these results establish harness execution and cleanup, not sustained capacity.

#### Packaged macOS acceptance follow-up

[Desktop acceptance and resource evidence](evidence/macos-2026-11-06/acceptance.md) records a ten-minute eight-VU desktop run, five cancellation cycles, navigation/diagnostics/window controls, saved-request and Keychain round trips, history comparison/deletion after restart, active-exit confirmation, warm startup, and sampled native/WebKit resources. Temporary data was removed. This adds macOS acceptance and stability smoke-test evidence; clean-machine installation, longer soak testing, and the other platform gates remain open.

## Rust audit corrections

I followed the desktop acceptance with a Microsoft Rust Guidelines audit. Corrections added redacted Debug implementations, independent credential admission, durable cleanup recovery, bounded history-finalization waits, a fatal panic policy, safe diagnostic categories, and inherited workspace settings. Regression tests cover blocked credential access, SQL/Keychain partial failures, restart recovery, panic termination, and diagnostic redaction. Local formatting, typechecking, tests, lint, binding checks, Rust documentation, and packaged builds passed after the fixes.
