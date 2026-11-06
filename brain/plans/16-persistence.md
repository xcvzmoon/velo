# Persistence, credentials, history, and comparisons

## Policies

- SQLite through rusqlite with bundled SQLite, in the application data directory. An OS file lock prevents another instance from recovering live records. One connection owned by a bounded worker; no storage calls on the request measurement path.
- Saved configurations omit URL path/query/userinfo, headers, query pairs, authentication and bodies by default. Names are user-authored and must not contain secrets. Re-enter omitted fields before execution.
- Explicit remember stores the complete request configuration in the OS credential store through keyring; SQLite holds only the reference and redacted configuration. Store failure requires re-entry; never write plaintext as a fallback.
- Summaries remain until explicit deletion. History pages contain 25 rows; comparisons load at most two selected records and require matching origin, method, and workload kind. Partial/interrupted runs are identified.
- Each run retains at most 600 aggregate intervals. At the bound, retain every second point and double the sampling stride. Display stride and missing-interval status; never interpolate missing observations. No individual request records.
- Bounded writer queue of 32 aggregate updates; overflow marks history incomplete without delaying engine sampling. Finalization drains queued updates before exit. Write failures are visible independently of execution success.
- Each interval is one transaction; no batch spans multiple intervals. Startup marks unfinished records interrupted. SQLite transactions keep snapshot/series metadata consistent; deletes cascade to series. No automatic summary expiry.

## Implementation

- [x] Native storage, recovery, deletion, compaction, bounded writer, safe errors.
- [x] Explicit OS credentials, saved configuration save/load/delete and validation.
- [x] Nuxt saved configuration controls, paginated history, selected summaries/charts and comparisons.
- [x] Persistence/credential failure tests and full repository verification.

## Dependencies

- rusqlite (bundled SQLite): https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html
- keyring 3 (native stores, no plaintext fallback): https://docs.rs/crate/keyring/3.6.0

## Local verification

- 2026-11-06, macOS arm64: `pnpm install`, then `pnpm check` → `pnpm typecheck` → `pnpm test` → `pnpm build` passed. `pnpm lint`, `pnpm bindings:check`, and `git diff --check` passed.
- Tests passed: 55 Rust unit/integration tests, 38 Rust doctests, and 78 frontend tests. New coverage includes redaction, unavailable credentials, corruption, transaction rollback, read-only writes, bounded compaction, cascade deletion, pagination, database reopen, exclusive ownership, queue overflow, finalization, and native IPC save/load.
- Built `target/release/bundle/macos/Velo.app` and `target/release/bundle/dmg/Velo_0.0.0_aarch64.dmg`.
- Real OS credential prompts and interactive UI behavior were not exercised. Credential behavior uses injected stores. Windows/Linux and remote CI remain unverified; milestone 8 remains open.
