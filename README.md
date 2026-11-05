# Velo

Velo is a local desktop API load-testing studio built with Tauri 2, Nuxt 4, Vue 3, and an independent Rust engine crate.

The application provides Dashboard, Tests, Results, and Settings navigation and a native diagnostic round trip. HTTP execution, workloads, metrics, charts, and persistence are not available yet.

## Install and develop

Install Node.js 26, pnpm 12, Rust 1.90 or newer, and the [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system. Windows needs the C++ build tools and WebView2; Linux needs WebKitGTK 4.1 and its development libraries; macOS needs Xcode command line tools.

```sh
pnpm install
pnpm dev
```

The development runner chooses an available port and launches Nuxt and Tauri together. The static Tauri configuration uses port `65432` when invoking the CLI directly.

Open Settings and run the native diagnostic. A successful response identifies the independent Rust engine and its version. Invalid labels show a validation error; the operation sends no HTTP requests. A browser-only frontend cannot execute native commands.

## Architecture

| Location                 | Responsibility                                                                |
| ------------------------ | ----------------------------------------------------------------------------- |
| `app/`                   | Vue screens, shadcn-vue/Reka UI components, Valibot validation, native client |
| `app/types/contracts.ts` | TypeScript contracts generated from Rust                                      |
| `crates/velo-engine/`    | Independent Rust API; no Tauri or webview dependency                          |
| `src-tauri/src/`         | Native startup, validated commands, transport DTOs, structured errors         |
| `scripts/bindings.ts`    | Binding generation and drift verification                                     |

Rust owns the serialized contracts. The desktop adapter validates IPC input before calling the engine. Vue validates user input for immediate feedback and displays native success or structured failure. This boundary keeps future load generation out of Vue and Tauri command handlers.

The frontend uses Composition API, TypeScript, Tailwind 4, and the existing shadcn-vue/Reka UI components. It does not introduce a second component library. The diagnostic sends no HTTP traffic and stores no credentials.

## Generated contracts

```sh
pnpm bindings:generate
pnpm bindings:check
```

The generator uses `ts-rs` declarations from Rust and runs through `cargo run -p velo --bin export-bindings`. Commit generated contract updates alongside the Rust types that require them. Do not edit `app/types/contracts.ts` manually. CI checks for drift. Nuxt exposes native contract types through `#imports`.

TypeScript and Vue component tests live under `tests/`, mirroring the application directories.

## Verify changes

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

Development and CI currently verify macOS only. Windows and Linux builds, packaging, and interactive checks are not verified yet. Configuring CI is not evidence of passing remote runs.

## Build and launch

```sh
pnpm build
```

Tauri generates the static frontend and builds the native application and installers. The Cargo workspace places build outputs under `target/`; distribution bundles are under `target/release/bundle/`. Build on the target operating system.

On macOS, open `target/release/bundle/macos/Velo.app`. On Windows or Linux, install the corresponding bundle produced on that platform. Verify navigation, native titlebar controls, theme switching, and the Settings diagnostic after launch. Signing and notarization need separate release configuration and credentials.

## Contributing

Read [AGENTS.md](AGENTS.md) for repository conventions, [CONTRIBUTING.md](CONTRIBUTING.md) for contributions, and [SECURITY.md](SECURITY.md) for vulnerability reports. Do not run future load tests against targets without authorization.

Velo is licensed under [MIT](LICENSE.md).
