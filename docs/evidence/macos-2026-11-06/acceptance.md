# macOS desktop acceptance, 2026-11-06

## Build and method

Tested the packaged `target/release/bundle/macos/Velo.app` built in this working tree on macOS 27.0 arm64, Apple M1, 8 GiB RAM. This was an existing developer machine, not a clean installation. Automation used macOS Accessibility actions and keyboard input against the actual packaged webview; native state was cross-checked using read-only SQLite queries. No browser simulation or injected IPC calls were used.

The target was an ephemeral Python `ThreadingHTTPServer` at `127.0.0.1:8080`, using HTTP/1.1, a 10 ms handler delay, and a seven-byte `payload` response. It ran on the same machine. This is a desktop acceptance workload, not an isolated capacity benchmark.

## Acceptance results

| Check                        | Evidence and outcome                                                                                                                                                                                                                                     |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Navigation during execution  | Tests → Results → Settings → Tests preserved the running workload.                                                                                                                                                                                       |
| Native diagnostics           | Settings returned `Connected to velo-engine 0.0.0.` during the run.                                                                                                                                                                                      |
| Theme and window controls    | Light/dark switching, resize to 640 × 480 and 1000 × 800, minimize and restore succeeded. Closing during a run presented the native exit confirmation.                                                                                                   |
| Sustained execution          | Eight constant VUs ran for 600 seconds with live charts visible. The UI reported a connected aggregate subscription and no detected event gaps.                                                                                                          |
| Final accounting             | 356,886 started = 356,867 successful responses + 19 transport failures. No status failures, timeouts, or cancelled requests. Final active VUs and in-flight requests were zero. Actual duration was 600.011191 seconds.                                  |
| Repeated cancellation        | Five separate UI start/stop cycles reached cancelled state, reconciled counters, and ended with zero active VUs and in-flight requests.                                                                                                                  |
| Save and restart             | A reviewed request saved with sensitive data omitted appeared in the saved list. All six initial test runs appeared in history after restarting the app.                                                                                                 |
| OS credential round trip     | A dummy `/acceptance-dummy` URL path was saved using “Remember full request in OS store”. After restart, Load restored the full path; SQLite held only the redacted origin. Keychain entry existence was checked without reading its password into logs. |
| Configuration deletion       | All three temporary configurations were deleted through the UI. The remembered Keychain entry was absent afterward (`security` exit code 44).                                                                                                            |
| History selection/comparison | Selecting two stored runs displayed both persisted summaries and cancellation status.                                                                                                                                                                    |
| Active exit                  | “Keep running” dismissed the confirmation. “Stop and quit” exited and retained a cancelled summary with zero active workers and in-flight requests.                                                                                                      |
| History deletion             | All eight session-created test records were exported to [history.json](history.json), then deleted through the UI. Read-only queries confirmed zero remaining runs and configurations.                                                                   |

The 19 early transport failures remained unchanged through the sustained run. Their cause was not isolated during the original acceptance run; the follow-up below resolves it. They are preserved in the summary and prevent treating this workload as a clean throughput result.

## Startup and resource measurements

Two warm launches reached an accessible dashboard in 1.831 s and 1.074 s, measured from `open` to Accessibility exposing Dashboard and Configure a test. These include polling/automation overhead and are not cold-start measurements.

[resources.csv](resources.csv) contains samples at approximately two-second intervals. Sampling began about twenty seconds into the sustained run and continued through cancellation, restarts, and idle checks. Columns distinguish native and WebKit RSS. WebKit helpers were identified from the process list; three helpers were present during the sustained run. Attribution may include another WebKit helper if one started during sampling.

For elapsed sample times 60–560 s (244 samples), summed native/WebKit RSS had a median of 117.0 MiB and a maximum of 142.625 MiB. Summed sampled CPU had a median of 25.6%. The idle window at 816.92–889.28 s (36 samples) had median RSS 106.109375 MiB and median CPU 0.15%.

RSS sums can double-count shared pages and are affected by OS memory pressure. `ps` CPU percentages are decaying process statistics, not exact per-interval CPU utilization. Activity outside Velo was not isolated. These measurements found no crash or retained active workers in the exercised workload; they do not prove an absence of leaks or establish a process-wide allocation bound.

## Remaining release evidence

A ten-minute workload and five cancellation cycles are a macOS stability smoke test. Multi-hour soak testing, chart retention beyond its 600-point window, broad workload/protocol coverage under resource monitoring, cold startup, clean-machine installation, and signed/notarized distribution still need separate evidence. Windows/Linux acceptance remains pending.

## Repository validation

After the desktop checks, `pnpm install`, `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build`, and `pnpm lint` passed. The final build produced both the app bundle and DMG. No product code was changed during this acceptance session.

## Transport-failure resolution

Controlled reproduction used the same public engine API, eight constant VUs, three ten-second repetitions, and the original Python handler. Temporary diagnostics captured OS error 54 (`ConnectionReset`) and OS error 32 (`BrokenPipe`) while sending requests. Python 3.9's `ThreadingHTTPServer` used a listen backlog of five, smaller than the eight-connection startup burst.

Changing only the server backlog to 128 removed the failures. Original-backlog repetitions recorded 172, 2, and 2 transport failures; corrected-backlog repetitions recorded zero failures across 6,342, 6,271, and 6,303 started requests. Velo's no-retry policy and transport accounting remain unchanged. Temporary instrumentation and the probe example were removed.

The corrected loopback fixture is retained as [acceptance-target.py](acceptance-target.py). Run it with `python3 docs/evidence/macos-2026-11-06/acceptance-target.py`; stop it after testing. Its backlog is target-test infrastructure, not a Velo configuration setting. The earlier resource samples and history remain the original measurements; they have not been rewritten as failure-free results.

The packaged desktop rerun at eight VUs for sixty seconds completed 36,125 successful requests with zero transport failures, timeouts, or status failures. Counters reconciled and terminal active VUs/in-flight requests were zero. The result is retained in [transport-desktop.json](transport-desktop.json); controlled engine results are in [transport-reproduction.json](transport-reproduction.json).
