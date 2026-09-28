# Current Noema context

## Product and implementation

Noema serves one local owner through one main Chat and delegated Tasks.
The Go server owns Chat, Task workers, integrations, model execution, authentication, notifications, and SQLite migrations.
Rust remains in the Tauri desktop shell. The former Rust backend is removed.
Web and native iOS clients use the shared GraphQL API.

Start with [the product overview](../project.md) and [the runtime index](../harness.md).
Use subsystem contracts for durable rules. Use Git history for completed plans, test reports, and screenshot revisions.

## Current boundaries

- The [security contract](../harness/security.md) separates implemented controls, required information handling, and missing general controls.
- Action review uses saved human requests. Agent-edited Task files cannot create new human authority.
- Models classify authorization and risk. Server rules enforce exact approvals, current revisions, and execution ownership.
- Connection policies can permit calls without human approval. There is no universal private-data disclosure check.
- Memory belongs to the local human. Markdown pages own its content; an in-process index provides search.
- Projects provide shared Task context, not separate access-control boundaries.
- Shared workspaces and multiple human accounts are not implemented.
- Chat can reuse healthy Codex and OpenAI sessions across turns. Changed context or authority can require fresh sessions.
- Chat and Tasks load connector tools on demand. Provider capabilities determine native loading or the compact directory path.
- `code.run_lua` uses standard Lua 5.4. Connector transform definitions still use the existing `language: "luau"` identifier.

## Open work and validation limits

[Proactive event sources](../plans/2026-08-15-proactive-event-sources.md) remain approved but unimplemented.
The plan records required outcomes without presenting them as current product behavior.

The [personal-assistant replay cases](../difficult-digital-personal-assistant-tasks.md) are historical fixture inputs.
Old acceptance labels do not establish current behavior.
A successful process exit, synthetic fixture, or model answer does not prove an external action succeeded.
Check stored execution and provider outcomes before claiming completion.

Native iOS builds and Apollo generation require macOS and Xcode.
APNs and Live Activities require enabled entitlements, signing profiles, and server credentials.
Physical Apple notification delivery, native keyboard behavior, and device recovery need device checks.

A desktop cross-build does not prove packaged startup, shutdown, credential storage, or remote connection behavior.
Signing, notarization, updates, package notices, and platform-specific execution remain release checks.
Native Windows SQLite concurrency needs validation on Windows.
Credential-backed provider consent and actions require checks with the actual provider.
Synthetic OAuth and browser fixtures cannot establish that compatibility.
Full backup restoration with pending decisions and scheduled work also needs end-to-end verification.

Model evaluation summaries describe their recorded suites and hardware.
See [hosted evaluations](../../evals/model-matrix/README.md) and [local evaluations](../../evals/local-models/README.md) for evidence limits.

## Development

Use the existing development instance and the access paths in [AGENTS.md](../../AGENTS.md#noema-development-access).
Keep credentials out of logs, model context, screenshots, and ordinary records.
The [README images](../images/README.md) show real product work at capture time.
They are not a live status display.

Follow [engineering simplicity](../development/simplicity.md) for code budgets and scope.
Follow [validation](../../AGENTS.md#validation) for focused checks, broad checks, and reuse of unchanged results.
