# Velo documentation

These guides describe the current implementation. Delivery history and dated measurements are kept separately from operating instructions.

| Guide                                                                | Contents                                                                                   |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [Test authoring](authoring.md)                                       | Definitions, environments, secret references, request imports, and preview                 |
| [Test data](datasets.md)                                             | Seeded Faker generation, CSV/JSON imports, lifetimes, allocation, and exhaustion           |
| [Architecture](architecture.md)                                      | Crate boundaries, frontend, and native communication                                       |
| [Execution and measurements](execution.md)                           | HTTP options, scheduling, cancellation, metrics, and thresholds                            |
| [Generator health](generator-health.md)                              | Native CPU/RAM, admission pressure, rate windows, retained timelines, and platform support |
| [Baseline comparisons and reports](reports.md)                       | Pinned references, release metadata, regression budgets, and HTML/JSON/JUnit exports       |
| [Persistence and credentials](persistence.md)                        | SQLite ownership, history retention, OS credentials, and recovery                          |
| [Development](development.md)                                        | Setup, checks, native failure policy, builds, and CI                                       |
| [Release readiness](release-readiness.md)                            | Remaining acceptance gates and benchmark reproduction                                      |
| [Implementation history](history.md)                                 | Delivery decisions and historical verification                                             |
| [macOS acceptance evidence](evidence/macos-2026-11-06/acceptance.md) | Desktop checks, resource samples, and transport-failure resolution                         |
