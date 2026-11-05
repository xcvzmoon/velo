# Constant arrival rate

- Scope: milestone 5, following [[plans/13-ramping-vus]]. No thresholds, persistence, or additional HTTP options.
- Add an arrival workload with positive integer iterations/second, scheduling duration, and maximum concurrent requests. Capacity must fit the explicit VU budget. Reject pacing for arrival workloads; deadlines control admission.
- Use monotonic deadlines at index/rate, beginning at zero and excluding the duration endpoint. Calculate elapsed slots with integer arithmetic. On delayed wake, drop expired slots in one calculation and offer only the current slot. Never queue or replay missed work.
- Each admitted task executes one request. Bound the JoinSet by concurrency capacity, reap completions before admission, and reuse the existing HTTP pool, observation path, drain, cancellation, and task joining.
- Report offered slots, capacity drops, missed deadlines, and maximum scheduler lateness independently of HTTP outcomes. Offered = started + capacity drops + missed deadlines after cleanup. Drops flag generator saturation and do not affect HTTP failure rate.
- Expose workload controls through Formisch/Valibot, native confirmation, generated contracts, live monitoring, and final summaries. Preserve template sibling spacing.
- Tests: integer deadline boundaries, delayed wakes, configuration rejection, fast-target achieved load, stalled-target capacity, cancellation, repeated runs, wire precision, and request/drop reconciliation.
- Verification: install; check → typecheck → test → build; lint; bindings drift; local packaged launch where available. Record evidence after verification.
- Timing is best effort, not real time. Maximum lateness measures scheduler wake delay against the next unprocessed deadline; skipped slots remain visible. Offered rate uses scheduling time; achieved throughput uses actual time including drain.
- References: [Tokio JoinSet](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html), [monotonic deadline sleep](https://docs.rs/tokio/latest/tokio/time/fn.sleep_until.html).

## Verification evidence

- Completed on macOS 27.0 arm64. No dependencies added.
- Passed `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`. Also passed `pnpm lint`, `pnpm bindings:check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `git diff --check`.
- Frontend: 52 tests. Engine: nine unit tests, nineteen local-server integration tests, and twenty-four doctests. Desktop command tests cover arrival confirmation through actual IPC dispatch.
- Paused time proves fractional deadlines, duration endpoint exclusion, maximum rate representation, and skipping a 500-ms delayed wake without replay. A fast-target test caught a duration-end loop after all slots were processed; the exit condition was fixed and the test passes.
- A stalled-target test offers 250 slots at 1,000/s with capacity three. Exactly three requests start and are cancelled after zero grace; remaining slots reconcile as capacity drops or missed deadlines. Final owned tasks and in-flight requests are zero. State is bounded by capacity, fixed histograms, and scalar counters; no sustained-memory benchmark is claimed.
- Packaged `target/release/bundle/macos/Velo.app` launched. Using macOS accessibility controls, selected constant arrival rate, reviewed 10 iterations/s with capacity ten for two seconds, and ran against a temporary loopback target with 100-ms responses. Results showed twenty successful responses, forty payload bytes, zero drops, zero in flight, peak actual concurrency two, 2.008 seconds actual duration, and maximum scheduler lateness 2,067 μs. Native confirmation and Results navigation worked. This is a functional smoke check, not a capacity measurement.
- Built both the app and `target/release/bundle/dmg/Velo_0.0.0_aarch64.dmg`. Windows/Linux verification remains milestone 8. No remote CI, signing, notarization, or benchmark evidence is claimed.
