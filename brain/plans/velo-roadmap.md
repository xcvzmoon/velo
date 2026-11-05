# Velo roadmap

## Status and scope

- Approved through the project interview on 2026-11-05. Phases 1 and 2 are implemented and verified locally on macOS; milestones 3–8 remain unstarted.
- Build a local, offline-capable desktop HTTP load-testing studio for macOS, Windows, and Linux.
- Follow the repository's [AGENTS.md](../../AGENTS.md): small correct changes, explicit boundaries, reliability before abstractions, and working increments.
- Preserve Nuxt 4, Vue Composition API, TypeScript, Tailwind 4, shadcn-vue/Reka UI, Valibot, theme tokens, and the existing native titlebar. Nuxt UI is superseded by the existing component stack.
- One test repeats one configured request. One run may be active per application instance.
- Exclude journeys, chaining, extracted variables, scripting, browser testing, distributed generators, cloud services, accounts, and external load engines.
- No artificial product limits on VUs, requests, or duration. Explicit local resource budgets and finite numeric representations still apply.
- First usable release is milestone 3: configure a request, execute constant VUs, monitor, stop, and inspect the final summary.
- Implementation backlog: [[todos]].

## Repository baseline

- Existing Tauri 2 shell uses Rust edition 2024, minimum Rust 1.90, and a client-rendered Nuxt application.
- Frontend currently has a topbar, theme toggle, Button, and ScrollArea. There are no product screens or request forms.
- No engine, runner commands, database, charts, product tests, or frontend test runner exists.
- `app/modules/tauri.ts` exposes CLI imports; it is not a runtime IPC client.
- `pnpm test` currently runs Cargo tests. `pnpm check` checks formatting; linting is a separate command.
- This baseline describes the repository before Phase 1.

## Architecture and directory boundaries

| Location                      | Responsibility                                                                        |
| ----------------------------- | ------------------------------------------------------------------------------------- |
| `app/pages/`                  | Dashboard, Tests, Results, Settings, and later execution views                        |
| `app/components/`             | Existing UI primitives and focused product components                                 |
| `app/composables/`            | Native invocation, subscription cleanup, bounded view state                           |
| `app/schemas/`                | Valibot input validation and UI-facing validation messages                            |
| `app/types/`                  | Generated contracts and global type declarations                                      |
| `crates/velo-engine/`         | Tauri-independent configuration, execution, scheduling, metrics, errors, cancellation |
| `src-tauri/src/commands/`     | Thin desktop command handlers                                                         |
| `src-tauri/src/`              | Native run ownership, event adapter, later storage and credentials                    |
| `brain/plans/velo-roadmap.md` | Approved scope, semantics, milestones, risks                                          |
| `brain/todos.md`              | Ordered implementation tasks and dependencies                                         |

- Use a root Cargo workspace containing the desktop crate and engine crate. Update scripts so engine tests, formatting, linting, and doctests are covered.
- Keep engine modules inside one crate until separate crates solve a demonstrated boundary problem. The engine must compile and test without Tauri or a webview.
- Choose a clean engine API over commands containing execution logic. A CLI is a future consumer, not an initial deliverable.
- Prefer generated Rust-owned contracts over duplicated TypeScript declarations. Select one maintained binding tool after checking official documentation and actual serialization support in milestone 1.
- Keep diagnostic transport DTOs in the desktop adapter; keep reusable engine contracts in the engine. Binding generation must not make the engine depend on Tauri.
- Prefer the existing UI system over replacing it or running two component libraries.
- Keep CPU-heavy aggregation and blocking storage work off the UI thread and latency measurement path.

## Dependency plan

Versions are selected during implementation from current official documentation, checked against the installed toolchains, and locked. This plan does not assert latest versions.

| Milestone | Dependency                                                                          | Purpose and boundary                                                                            |
| --------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| 1         | Existing Nuxt, Vue, Tailwind, shadcn/Reka, Valibot                                  | Shell, accessible controls, form validation                                                     |
| 1         | Existing Tauri and Serde; add runtime Tauri JS API                                  | Commands and serialization; CLI remains build tooling                                           |
| 1         | One Rust-to-TypeScript binding generator                                            | Reproducible command contracts and later event contracts                                        |
| 1         | Thiserror                                                                           | Typed engine and adapter failures with stable external codes                                    |
| 1         | Tracing                                                                             | Native structured diagnostics with secret-safe fields; coordinate with existing desktop logging |
| 1         | Vitest and Vue Test Utils if needed for the first meaningful frontend contract test | Test validation and native adapter behavior without module mocking                              |
| 2         | Tokio, Reqwest, HDR Histogram                                                       | Async bounded execution, pooled HTTP, bounded latency distributions                             |
| 2         | Tokio cancellation utilities if they simplify ownership                             | Explicit shutdown and joined task cleanup                                                       |
| 2         | Hyper through Reqwest; direct dependency only when needed                           | HTTP transport; avoid a second production networking implementation                             |
| 2         | Local HTTP server support in dev dependencies                                       | Deterministic integration tests, including stalled and failing responses                        |
| 3         | uPlot                                                                               | Bounded time-series charts with controlled updates                                              |
| 7         | One SQLite Rust library and one OS credential-store integration                     | Local summaries/configurations and explicit remembered credentials                              |

Do not add networking, charts, SQLite, or credential packages in Phase 1. Check existing dependency capabilities before adding a package.

## Communication contracts

### Foundation

- Rust owns Serde DTOs; TypeScript bindings are generated by a documented deterministic command and checked for drift in CI.
- The diagnostic command accepts a small validated payload and returns native application/engine identity. It must exercise the independent crate without sending HTTP traffic.
- Valibot validates UI input. Native validation remains authoritative at the IPC boundary; the engine accepts validated configuration rather than repeatedly parsing inside request loops.
- Invalid input returns a structured error envelope with a stable code, safe message, and optional field path. Distinguish configuration errors from internal failures without exposing secrets.
- Define numeric wire representations explicitly; Rust counters and durations must not silently lose precision as JavaScript numbers.
- Do not introduce fake run commands, synthetic performance data, or placeholder execution events.

### Execution, added in milestones 2 and 3

- Commands start a validated run, request stop, and retrieve authoritative current/final state. Start returns a run ID, not a promise held open until completion.
- Rust owns `idle`, `starting`, `running`, `stopping`, `completed`, `cancelled`, and `failed` transitions. Reject overlapping starts atomically, including start/stop races.
- Commands translate configuration and errors; the engine owns HTTP execution, scheduling, measurement, cancellation, and task cleanup.
- Events contain run ID, sequence, monotonic elapsed duration, cumulative counters, interval metrics, execution state, and saturation/integrity indicators. Default update interval is one second and is configurable.
- Subscribe before starting or recover through a state command. Ignore events for old run IDs, detect sequence gaps, and fetch final state if the final event is missed.
- Events carry aggregates only. Cumulative counters recover totals after missed UI updates; missing chart intervals remain visible rather than being fabricated.
- Navigation preserves runs. Unmounting removes listeners and charts. Application exit asks for confirmation during a run and then cancels and cleans up native work.
- Keep a bounded exact aggregation path. If backpressure delays scheduling, report generator interference. Never silently drop request observations to keep a chart responsive.

## Approved execution and metric semantics

### Request execution

- First release supports GET/POST, custom headers/query parameters, JSON/plain-text bodies, configurable timeout, and explicit redirect handling.
- Default timeout is 30 seconds; redirects default to disabled. Validate HTTP(S) URLs and reject malformed request settings before execution.
- Use system certificate validation, HTTP version negotiation, pooled keep-alive connections, no automatic retries, and no cookie jar.
- VUs share a connection pool and have no independent session identity. A VU is a sequential execution loop, not a separate client connection.
- One iteration is one logical request including configured redirects. Primary request counters count logical requests; redirect payloads contribute to body-byte totals. Document this distinction from individual network exchanges.
- Start latency immediately before HTTP execution; finish after fully consuming the body. Include connection acquisition, DNS, TCP/TLS work when needed, redirects, and body reading. Exclude subsequent aggregation and UI work.
- Stream and discard response bodies. Bound transient buffers and avoid retaining per-request results.
- Default HTTP success is final status 200–399. Separately count status failures, transport failures, timeouts, cancellations, and later assertion failures.
- Transport and timeout observations have separate duration statistics. Main response latency includes completed HTTP responses, including status failures; excludes transport failures, timeouts, and cancellations.
- Body-byte metrics describe application payloads, including redirects, and exclude headers, TLS, and retransmissions. Specify encoded/decoded behavior and partial-body accounting before implementing byte metrics; do not call them total wire traffic.

### Scheduling and lifecycle

- Constant VUs run sequential request loops for the scheduling duration. Optional delay between iterations defaults to zero; response speed controls achieved throughput.
- Use Tokio tasks with explicit bounded concurrency. No OS thread per VU, unbounded spawn loop, or pending-request backlog.
- Ramping stages interpolate desired VUs over stage duration. Retiring VUs finish their current request and exit. Report desired and actual VUs separately.
- Arrival-rate work uses evenly spaced monotonic deadlines. Count capacity rejections and missed deadlines separately as dropped iterations; never burst to catch up.
- Track scheduler lateness, dropped work, aggregation interference, active/max VUs, and in-flight requests. State when measurements are affected by generator saturation.
- Stop and duration expiry stop new iterations immediately. Drain in-flight requests for a configurable grace period, default five seconds, then abort remaining work and join owned tasks.
- Report scheduling duration, drain duration, and actual duration separately. Actual duration includes drain; the run is terminal only after cleanup.
- Configure resource budgets and high-load warning levels. Confirmation shows target and requested load and requires authorization acknowledgement. It does not verify ownership.
- A cancelled run retains its partial summary and cancellation counts; cancellation alone is not an engine failure.

### Counters, distributions, and rates

- Maintain requests started, completed HTTP responses, successful responses, status failures, transport failures, timeouts, cancelled requests, and in-flight requests.
- Accounting invariant: started = completed responses + transport failures + timeouts + cancelled requests + in-flight requests. In-flight is zero after cleanup.
- HTTP failure rate denominator is completed responses + transport failures + timeouts. Numerator is status failures + transport failures + timeouts. Drops and cancellations do not enter this denominator.
- Each iteration contributes once to its outcome. Later assertions may fail an iteration without changing its HTTP outcome; report assertion failures independently and never double-count overall failed iterations.
- Final request throughput is terminal non-cancelled attempts divided by scheduling plus drain time. Report successful-response throughput separately. Live charts use interval deltas and actual interval duration.
- Report completed/failed iterations and iteration rate separately. Define their relationship to later assertion outcomes before adding assertions.
- Produce total requests, successes/failures, error rate, body bytes, mean/min/max/median/p90/p95/p99 response latency, active/max VUs, iterations, drops, in-flight requests, and actual duration incrementally across milestones.
- HDR histograms use microseconds, three significant digits, and a supported range of 1 μs–1 hour. Constrain request timeouts to the supported range; record and surface out-of-range samples rather than silently clamping or omitting them.
- Percentiles are approximate. Include histogram population, precision, and out-of-range counts in documentation and summaries. Bound cumulative and interval histogram memory.
- Thresholds evaluate final cumulative metrics using explicit strict comparisons, such as p95 < 500 ms, error rate < 1%, and throughput > minimum.
- Empty required populations produce insufficient data and cannot pass. Cancelled/partial runs are visibly partial; evaluate against their observed population and retain that context.

## Milestones and acceptance gates

These eight milestones replace the original five broad phases. Each milestone depends on the preceding one; small implementation tasks remain independently reviewable. Later milestones are briefs, not permission to implement them during Phase 1.

### 1. Foundation and typed native communication

- Deliver the workspace/engine boundary, desktop navigation, generated contracts, structured errors, and real diagnostic round trip.
- Preserve native titlebar controls, dark styling, and keyboard usability. Dashboard, Tests, Results, Settings have honest empty states.
- Validate diagnostic input in Vue and Rust; show pending/success/failure states without module mocking in tests.
- Cover the independent API and diagnostic behavior; check binding drift and engine independence. Establish frontend testing only for meaningful behavior.
- Gate: required checks and lint pass; engine tests run independently; packaged app launches and completes the diagnostic operation. Establish macOS build CI and record local launch evidence. Windows/Linux verification is deferred to milestone 8.
- Exclude HTTP execution, run state machine scaffolding, charts, persistence, request editor, and fabricated metrics.
- Detailed tasks and dependencies: [[todos#Phase 1 foundation]].

### 2. Minimum constant-VU engine

- Implemented. Detailed scope, decisions, and local verification evidence: [[plans/11-constant-vu-engine]].
- Add validated GET/POST construction, pooled transport, constant VUs, pacing, duration, start/stop, bounded metrics, and final summaries. Rust run ownership is introduced here.
- Establish core budgets and high-load confirmation before exposing runnable load controls. A minimal diagnostic execution view may precede the full editor.
- Types: validated request/workload configuration, run handle/state, typed request outcomes, histogram snapshot, final summary.
- Gate: local-server tests prove request construction, status/transport/timeout classification, body consumption, latency population, counter reconciliation, VU limits, duration, cancellation, repeated runs, cleanup, and stalled-target overload behavior.
- Test scheduling with controllable time where practical; use tolerances for actual network timing. Confirm zero in-flight requests and no owned tasks remain after stop.
- No advanced workloads, thresholds, saved configurations, or every-request persistence.

### 3. Live monitoring and first usable release

- Add the request editor, execution controls, aggregated events, uPlot charts, progress, error display, and final results view.
- Charts show interval request rate, response latency, HTTP failure rate, and VUs; cards show totals, duration, and state. Update at one second by default.
- Types: sequenced metric snapshot, bounded chart series, frontend run view state.
- Gate: a developer completes the full workflow against a local server; navigation preserves a run; stop/exit behaves as agreed; listener/chart cleanup works; delayed/missed events recover totals and final state.
- Verify memory reaches a stable bound during an extended run and UI updates do not enter measured latency. No history is promised in this release.

### 4. Ramping VUs

- Add staged workloads, stage validation, monotonic interpolation, retiring-worker ownership, and desired/actual VU display.
- Gate: deterministic tests cover ramp-up, ramp-down, zero-VU stages, boundaries, cancellation mid-stage, and cleanup under slow responses. No abrupt cancellation solely to reach a lower target.

### 5. Constant arrival rate

- Add target iteration rate, explicit concurrency capacity, deadline scheduling, dropped-iteration reasons, lateness, and saturation warnings.
- Gate: local-server overload tests prove bounded tasks/memory, no backlog, no catch-up bursts, correct drop accounting, and cleanup. Separate inability to offer load from target HTTP failures.
- Document offered versus achieved rates and scheduler timing tolerances.

### 6. Thresholds and remaining HTTP options

- Split implementation into focused increments: final threshold evaluation; remaining methods; Bearer/Basic auth; URL-encoded bodies; multipart/files; HTTP version/redirect controls.
- Add PUT, PATCH, DELETE, HEAD, OPTIONS; preserve HTTP/1.1 and HTTP/2 support, connection reuse, and timeout validation.
- Stream file uploads and close handles on completion/cancellation. Keep secrets out of diagnostics and generated errors before adding auth controls.
- Gate: threshold boundary/empty-sample tests and local-server tests for each request option, redirect limits, file cleanup, and negotiated/forced protocols. Threshold UI exposes the metric population and comparison operator.
- Assertions remain outside this roadmap; retain distinct outcome semantics for a future design.

### 7. Persistence, credentials, history, and comparisons

- Choose a maintained SQLite library; store saved configurations, run history, final summaries, thresholds, and aggregated time series. No individual request storage by default.
- Omit secrets from saved configurations by default. Explicit remembered credentials use the OS store; SQLite contains references. Protect sensitive URL/query/header/body data as well as dedicated auth fields.
- If a credential store is unavailable, require re-entry; do not fall back to plaintext. Do not promise automatic detection of every secret in arbitrary payloads.
- Use a bounded persistence pipeline; surface write failures, backpressure, missing intervals, and incomplete history. Keep execution/summary state truthful if disk is full.
- Select finite defaults for chart windows, history compaction, batching, and comparison selection during this milestone. Summaries remain until deletion; compacted chart resolution is visible.
- Gate: tests cover save/load, secret omission and reference resolution, unavailable credentials, transactions, disk errors, compaction, deletion, restart recovery, and comparable-run selection without loading all history.
- Interrupted runs are distinguishable from completed results. Long-run chart storage and memory must have a documented bound.

### 8. Release hardening and benchmarks

- Complete platform packaging, resource safeguards, release documentation, benchmark harness, and clean-machine installation/launch checks.
- Milestones 1–7 verify macOS only. Once the application is fully working on macOS, milestone 8 adds Windows/Linux CI, builds, packaging, and interactive launch checks before a public release.
- Benchmarks report hardware, OS, payload, HTTP protocol, connection settings, target capacity, offered/achieved load, resource consumption, and measurement integrity. Separate generator and target bottlenecks.
- Gate: published capacity statements have reproducible measurements; no equivalence to k6 is claimed without comparative benchmarks. Signed distribution, where offered, needs platform-specific release evidence.
- Keep startup, idle resources, cancellation, and long-run stability in release verification. Broader refactoring is justified by measured issues only.

## Architectural risks and required evidence

| Risk                                                       | Required response                                                                                |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Scheduler or aggregator becomes the bottleneck             | Bound queues/tasks, expose lateness/backpressure, test overload, publish measured limits         |
| Cancellation leaks work or mislabels failures              | Own and join tasks, classify aborted attempts separately, reconcile counters after stop          |
| Histogram range or JS numeric precision loses observations | Explicit range/overflow policy, wire encoding, precision tests, integrity flags                  |
| Redirects, compression, partial bodies distort accounting  | Define logical request and body-byte semantics; test redirect and stream failure paths           |
| UI events disappear or arrive late                         | Run IDs, sequences, cumulative counters, current/final state recovery                            |
| SQLite stalls or chart history grows forever               | Bounded independent persistence, visible errors, compaction and long-run tests                   |
| Secrets enter payloads/logs/history                        | Redaction policy, explicit sensitive fields, credential references, persistence tests            |
| Desktop starter assumptions break portability              | Review CLI imports, macOS options, asset permissions, CSP, dev hooks, and packaging per platform |
| Generated bindings drift from serialization                | Deterministic generation, CI drift checks, authoritative native validation                       |

## Verification and implementation guidance

- Use relevant project skills during implementation: `xcvzmoon-code-conventions`, `ts-best-practices`, `vue-best-practices`, `tauri-v2`, `valibot`, `frontend-design`, `unslop`, and testing skills when writing tests.
- Follow Rust public-API documentation and frontend exported-function JSDoc requirements. Keep examples executable and docs free of warnings.
- Use the main worktree. Do not commit or push without an explicit request. Do not modify vendored skills or lint-plugin code for product work.
- Run `pnpm install` before the first typecheck.
- Required verification order for completed work: `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Also run `pnpm lint` because formatting checks do not run Oxlint/Clippy.
- Workspace scripts must include engine tests/doctests, not just the desktop crate. Add meaningful frontend contract tests to the aggregate test command.
- A successful build is not launch evidence. Record platform, commands, launch outcome, and exercised behavior; report unavailable platform evidence honestly.
- Later-milestone implementation starts only when requested. Resolve version/library selection and detailed storage defaults at their assigned task gates, without silently changing the approved product semantics.

## Phase 1 implementation decisions

- Root Cargo workspace owns both crates and the lockfile. Engine has no dependencies; its identity API proves the crate boundary without fake execution.
- ts-rs 12.0.1 generates Serde-compatible DTOs through `pnpm bindings:generate`; `pnpm bindings:check` compares formatted output without rewriting tracked contracts.
- Runtime API is @tauri-apps/api 2.12.4. Vitest 5.1.0 runs injected transport tests without module mocking. Thiserror 2.0.21 and tracing 0.1.44 support native errors and diagnostics.
- Tracing owns native logging; the starter log plugin was removed to prevent competing global logger initialization during development launch.
- Diagnostic labels use 1–64 Unicode code points after trimming. No numeric counters cross IPC yet. Future u64 counters/durations will serialize as decimal strings with matching generated TypeScript strings; chart conversions must check numeric range.
- Runtime font assets and their OFL license are bundled locally. CSP permits local assets and Tauri IPC; the unused broad asset filesystem protocol was removed.
- Native metrics/events remain deferred to milestones 2–3 as agreed. Active CI targets macOS only; Windows/Linux verification is deferred to milestone 8.
- Official references: [ts-rs](https://docs.rs/ts-rs/12.0.1/ts_rs/) and [Tauri commands](https://v2.tauri.app/develop/calling-rust/).

### Local verification evidence, 2026-11-05

- `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed on macOS arm64. `pnpm lint` passed with two preserve-caught-error warnings where the UI adapter intentionally replaces infrastructure details with a safe error message.
- Four frontend contract tests, two native diagnostic tests (also compiled in the exporter target), and both public-API doctests pass. `cargo test -p velo-engine` passes independently.
- `pnpm bindings:check` passes. Formatting and `git diff --check` pass.
- Development launch passed after removing the duplicate global logger. Route templates have single roots for Nuxt transitions.
- Final application and DMG were generated in `target/release/bundle/`. An initial DMG attempt encountered a busy temporary disk image; detaching that generated image and rebuilding succeeded.
- Packaged app launched, Settings navigation worked through the native accessibility tree, and Check connection returned `Connected to velo-engine 0.0.0.`. Shell screenshot was inspected during development.
- Windows/Linux builds and interactive checks are deferred to milestone 8; active CI targets macOS only. No remote CI run, signing, or notarization is claimed.

### Verification scope update

- User decision: complete the working macOS product first. Verify Windows and Linux in milestone 8; they are not acceptance gates for milestones 1–7.
