# Development and verification

## Setup

Install Node.js 26, pnpm 12, the Rust toolchain pinned in `rust-toolchain.toml`, and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system. Windows needs the C++ build tools and WebView2; Linux needs WebKitGTK 4.1 and its development libraries; macOS needs Xcode command line tools.

```sh
pnpm install
pnpm dev
```

The development runner chooses an available port and launches Nuxt and Tauri together. The static Tauri configuration uses port `65432` when invoking the CLI directly.

Open Settings and run the native diagnostic. A successful response identifies the independent Rust engine and its version. Invalid labels show a validation error; the operation sends no HTTP requests. A browser-only frontend cannot execute native commands.

## Checks

Install dependencies before the first typecheck, then run the required sequence:

```sh
pnpm check
pnpm typecheck
pnpm test
pnpm build
pnpm lint
```

`check` checks formatting. `lint` runs type-aware Oxlint and Clippy. The test command covers frontend boundary behavior and Rust workspace tests, including doctests. The engine can also be checked independently:

```sh
cargo test -p velo-engine
```

Local acceptance evidence currently covers macOS only. Windows and Linux builds, packaging, and interactive checks are not verified yet. Configuring CI is not evidence of passing remote runs.

## Packaging

```sh
pnpm build
```

Tauri generates the static frontend and builds the native application and installers. The Cargo workspace places build outputs under `target/`; distribution bundles are under `target/release/bundle/`. Build on the target operating system.

On macOS, open `target/release/bundle/macos/Velo.app`. On Windows or Linux, install the corresponding bundle produced on that platform. Verify navigation, native titlebar controls, theme switching, and the Settings diagnostic after launch. Signing and notarization need separate release configuration and credentials.

## Continuous integration

CI is configured for macOS 15, Ubuntu 24.04, and Windows Server 2022. It runs lint, binding drift checks, formatting, typechecking, tests, and installer builds. Artifacts are retained for seven days. Configuration does not establish remote success or interactive platform acceptance. The tag-triggered release workflow remains macOS-only.

## Native failure policy

Sensitive request types redact Debug output. HTTP diagnostics record fixed error categories at most once per category per run, without source messages or request data. Storage diagnostics retain safe categories and SQLite error codes. Programming panics abort the desktop process without printing panic payloads; incomplete persisted runs are recovered as interrupted on the next launch.
