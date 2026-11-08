# Architecture

| Location                 | Responsibility                                                                     |
| ------------------------ | ---------------------------------------------------------------------------------- |
| `app/`                   | Vue screens, shadcn-vue/Reka UI components, Valibot validation, native client      |
| `app/types/contracts.ts` | TypeScript contracts generated from Rust                                           |
| `crates/velo-engine/`    | Independent Rust API; no Tauri or webview dependency                               |
| `crates/velo-contracts/` | Shared diagnostic contracts and the desktop-independent binding exporter           |
| `crates/velo-cli/`       | Standalone headless host, noninteractive inputs, signals, and shared report export |
| `src-tauri/src/`         | Native startup, validated commands, transport DTOs, structured errors              |
| `scripts/bindings.ts`    | Binding generation and drift verification                                          |

Rust owns the serialized contracts. The desktop adapter delegates execution validation to the engine boundary. Vue validates user input for immediate feedback and displays native success or structured failure. The engine owns HTTP execution, scheduling, metrics, and cancellation; command handlers adapt native state and events.

The frontend uses Composition API, TypeScript, Tailwind 4, and the existing shadcn-vue/Reka UI components. It does not introduce a second component library. The diagnostic sends no HTTP traffic and stores no credentials.
