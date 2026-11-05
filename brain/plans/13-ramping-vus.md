# Milestone 4: ramping virtual users

## Scope

Add staged VU ramps to the working request editor and engine. Preserve constant workloads, request behavior, aggregate recovery, resource budgets, and stop/drain cleanup. Arrival-rate scheduling, persistence, and thresholds remain out of scope.

## Decisions

- Use a tagged workload contract. Constant workloads define VUs and duration. Ramps define starting VUs and ordered stages with a positive duration and nonnegative target.
- Interpolate from each stage's starting count to its target using monotonic elapsed time. Round toward the starting count until the next whole-VU transition. Stage endpoints are exact, including zero-VU stages.
- Wake at the next whole-VU transition or stage boundary. Delayed scheduler wakes reconcile directly to the current target without replaying missed transitions.
- Retiring workers finish the current request and exit. Retirement interrupts pacing and prevents another request. Retirement does not cancel a response.
- Count retiring workers against the peak workload allocation. If slow retirees occupy available slots, defer new workers until slots are joined. Expose desired, actual, and retiring VUs separately.
- Native validation checks stage durations, summed duration representation, peak VUs, and stage configuration memory within the explicit request-data budget before starting.
- Reuse the existing Formisch editor and bounded uPlot charts. Review shows the full native-validated stage schedule before authorization.

## Tasks

1. [x] Add workload contracts, validation, monotonic scheduling, and bounded retirement ownership.
2. [x] Add stage editing, review, desired/actual chart series, and lifecycle counters.
3. [x] Test interpolation, zero stages, delayed wakes, slow retirement, cancellation, repeated runs, and budget enforcement. Run required verification and record evidence.

## Verification evidence

- Deterministic tests cover ramp-up/down boundaries, zero/hold stages, delayed sampling, nanosecond rounding, and maximum duration/count representations. Paused Tokio time verifies zero-worker scheduling and cancellation before a future ramp.
- Local-server tests hold responses behind an explicit gate to prove graceful retirement, no readmission by retirees, deferred ramp-up within the peak allocation, and zero owned/in-flight work after stop. Slow-response tests cover natural completion and repeated runs; a pacing test proves retirement wakes idle workers without another iteration.
- Frontend tests cover stage conversion and validation, duration overflow, peak budgets, inactive field edits, and distinct desired/actual chart values with gaps. Native command tests check ramp confirmation serialization.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` on macOS arm64. Also passed `pnpm lint`, `pnpm bindings:check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Frontend: 44 tests; engine: seven unit tests, sixteen local-server integration tests, and twenty-three doctests; desktop command and diagnostic tests also pass.
- Packaged app verification used a temporary loopback server with 50-ms responses. The editor reviewed a zero-start ramp to four VUs over four seconds, then to zero over ten seconds. It completed 559 successful requests with zero cancellations and zero in flight, peak requested/actual VUs of 4/4, and 14.017 seconds actual duration. Dashboard/Results navigation preserved the run; both VU chart series and the final summary rendered correctly. Screenshots and native accessibility values were inspected locally. This is a functional smoke check, not a capacity measurement.
- macOS is the available platform; Windows/Linux gates remain milestone 8. No remote CI, signing/notarization, or capacity benchmark is claimed.

- Scheduling uses Tokio monotonic deadlines and cancellable task joins: [sleep_until](https://docs.rs/tokio/1.53.1/tokio/time/fn.sleep_until.html), [JoinSet](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html). Stage controls use the existing [Formisch FieldArray](https://formisch.dev/vue/api/FieldArray/). No dependencies were added.
