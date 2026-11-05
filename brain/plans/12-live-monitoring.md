# Live monitoring and first usable release

- Scope: milestone 3 of [[plans/velo-roadmap]]. Constant VUs only; no saved history, ramps, thresholds, or additional HTTP methods.
- [x] Add sequenced, bounded aggregate events with independent interval response histograms. Keep cumulative snapshots authoritative and emit terminal state after cleanup.
- [x] Add a Formisch/Valibot GET/POST request editor, safe native preparation, workload/budget confirmation, and explicit authorization acknowledgement.
- [x] Own subscriptions at application scope so navigation preserves runs. Recover current/final state through periodic reads; ignore stale runs and sequences. Mark missing chart intervals rather than interpolating them.
- [x] Add uPlot charts for interval request throughput, response p95, HTTP failure fraction, and active VUs. Retain at most 600 points per series and destroy charts/listeners on teardown.
- [x] Show progress, integrity warnings, stop/drain state, and the current run's complete summary. Confirm exit during active execution and await native cleanup.
- [x] Exercise local-server execution, event recovery, precision, bounded retention, and cleanup. Run install, check, typecheck, test, build, lint, and binding drift checks; record exact evidence and remaining manual gates.

## Decisions

- Events default to one second; the execution view offers 500 ms, 1 s, and 2 s updates. Native validation accepts only 250–10,000 ms.
- UI retains the latest run only. Request secrets remain in the in-memory editor; events and summaries contain safe origin/workload metadata.
- Exact counters stay decimal strings; chart rates use bounded interval deltas. Histograms have fixed allocation and reset after each aggregate sample.
- Native emission failures do not fail HTTP execution. Recovery reads restore totals and terminal state without reconstructing missed chart measurements.
- macOS AppKit termination is routed through the native exit handler using the existing Objective-C runtime dependencies. The delegate's optional method is installed once and its registration refreshed; startup fails with a stable code if the framework already supplies that method. Recheck this integration on Tauri upgrades. Background: [Tauri termination support](https://github.com/tauri-apps/tauri/issues/12978).
- Exit confirmation is attached to the main window. Cancelling restores admission; confirmed exit waits for workers and the aggregate publisher to be joined.

## Verification evidence, 2026-11-05

- Installed dependencies with `pnpm install` before typechecking. Added uPlot 1.6.32 and the Rust dialog plugin 2.8.0; macOS Objective-C dependencies reuse already locked runtime versions.
- Frontend tests cover request conversion/validation, subscription ordering, overlapping actions, exact counter strings, stale runs/sequences/lifecycle states, gaps, terminal recovery, subscription retry, and cleanup. A test with nearly 100,000 updates keeps at most 600 chart points.
- Engine local-server tests cover interval outcome reconciliation, histogram reset, monotonic duration accounting, stale publisher rejection, and terminal cleanup. Desktop tests exercise real command dispatch and aggregate delivery using Tauri's test runtime.
- Packaged macOS checks use an isolated HTTP server bound to 127.0.0.1:8080. The editor prepares and starts real requests; all four charts receive native measurements. Navigating to Dashboard and Results preserves the run and charts.
- A two-minute packaged run completed naturally and displayed the final summary. A subsequent stopped run displayed partial results with zero active VUs. Native/WebKit RSS samples at ten-second intervals showed no upward growth during the local run; this is a short observation, not a sustained-load benchmark.
- Packaged native Quit reaches the active-run confirmation. The Keep running choice dismisses the attached sheet and preserves the same run for both native Quit and the red window-close control. Confirmed exit logs cleanup and the packaged executable exits with status zero.
- DMG packaging intermittently encountered a busy temporary disk image on this host. Ejecting the task-created image and rerunning the original build succeeded; no packaging checks were bypassed.
- Windows/Linux builds and interaction checks, signing/notarization, and sustained-load capacity benchmarks remain milestone 8. Local observations do not establish a machine capacity guarantee.
- Passed the required sequence: `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Also passed `pnpm lint`, `pnpm bindings:check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Frontend suite: 33 tests; engine integration suite: 11 tests plus unit tests and workspace doctests.
