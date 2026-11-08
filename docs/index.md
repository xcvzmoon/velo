# Velo documentation

These guides describe the current implementation. Delivery history and dated measurements are kept separately from operating instructions.

| Guide                                                                | Contents                                                                                         |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| [API workspace](api-workspace.md)                                    | Collections, functional runs, OAuth, sessions, response inspection, history, and load-test reuse |
| [Authentication security audit](authentication-security-audit.md)    | OAuth and HTTP authentication findings, fixes, regression evidence, and verification limits      |
| [API workspace Rust audit](api-rust-audit.md)                        | Microsoft Rust guideline findings, fixes, verification, and external integration limits          |
| [Test authoring](authoring.md)                                       | Definitions, environments, secret references, request imports, and preview                       |
| [MCP server](mcp.md)                                                 | Local client setup, offline authoring, selected evidence, and proposal review                    |
| [AI assistance](ai-assistance.md)                                    | Reviewed cloud submissions, editable drafts, evidence references, and credential handling        |
| [Test data](datasets.md)                                             | Seeded Faker generation, CSV/JSON imports, lifetimes, allocation, and exhaustion                 |
| [Headless CLI and CI](headless.md)                                   | Preparation, authorization, secret inputs, datasets, exit statuses, artifacts, and CI            |
| [Architecture](architecture.md)                                      | Crate boundaries, frontend, and native communication                                             |
| [Execution and measurements](execution.md)                           | HTTP options, scheduling, cancellation, metrics, and thresholds                                  |
| [Generator health](generator-health.md)                              | Native CPU/RAM, admission pressure, rate windows, retained timelines, and platform support       |
| [Baseline comparisons and reports](reports.md)                       | Pinned references, release metadata, regression budgets, and HTML/JSON/JUnit exports             |
| [Persistence and credentials](persistence.md)                        | SQLite ownership, history retention, OS credentials, and recovery                                |
| [Development](development.md)                                        | Setup, checks, native failure policy, builds, and CI                                             |
| [Release readiness](release-readiness.md)                            | Remaining acceptance gates and benchmark reproduction                                            |
| [Implementation history](history.md)                                 | Delivery decisions and historical verification                                                   |
| [macOS acceptance evidence](evidence/macos-2026-11-06/acceptance.md) | Desktop checks, resource samples, and transport-failure resolution                               |
