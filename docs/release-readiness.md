# Release readiness

Local macOS evidence includes packaged launch, desktop acceptance, a ten-minute workload, repeated cancellation, credential/history recovery, and resource sampling. See [acceptance evidence](evidence/macos-2026-11-06/acceptance.md) and [implementation history](history.md).

Before declaring a public release ready:

- Execute Windows/Linux builds and clean-machine installation and interactive acceptance.
- Record cold startup and multi-hour resource stability, including chart retention beyond the live window.
- Measure capacity against a separately provisioned target with generator and target resource monitoring.
- Decide and verify platform signing/notarization and distribution requirements.

The opt-in engine baseline runs a five-second warmup and three thirty-second loopback measurements:

```sh
cargo test --locked --release -p velo-engine --test execution release_benchmark -- --ignored --exact --nocapture --test-threads=1
```

The target shares the generator process. Capture the commit, lockfile, toolchain, hardware, OS, power mode, workload, and resource samples. Exclude warmup results. This baseline does not establish isolated generator capacity or equivalence to another load engine.
