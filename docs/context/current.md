# Current Noema Context

This is the active working brief for Codex sessions. Keep it below 300 lines.
Durable subsystem contracts belong in their closest document; Git history owns
completed milestone detail.

## Active Direction

Noema is an always-on, self-hosted personal agent operating system for humans,
agents, conversations, workspaces, projects, tasks, tools, memory, and governed
automation. Chat remains the primary surface; deeper management and inspection
appear when backed state and the human's current job require them.

The current product foundation is:

- one continuously available Noema server with web and desktop shells;
- SQLite-backed structured state owned exclusively by the server process;
- Noema-owned Codex OAuth plus first-party local GGUF inference;
- a native local-human Markdown memory tree with rebuildable lexical search;
- persisted conversations, transcript items, tools, artifacts, approvals, and
  provider/model settings;
- Work as the durable workspace/project/task and supervised-agent system;
- a React/Astryx frontend that starts from chat and progressively reveals
  memory, Work, settings, and inspection.

New slices should be vertical, user-visible, and small. Reuse or simplify the
current authority before adding types, ports, fixtures, or future-facing
frameworks. Follow `docs/development/simplicity.md` for budgets, tests,
subagents, reviews, and size measurement.

## Current Architectural Constraints

### Home and storage

- `${NOEMA_HOME:-$HOME/.noema}` is the durable home. SQLite lives at
  `db/noema.sqlite3`; model blobs, downloads, object-owned files, sidecar data,
  and rebuildable system state follow `docs/project.md`.
- The Noema server is the only process that opens the canonical SQLite
  database. Web, desktop, and future mobile clients use Noema APIs.
- This is pre-V1. Schema changes may rewrite the bootstrap and tables directly;
  there is no SurrealDB or older-SQLite migration path.
- Concrete object rows remain canonical. Actor/principal, governable scope,
  provenance source, and transcript item are behavior contracts implemented by
  concrete objects rather than universal parent tables.
- Filesystem paths hold durable object-owned bytes. SQLite owns metadata,
  relationships, policies, and immutable version records. `system/` state is
  derived and rebuildable.

### Conversations and runtime

- Durable chat history is reconstructed from `conversation_items`; daemon
  WebSocket state and `agent_status` are live coordination state only.
- Browser GraphQL subscriptions retry with capped backoff. After reconnect,
  clients refetch active queries and backfill durable transcript pages before
  trusting later live completion events.
- OpenAI and Codex are separate Responses dialects. Codex uses Noema-owned OAuth
  homes, exact ChatGPT workspace/client identity, and the provider catalog; it
  does not silently import Codex CLI authentication.
- Runtime execution, prompts, context, tools, persistence, task workers, and
  transport-neutral events live in `noema-runtime`. Host composition lives in
  `noema-host`; GraphQL lives in `noema-api`; server and desktop remain process
  shells.

### Providers and local models

- Provider selections persist exact provider instance, account, model/profile,
  and reasoning effort. Foreground work resolves current selections; admitted
  background runs retain their exact snapshots.
- First-party local inference is the `local_models` provider over a supervised,
  loopback-only pinned llama.cpp runtime. Runtime lookup never trusts ambient
  `PATH`; packaged assets and explicit development overrides are authoritative.
- Verified GGUFs are content-addressed, downloads are resumable, and public
  imports require immutable provenance plus SHA-256. Activation, removal,
  process leases, and in-flight requests must remain race-safe.
- The bundled catalog contains only qualified model/build combinations. Gemma
  4 E4B IT remains the current measured default; qualification reports under
  `evals/local-models/results/` own historical measurements.
- Built-in and MCP tools share one typed request catalog. Local grammar lowering
  may relax llama.cpp-incompatible schema constraints, but canonical runtime
  handlers still validate authoritative payload schemas.
- Local generation is priority-arbitrated so foreground chat precedes memory,
  tasks, audits, compaction, and task-originated web summaries. Prompt-cache and
  memory-observation scheduling must not regress foreground latency.

### Memory

- Markdown under `memory/human/` is the only memory authority for `human:local`.
  Frontmatter owns page metadata and provenance; the filesystem owns hierarchy.
- Canonical pages use a compact Wikipedia editorial form: a single generated
  title, concise lead, coherent sections, inline citations, and collected
  references. The root is the human overview; children are topic articles.
- `root.md` is bounded to 750 words and enters every ordinary turn. Native page
  reads and lexical search retrieve deeper detail; their page text is ephemeral
  while durable tool results retain references and hashes only.
- Memory updates read one captured range from the primary conversation using
  its existing `sequence_index`. Completed human text is evidence; assistant
  text may provide context but is not independent evidence.
- Context compaction and the Memory-page action schedule the same separate,
  single-flight background job through the selected Memory model. Publication
  recovers forward from `.pending`, rebuilds FTS, and advances `.state.md` last.
- Direct editing, history, private memory, additional scopes, vectors, and
  migration from the retired service remain outside the first slice.

### Capabilities, MCP, and artifacts

- Capability eligibility, approval, invocation, and audit authority remain
  explicit and fail closed. Do not infer semantic intent from English phrases,
  prefixes, or field names.
- `noema-capabilities-mcp` owns MCP contracts and transports. Stdio and rmcp
  Streamable HTTP are the supported transports; deprecated HTTP+SSE stays
  removed.
- MCP secrets use atomic, path-safe storage. Transport input, process trees,
  timeouts, cancellation, OAuth state, and per-server mutations remain bounded
  and race-safe.
- Active model context is always treated as exposed to untrusted content.
  External writes and exports pass through the action reviewer selected under
  Safety > Privacy; an unset or failed reviewer creates a durable approval
  request instead of executing.
- A clear reviewer decision may execute automatically. Otherwise approval is
  exact, revision-fenced, payload-bound, and consumed once. Foreground and Work
  task actions use the same authority; waiting task runs release their lease
  and resume through a pinned child run after the exact action outcome is
  recorded.
- Work reviewer authority comes from the exact source human item or
  authenticated Work UI request, never the rendered task prompt. Task actions
  are lease/generation fenced, and TaskReviewer has no export tools.
- Ready MCP reads remain directly available under their current fingerprint and
  policy. Ready MCP writes and exports are advertised through the governed
  gateway, which revalidates the live binding before approved execution.
- Web search and unobserved fetches are governed exports. Search-result URLs and
  links extracted from fetched HTML enter the local `observed_urls` authority;
  a later exact bare fetch may skip review, while every request still reruns
  current URL, DNS, and SSRF checks. Approved web replay is destination- and
  digest-bound; observations do not expire.
- The web app surfaces pending actions above the primary composer, in task
  detail, and in Work Needs You. The persisted action/event authority is
  delivery-neutral so another client can project the same attention state.
- Governed artifacts are versioned outputs owned by concrete contexts. Local
  writes use artifact storage helpers that reject traversal and symlinks;
  external versions accept validated HTTP(S) URLs only.
- The primary agent creates local files through `artifact.create_local_file`.
  Chat renders typed artifact references and opens local text/Markdown versions
  in the generic detail rail.

### Work

- Work is the durable workspace/project/task system. V1 seeds one Personal
  workspace and one executable workflow. Optional projects organize tasks
  without changing execution policy.
- `tasks.stage_id` is the workflow-state authority. Current run, gate, review,
  attention, and completion labels are derived projections, not parallel
  status fields.
- The seven stage behaviors are Intake, Dispatch, Active, HumanGate,
  Acceptance, TerminalSuccess, and TerminalCancelled.
- Semantic commands are revision- and generation-fenced, idempotent SQLite
  transactions. State updates, audit events, required notifications, and
  command receipts commit together.
- Planner, Executor, and Reviewer runs are bounded and supervised. Human gates,
  contracts, submissions, reviews, cancellation, retry, acceptance, and
  request-changes preserve generation and lease authority.
- The global `work_events` ledger is an audit and invalidation surface. Do not
  turn it into a second state authority or add replay infrastructure without a
  concrete runtime requirement.
- The primary chat surfaces concise task markers and human decisions; `/work`
  provides denser management. Both reuse the same task detail and server-owned
  action vocabulary.
- Task detail keeps a stable state, brief, activity, and metadata stack; only
  the current-state object and controls vary by stage, while available
  cancellation stays in the header.
- Current Work behavior and `docs/workspaces/README.md` are authoritative. The
  completed multi-agent implementation packets remain in Git history and should
  not drive new implementation.

### Frontend

- Noema product UI follows `docs/frontend/product-design.md` and the repo-local
  `noema-product-ui` skill.
- Design starts from the human's job, focal action, information priority, and
  semantic grouping. Productive surfaces use Astryx components and spacing
  tokens before one-off controls or raw values.
- Chat, detail rails, task transcripts, settings, and domain objects reuse
  existing Noema presentation patterns. Evidence and internals stay available
  through progressive disclosure instead of flattening every field into the
  default view.
- Stubbed actions and controls remain hidden until real backend operations
  exist. Backend field availability is not a requirement to display a field.
- Frontend generated GraphQL and route artifacts are generated, never edited by
  hand.

## Open Loops

- Complete third-party MCP installation/auth management, approval decision
  mutations, and richer audit persistence without reviving deprecated
  transports or implicit approval.
- Evaluate native-memory recall, citation accuracy, page churn, secret-copy
  behavior, and root growth before adding scopes, vectors, or editing.
- Decide which export formats ship first and how preview/redaction works.
- Add signing, notarization, updates, and production distribution after the
  unsigned developer desktop build is stable.
- Implement the approved system-error log contract from
  `docs/superpowers/specs/2026-07-02-system-errors-log-design.md` when it becomes
  the active slice.
- Revisit migrations only when persisted user-data compatibility becomes a
  real product requirement.
- Continue simplifying Work, Store, Runtime, Providers, and their test fixtures
  under measured net-negative slices. Do not start another repository-wide
  horizontal rewrite.

## Codex Workflow

- Be direct and implementation-oriented when asked to build, but preserve task
  modes so exploration, planning, implementation, review, and shipping do not
  blur together.
- Preserve unrelated dirty worktree changes. Work on main unless instructed
  otherwise and commit each finished unit; push only when asked.
- Use `docs/development/simplicity.md` for code/test budgets, the Rust size
  reporter, subagent ownership, test selection, review severity, and stop
  conditions.
- Prefer one vertical implementer and one read-only review pass. Additional
  agents or review rounds require a concrete independent slice or serious
  unresolved defect.
- Treat raw `~/.codex/sessions` as private source material. Read only when
  requested, summarize durable decisions, and never quote raw transcripts
  without explicit permission.
- Train references are welcome when they fit naturally.

## Validation Defaults

Rust implementation and refactoring:

```bash
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

Frontend/UI work uses `bun run gen:types`, `bun run lint`, and `bun run build`
from `apps/web`. Do not add frontend unit tests or use browser inspection unless
explicitly requested.

Before commit or push, run `git status --short --branch`, `git diff --check`,
and inspect `git diff --cached --stat` plus `git diff --cached --name-status`.
