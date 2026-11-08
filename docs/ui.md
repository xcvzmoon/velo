# Workspace interface

Velo uses a compact engineering studio with persistent navigation, emerald accents, tabular measurements, and light and dark themes. Select a theme in the titlebar or under **Settings → Workspace**. UI state stays local; submitting to an AI provider requires a separate payload review.

## Find a workflow

| Workspace     | Sections and actions                                                                                                                          |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Overview      | Open a workspace and monitor the current or most recent execution.                                                                            |
| API workspace | Choose a collection; edit Requests, Run collection, inspect History, or manage Secrets.                                                       |
| Test studio   | Edit a Request visually, as JSON, or as Scenarios. Configure Data, inspect Preview & run, manage Library & secrets, or request AI assistance. |
| Results       | Inspect the Latest run or open History & comparisons to select runs, pin baselines, edit regression budgets, and export evidence.             |
| Settings      | Set appearance and test the native connection under Workspace. Configure optional AI providers separately.                                    |

Switching sections retains form edits. Test Studio also retains its draft during route navigation and accepts definitions opened from the API workspace. Importing or loading a definition replaces the editable test; data-file bindings must be selected again. Preparing and previewing data stays in the existing authoring boundary.

**Preview & run** sends a single request only through its explicitly authorized preview button. **Review load** prepares the definition before presenting the target, workload, warnings, and local budgets. Starting load requires the separate authorization action. Navigation preserves active execution; **Stop run** remains available in Overview, Test Studio, and the latest Results. It closes admission and uses the existing bounded drain behavior.

Results distinguish partial measurements, missing samples, failed assertions, regression verdicts, and unavailable comparisons. History remains local. Deleting stored runs, saved tests, configurations, API history, and collections opens a confirmation dialog. Cancel receives initial focus, Escape dismisses the dialog, and focus returns to its trigger.

## Components and state

Nuxt UI owns controls, menus, tabs, progress, badges, empty states, and confirmation dialogs. `app/app.config.ts` defines component defaults; `app/assets/css/tailwind.css` defines typography and page overflow behavior. Icons are bundled locally so the packaged application does not need an icon CDN.

Formisch remains the form-state and submission owner, and Valibot remains its validator. Nuxt UI provides visual controls without adding a second form-state system. Semantic labels and error descriptions identify controls. Native validation, secret storage, engine measurements, report contracts, and portable test formats retain their existing ownership.

The shell owns navigation and scrolling. Feature components own their sections and emit typed actions to existing coordinators. Tab content stays mounted to avoid discarding drafts or restarting preparation on each section switch. Charts use theme-aware labels and line colors and retain gaps and measurement boundaries.

## Migration prerequisites

On 2026-11-08, GitHub marked all prerequisites complete: [#11](https://github.com/xcvzmoon/velo/issues/11), [#12](https://github.com/xcvzmoon/velo/issues/12), [#13](https://github.com/xcvzmoon/velo/issues/13), [#14](https://github.com/xcvzmoon/velo/issues/14), [#15](https://github.com/xcvzmoon/velo/issues/15), [#16](https://github.com/xcvzmoon/velo/issues/16), [#17](https://github.com/xcvzmoon/velo/issues/17), [#18](https://github.com/xcvzmoon/velo/issues/18), and [#19](https://github.com/xcvzmoon/velo/issues/19). The open-issue search returned only [#20](https://github.com/xcvzmoon/velo/issues/20), with no open stabilization bugs. The user approved the compact engineering studio direction before the broad migration.

The migration removes the vendored shadcn-vue controls, their configuration, old theme tokens, and unused direct dependencies. Reka UI remains a transitive Nuxt UI dependency. The Nuxt UI agent skill now comes from the upstream Nuxt UI repository; `AGENTS.md` identifies Nuxt UI as the application component system.

## Verify changes

Follow the [development checks](development.md#checks), then run `pnpm test:ui`. The browser suite exercises the generated frontend in Chromium and WebKit. Populated result checks use a synthetic IPC fixture; they verify rendering and controls, not native transport. Automated contrast and accessibility checks supplement keyboard and workflow tests; they do not establish full accessibility conformance or replace native desktop acceptance.

In the packaged app, check the native diagnostic, request preview, explicit load authorization, stopping and drain, live metrics, stored history, baseline comparisons, exports, and OS credential dialogs. Keep native evidence distinct from browser-only checks. Signing, distribution, and platform acceptance retain the limits described in [release readiness](release-readiness.md). Browser checks and screenshots are recorded in the [UI migration evidence](evidence/ui-2026-11-08/acceptance.md).
