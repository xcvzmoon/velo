# Generator health and capacity

Generator health pairs native host observations with existing workload admission accounting. Open a run's monitor or expand a stored run in Results to inspect resource charts, VU counts, and timestamped pressure evidence. Expanding two history entries lets you compare their retained observations on the same workload-relative time axis.

## Resource measurements

| Measurement        | Scope                                                | Units and interpretation                                                                                                                           |
| ------------------ | ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native process CPU | Velo's native process; excludes WebView subprocesses | Percent of all logical CPUs. On eight logical CPUs, one fully occupied CPU is 12.5%; total capacity is 100%.                                       |
| Resident memory    | Velo's native process; excludes WebView subprocesses | Bytes on the native boundary, displayed as MiB (1,048,576 bytes). Shared resident pages may be included by the OS.                                 |
| Host CPU           | All processes on the local host                      | Percent of all logical CPU capacity, from 0 to 100%.                                                                                               |
| Available RAM      | Local host physical memory                           | OS-reported RAM available for allocation, displayed as MiB. OS cache and reclaimability accounting varies. This is not a memory-stall measurement. |

The sampler starts with workload execution, after scenario setup, and stops after request draining, before scenario cleanup. It waits one second between native refreshes, with no catch-up queue. Each sample records its actual refresh interval and end timestamp in monotonic microseconds relative to workload start. CPU averages cover that native window; memory values describe the refresh observation.

A dedicated native thread owns one `sysinfo` instance and refreshes only the current process, CPU usage and physical RAM. It does not enumerate process threads, collect command lines, refresh disks or networks, or hold the HTTP metrics lock during native reads. A latest-value channel retains one sample, and HTTP execution does not wait for native reads. Normal completion and stop join the sampler before publishing the terminal run state. Dropping its owner disconnects the stop channel so the sampler exits after any native call already in progress.

## Rates and admission evidence

Requested arrival rate uses iterations per second. Offered slots and admitted iterations both use the observed scheduling-only window. Admitted iterations equal offered slots minus capacity drops, missed deadlines and dataset drops. Scenario iterations may execute multiple HTTP requests, so their rate is a separate population.

Started and completed HTTP request rates both use elapsed scheduling plus drain. Completed requests include HTTP responses, transport failures and timeouts, and exclude cancellations. These full-window HTTP rates are not equivalent to scheduling-only iteration rates. A run with two seconds of scheduling and two seconds of drain divides offered/admitted iteration counts by two seconds and HTTP request counts by four seconds.

Capacity drops mean all configured arrival capacity was owned. Missed deadlines mean the scheduler skipped arrival slots; maximum lateness uses microseconds. Dataset drops remain distinct. These counters do not enter HTTP or assertion failure rates. Desired, actual and retiring VUs reuse engine accounting; actual includes retiring workers, rather than introducing a separate concurrency counter.

Pressure evidence names the observation and its threshold: native-process or host CPU at least 90% of machine capacity, host available RAM at most 10% of total physical RAM, or new capacity drops or missed slots. These fixed thresholds are inspection heuristics, not calibrated throughput limits. A fully occupied single CPU can constrain work while total machine CPU remains below 90%. A zero pressure marker means no listed evidence in a complete resource sample, not proven spare capacity. The evidence does not identify a target service, database or network as a root cause.

## Missing data and retention

Native fields use null for unavailable observations. Sampler state distinguishes warming up, available, unavailable and unsupported. Available samples can contain null fields when only some measurements exist. Thread creation or native initialization failure does not fail HTTP execution. Short runs may finish before the first complete CPU window. Missing process data does not become zero CPU or memory; an unavailable resource sample produces an unknown pressure marker unless separate admission evidence establishes pressure.

The UI treats samples more than 2.5 seconds old, or with a future timestamp, as unavailable. Resource charts use native timestamps and plot a repeated sample once. They leave gaps between native observations more than 2.5 seconds apart. The pressure and VU timelines use aggregate observation times; evidence lists show up to 20 recent observations. The UI never reconstructs a native sample that the publisher did not observe.

Live charts retain the latest 600 aggregate observations. SQLite retains at most 600 aggregate intervals per run using the existing history compaction policy. Compaction selects every second retained point and doubles the recorded stride; omitted peaks cannot be reconstructed. Gaps break both native charts and admission-counter deltas. A history entry with write gaps hides timeline charts and keeps its latest cumulative and resource observations visible. History storage failure retains the existing incomplete-history reporting.

## Platform behavior and verification

The native adapter uses `sysinfo` 0.37's system backend, which supports macOS, Linux and Windows desktop targets. The dependency fits the workspace's Rust 1.90 minimum. Restricted environments may deny observations. Unsupported backends report unsupported with null fields. The library provides best-effort CPU and RAM counters rather than OS pressure-stall metrics, container-quota normalization, thermal limits or swap activity; Velo does not claim to measure those signals.

Local macOS verification covers a real process CPU/memory sample, native timestamps and intervals, missing-process observations, injected unavailable readings, sampler cancellation and joining, HTTP execution with telemetry, and history round trips through compaction. Frontend tests cover missing, stale, future and partial samples, explicit rate windows, native deduplication, and gaps in retained evidence. The existing CI matrix runs the same Rust and frontend tests on macOS, Ubuntu and Windows. Linux and Windows results require that CI or native verification on those systems; a local macOS pass does not establish their runtime behavior. No maximum throughput claim follows from these tests.

## Rust guideline audit

The feature was reviewed against Microsoft's [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/guidelines/checklist/index.html), particularly mockable native reads, telemetry overhead, cancellation, public type documentation and workspace dependencies. The resulting changes keep counters numeric inside Rust, serialize large values as decimal strings at the boundary, isolate the native reader for failure injection, and reject a refresh after sampler shutdown. Public contracts derive Debug, expose nullable availability, and avoid external library types. Sampling has constant retained state, bounded refresh work, no per-request telemetry, no unsafe code, and no panic-based handling of expected measurement failure.
