# Velo

Velo is a local desktop studio for testing HTTP APIs under load. Configure a request, choose a workload, watch live response and throughput charts, and inspect or compare results saved on your machine.

## What you can do

- Run constant virtual users, staged VU ramps, or a constant arrival rate with bounded concurrency.
- Configure HTTP methods, headers, query parameters, authentication, text/JSON/form bodies, and streamed multipart uploads.
- Set final thresholds for response p95, HTTP failure percentage, and request throughput.
- Stop a run with graceful drain, navigate without interrupting it, and retain partial results.
- Edit and share versioned JSON test definitions, select environments, import API requests, and preview one request.
- Save tests with OS secret references and compare compatible historical runs.
- Run exported definitions headlessly in CI with explicit target authorization and JSON, JUnit, or HTML reports.

Each desktop instance or CLI process runs one test at a time. Tests can repeat one request or run configured scenarios. Velo does not provide browser testing or distributed load generation.

## Run locally

Install Node.js 26, pnpm 12, the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml), and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```sh
pnpm install
pnpm dev
```

The development runner launches Nuxt and the native desktop shell together. A browser-only frontend cannot run load tests. Use the desktop app for editing and local history, or the [headless CLI](docs/headless.md) for automation.

## Run your first test

1. Open **Tests** and enter the URL of a target you are authorized to test.
2. Choose the HTTP method and any headers, authentication, or body the request needs.
3. Select a workload and scheduling duration. Start with a small virtual-user count.
4. Select **Review run**, inspect the target and requested load, then acknowledge authorization and start.
5. Watch the live charts or select **Stop run** to end admission and drain active requests.
6. Open **Results** to inspect the summary, select stored runs for comparison, or delete history.

Save the editable definition under **Saved tests**. Definitions retain literal request fields and non-secret environment values locally; use OS secret references for private values. See [test authoring](docs/authoring.md) for JSON files, environments, imports, and preview, and [persistence and credentials](docs/persistence.md) for retention and recovery.

## Implementation

The application uses Tauri 2 with a client-rendered Nuxt 4/Vue 3 frontend. Rust owns HTTP execution, scheduling, bounded metrics, cancellation, and authoritative validation. The frontend uses TypeScript, Tailwind 4, shadcn-vue/Reka UI, Formisch, Valibot, and uPlot.

| Location                 | Responsibility                                                                 |
| ------------------------ | ------------------------------------------------------------------------------ |
| `app/`                   | Request editing, live monitoring, results, and native client                   |
| `crates/velo-engine/`    | Desktop-independent Tokio/Reqwest engine and HDR metrics                       |
| `crates/velo-contracts/` | Shared desktop contracts and binding exporter                                  |
| `crates/velo-cli/`       | Headless runner and CI report adapter                                          |
| `src-tauri/src/`         | Commands, run ownership, SQLite history, OS credentials, and desktop lifecycle |
| `app/types/contracts.ts` | Generated Rust-owned TypeScript contracts                                      |
| `docs/`                  | Implementation guides, development instructions, and dated evidence            |

Load-run responses are streamed and discarded. Explicit single-request previews retain bounded response content temporarily. Live charts and stored time series are bounded; individual requests are not persisted. Retained run summaries grow disk usage until deleted. See [execution and measurements](docs/execution.md) for outcome definitions and workload semantics.

## Build and verify

```sh
pnpm check
pnpm typecheck
pnpm test
pnpm build
pnpm lint
```

Build on the target operating system. Installers are written under `target/release/bundle/`; on macOS, the application is `target/release/bundle/macos/Velo.app`.

Local acceptance evidence covers macOS. Windows/Linux CI is configured, but installation and interactive verification remain pending. Signing, clean-machine acceptance, and capacity measurements are separate [release gates](docs/release-readiness.md).

Read the [documentation index](docs/index.md) for implementation and operating details, [CONTRIBUTING.md](CONTRIBUTING.md) for contributions, and [SECURITY.md](SECURITY.md) for vulnerability reports. Velo is licensed under [MIT](LICENSE.md).
