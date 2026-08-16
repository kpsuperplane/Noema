# Documentation audit — 2026-08-16

## Outcome

This audit covers every project-owned Markdown file tracked by Git.

| Result | Files | Lines | Meaning |
| --- | ---: | ---: | --- |
| Delete now | 12 | 4,665 | Git already preserves completed work or removed designs. |
| Delete after extraction | 2 | 964 | Move one current rule or open acceptance item first. |
| Modify | 38 | 7,318 | Correct facts, status, terms, commands, or provenance. |
| Decision-led file | 4 | 1,610 | Keep the file until the owner resolves its contract conflict. |
| Retain unchanged | 6 | 1,610 | The file remains accurate and useful. |
| Total | 62 | 16,167 | Complete tracked Markdown baseline. |

The baseline excludes this report.

The most urgent defect is in `docs/project.md`.
Its complete-backup list omits several live source authorities.
This defect can cause data loss during recovery.

This report recommends changes.
It does not apply the recommended deletions or modifications.

## Scope

The audit includes 62 tracked `.md` files.
No tracked `.mdx` file exists.
Before this report, no untracked project Markdown file existed.

The audit excludes dependency, build, and runtime-state directories.
Examples include `node_modules/`, `target/`, and `.noema-dev/`.
Runtime state can contain private information.

The review compared prose with these sources:

- Current Rust types, constants, migrations, and path owners.
- GraphQL schema and frontend route definitions.
- Cargo workspace metadata and package scripts.
- iOS project settings and code-generation configuration.
- Git history for completed plans and renamed features.
- The terminology rules in `docs/development/terms.md`.

Automated checks covered local links, local anchors, tracked paths, package names, and documented versions.
One local link is broken.
It is inside a completed roadmap that this report recommends deleting.

The audit did not run browser or assistive-technology checks.
It did not rerun model evaluations or live connector cases.

## Priority findings

### P0 — Repair the backup contract

`docs/project.md:263-274` defines a complete backup.
The list omits live source authorities.

The omitted data includes:

- Human memory under `memory/`.
- Adapter definitions, connections, OAuth grants, and protected generations.
- Provider credentials under `providers/`.
- MCP configuration and protected credentials under `mcp/`.
- APNs provider authority under `notifications/`.
- Verified local model weights under `models/blobs/`.
- Task artifacts under `tasks/`.
- In-flight capability authentication under `run/capability-auth/`.

`crates/noema-home/src/paths.rs:79-100` and `148-218` define these active paths.
`docs/project.md:118-126` also names several omitted authorities.

Required change:

1. Tell users to back up the complete Noema home while the server is stopped.
2. List an exclusion only when restore tests prove that Noema rebuilds it.
3. Define restore behavior for in-flight capability authentication.

### P1 — Fix invalid configuration examples

`README.md:55-72` and `135-142` set an explicit model without `reasoning_effort`.
The host rejects that configuration.

`crates/noema-host/src/config/raw.rs:354-374` requires both values together.
`crates/noema-host/src/config/tests/default_provider.rs:21-35` tests the rejection.
The current default model is `gpt-5.6-luna`.

Remove each explicit model override.
Alternatively, add a valid `reasoning_effort` value.

### P1 — Replace the removed native pairing flow

`README.md:260-275` describes a ten-minute pairing link and one stored bearer credential.
That flow is no longer current.

The connection link now contains only the server origin.
Desktop authorization uses browser OAuth, PKCE, and passkey approval.
The operating-system store keeps the rotating refresh credential.
The access token remains transient.

Update the root README and native-client documents.
Use “connection” or “native OAuth,” not “pairing.”

Affected files:

- `README.md`
- `apps/ios/README.md`
- `docs/frontend/current-contract.md`
- `docs/project.md`

### P1 — Replace adapter manifest version 6

`docs/harness/capabilities.md:71-157` presents manifest version 6 as current.
It also describes automatic version 5 conversion.

The compiler accepts only manifest version 9.
See `crates/noema-capabilities/adapters/src/compiler.rs:203` and `294-296`.

Replace the version 6 contract with a compact version 9 contract.
Remove completed conversion and initial-slice sections.

### P1 — Remove two obsolete design authorities

`docs/frontend/governance-inspection.md` describes removed routes and GraphQL fields.
Examples include `memoryGraph` and `/settings/mcps`.

Current routes live in `apps/web/src/app/routes.ts:23-98`.
Old flat settings routes resolve to Chat.
The current memory model uses articles, pages, changes, citations, and search results.

`docs/harness/memory-context.md` describes an unimplemented graph-memory design.
`docs/memory.md:14` already marks it as historical.
No current source defines `ContextPacket` or `MemoryUseRecord`.

Delete both files.
Remove `memory-context.md` from the current authority list in `docs/harness.md`.

### P1 — Reconcile action and security policies

`docs/harness/action-governance.md` omits current action states.
The store defines eleven states in `crates/noema-store/src/governed_actions.rs:64-120`.

`docs/harness/security.md:381-406` describes reusable, expiring, modified, and revoked approvals.
The current decision API supports approval and decline only.
See `crates/noema-store/src/governed_action_approvals.rs:10-17`.

The two documents also disagree about reviewed external writes.
The questions table records the required product decisions.

### P1 — Reduce the current project brief

`docs/context/current.md` says that Git owns completed milestone history.
However, most of the file records completed migrations and validation results.

Move durable rules into their subsystem authorities.
Remove completed execution history.
Keep active direction, current constraints, recent decisions, and open loops.

The file also lists an incomplete set of baseline gate failures.
Decide whether this brief should contain volatile validation failures.

### P2 — Correct current product names and routes

Several current documents still use `Work` as the product surface.
The current route is `/tasks`.
`/work` resolves to Chat in `apps/web/src/app/routes.test.ts:80`.

Replace obsolete product terms in:

- `docs/frontend/pwa.md`
- `docs/harness.md`
- `docs/harness/action-governance.md`
- `docs/harness/web-browsing.md`
- `docs/workspaces/README.md`
- `.agents/skills/noema-product-ui/SKILL.md`

`docs/frontend/pwa.md:43` also names `HumanInterventionCard`.
The current GraphQL projection is `HumanIntervention`.

### P2 — Separate current contracts from historical evidence

The audit index calls dated snapshots “Current.”
Several completed plans remain beside active contracts.
Several evaluation reports call old suite results current.

Use three clear labels:

- Current contract.
- Dated evidence snapshot.
- Active approved plan.

Delete completed execution plans when a current authority preserves durable rules.
Add cutoff dates and source revisions to retained evidence.

### P2 — Correct contributor and client guidance

The root and folder READMEs contain smaller factual defects.

- Use the Bun-installed Tauri CLI, not “Cargo Tauri CLI.”
- List all supported chat provider types.
- State the exact `cargo dev` platform limits.
- Add the Foundation bridge watcher to development behavior.
- Add the missing workspace members to the layout.
- Document an exact Apollo CLI installation method.
- State the Live Activity extension signing requirement.
- Describe exact Foundation bridge build conditions.
- Replace “verbatim license” unless the font license bytes become exact.

## Delete recommendations

### Delete now

Historical status alone does not justify deletion.
These files duplicate completed execution or removed designs.
`docs/audits/README.md:4-7` also limits implementation plans to active work.

| File or group | Reason |
| --- | --- |
| `.superpowers/sdd/task-1-fix-report.md` | Completed scratch report with removed SurrealDB paths. |
| `.superpowers/sdd/task-1-report.md` | Completed artifact task report with old crate ownership. |
| `.superpowers/sdd/task-11-fix-report.md` | Completed UI task report with transient review notes. |
| `.superpowers/sdd/task-3-fix2-report.md` | Completed MCP fix report with removed tests. |
| `.superpowers/sdd/task-3-report.md` | Completed artifact report with removed paths. |
| `.superpowers/sdd/task-5-report.md` | Completed artifact projection report. |
| `.superpowers/sdd/task-8-fix-report.md` | Completed report for a removed GraphQL feature. |
| `docs/audits/2026-08-08-codex-conversation-remediation-plan.md` | The completed reliability roadmap superseded this proposal. |
| `docs/audits/2026-08-08-overengineering-roadmap.md` | The remediation report superseded this historical execution plan. |
| `docs/frontend/governance-inspection.md` | Removed routes, memory fields, and inspection scope. |
| `docs/harness/memory-context.md` | Historical, unimplemented graph-memory design. |
| `docs/superpowers/plans/2026-08-08-noema-practical-reliability-roadmap.md` | Completed 1,858-line plan with obsolete paths. |

The reliability roadmap contains the only broken local link.
`docs/superpowers/plans/2026-08-08-noema-practical-reliability-roadmap.md:203` names removed `strict_schema.rs`.
Its current successor is `provider_schema_conversion.rs`.

### Delete after extraction

| File | Extract first |
| --- | --- |
| `docs/frontend/experience-layers.md` | Move approved onboarding, provider-choice, model-review, and disclosure rules into current contracts. |
| `docs/plans/2026-08-12-reusable-oauth-applications-and-grants.md` | Record any remaining live Google acceptance item in the current brief. |

The frontend file presents completed rollout stages as future work.
It also conflicts with the visible Tasks and Memory routes.

The OAuth file records a completed implementation across 681 lines.
Git preserves the implementation sequence.
A short active acceptance item is sufficient.

## Required modifications

| Priority | File or group | Required change |
| --- | --- | --- |
| P0 | `docs/project.md` | Correct the home tree and backup contract. Replace paired-client wording. |
| P1 | `README.md` | Fix invalid models, OAuth flow, providers, requirements, and workspace layout. |
| P1 | `docs/harness/capabilities.md` | Replace version 6 and version 5 conversion with strict version 9. |
| P1 | `docs/harness/action-governance.md` | Add current states. Remove implementation sequencing. Apply approved terms. |
| P1 | `docs/harness/security.md` | Separate implemented policy from future policy. Resolve approval conflicts. |
| P1 | `docs/context/current.md` | Remove milestone history. Keep only current direction and open loops. |
| P1 | `docs/audits/README.md` | Index snapshots as historical. Remove deleted-plan links. |
| P1 | `docs/plans/2026-08-15-proactive-event-sources.md` | Define Gmail authentication and missed-event recovery. Mark exact plan status. |
| P2 | `CLAUDE.md` | Keep only the pointer to `AGENTS.md`. Delete the conflicting Astryx copy. |
| P2 | `AGENTS.md` | Use the locked Astryx command. Apply the stored-state terminology. |
| P2 | Current frontend documents | Replace Work and pairing terms. Correct intervention names. |
| P2 | Current storage documents | Apply source-data and stored-state terms. Correct partial path trees. |
| P2 | Historical audits | Add cutoff status. Remove live-sounding headings and deleted-plan links. |
| P2 | Validation and evaluation records | Add suite versions, revisions, cutoffs, and evidence-retention status. |
| P3 | iOS and provider READMEs | Correct prerequisites, signing, generated paths, and build conditions. |
| P3 | Font license index | Use “license text,” or restore byte-identical upstream files. |

## Questions for the project owner

| ID | Conflict | Evidence | Recommended decision |
| --- | --- | --- | --- |
| Q1 | Backup behavior for in-flight authentication is undefined. | `docs/project.md:119`; `crates/noema-home/src/paths.rs:148-169` | Back up the complete home. Cancel transient authentication during restore. |
| Q2 | Security requires approval for writes. Action governance can auto-execute reviewed writes. | `docs/harness/security.md:240-243`; `crates/noema-store/src/governed_actions.rs:952-965` | Define exact write-risk classes. Then align code and both contracts. |
| Q3 | The schema permits approval revocation. No production command performs it. | `crates/noema-store/src/schema.rs:1063-1078`; `governed_action_approvals.rs:10-17` | Remove dormant revocation unless a named product flow needs it. |
| Q4 | Security describes private-memory grants. Current memory excludes private scopes. | `docs/harness/security.md:484-506`; `docs/memory.md:69-70` | Move the future policy out of the current contract. |
| Q5 | The old experience plan hides Tasks during onboarding. The UI shows Tasks. | `docs/frontend/experience-layers.md:273-283`; `apps/web/src/components/onboarding/ModelSetup.tsx:30-50` | Treat the current UI as correct. Extract all approved onboarding rules before deletion. |
| Q6 | The proactive plan promises lossless Gmail delivery. Gmail notifications can be dropped. | `docs/plans/2026-08-15-proactive-event-sources.md:20-40` | Permit a bounded recovery check. Define callback authentication before implementation. |
| Q7 | OAuth code is active. The completed plan leaves live Google acceptance open. | `docs/context/current.md:92-103`; OAuth plan acceptance sections | Track one short acceptance item. Delete the implementation plan. |
| Q8 | Model recommendations use older suites. Current plan validation requires suite version 9. | `crates/noema-providers/src/recommendations.rs:85`; `crates/noema-runtime/src/eval_support/mod.rs:9` | Define an evidence-validity policy. Rerun only when that policy requires it. |
| Q9 | Raw evaluation and incident evidence is machine-local or ignored. Summaries remain tracked. | Eval reports and audit snapshot references | Decide whether summaries are sufficient durable evidence. |
| Q10 | A2UI stores pinned JSON schemas. Runtime validation uses handwritten checks. | `crates/noema-runtime/src/a2ui/validation.rs:175`; `crates/noema-runtime/src/a2ui/component_validation.rs:10` | Mark JSON as reference-only, or make it runtime authority. |
| Q11 | The current brief lists selected gate failures. Current failures differ. | `docs/context/current.md:198-203`; current `cargo gate-lint` baseline | Keep volatile failures in an issue system, not the project brief. |
| Q12 | The web UX audit left browser and assistive checks open. | `docs/audits/2026-08-10-web-ux-audit.md:784-820` | Create an owned validation task, or close it as static-only evidence. |
| Q13 | `cargo dev` enables a Unix socket, but the README does not state a platform limit. | `crates/noema-dev/src/main.rs:80`; `README.md:12-20` | Support Windows explicitly, or document the Unix requirement. |

## Complete file disposition

Legend:

- D: delete now.
- X: extract current material, then delete.
- M: modify.
- Q: keep until the owner answers a contract question.
- R: retain unchanged.

| File | Result | Main action |
| --- | :---: | --- |
| `.agents/skills/noema-product-ui/SKILL.md` | M | Replace Work. Prefer existing domain components. Add exact checks. |
| `.superpowers/sdd/task-1-fix-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-1-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-11-fix-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-3-fix2-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-3-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-5-report.md` | D | Remove tracked scratch history. |
| `.superpowers/sdd/task-8-fix-report.md` | D | Remove tracked scratch history. |
| `AGENTS.md` | M | Use locked commands and approved stored-state terms. |
| `CLAUDE.md` | M | Keep only the pointer to `AGENTS.md`. |
| `README.md` | M | Correct configuration, OAuth, providers, and layout. |
| `apps/ios/Noema/Fonts/FONT-LICENSES.md` | M | Correct the byte-identical license claim. |
| `apps/ios/Noema/Generated/README.md` | R | Folder ownership and generated path are current. |
| `apps/ios/Noema/Operations/README.md` | M | Say source schema. Clarify the configured relative path. |
| `apps/ios/README.md` | M | Add Apollo setup, signing, and current OAuth terms. |
| `crates/noema-providers/apple-foundation-bridge/README.md` | M | State exact debug, Swift, and macOS requirements. |
| `crates/noema-runtime/a2ui/README.md` | Q | Decide whether vendored JSON is runtime authority. |
| `docs/audits/2026-08-08-codex-conversation-remediation-plan.md` | D | Superseded proposal. |
| `docs/audits/2026-08-08-codex-conversation-retrospective.md` | M | Add cutoff status and superseding authority. |
| `docs/audits/2026-08-08-current-build-ux-survivors.md` | M | Mark live-status claims as superseded. |
| `docs/audits/2026-08-08-noema-dev-user-experience-history.md` | M | Mark raw snapshots as machine-local evidence. |
| `docs/audits/2026-08-08-overengineering-audit.md` | M | Link remediation instead of the deleted roadmap. |
| `docs/audits/2026-08-08-overengineering-evidence.md` | R | Exact historical baseline remains clear. |
| `docs/audits/2026-08-08-overengineering-remediation.md` | M | Rename remaining actions as completion-time status. |
| `docs/audits/2026-08-08-overengineering-roadmap.md` | D | Completed historical execution plan. |
| `docs/audits/2026-08-10-web-ux-audit.md` | M | Add source revision, cutoff, and static-only status. |
| `docs/audits/README.md` | M | Index dated snapshots and this audit by status. |
| `docs/common-personal-agent-tasks.md` | M | State that unchecked boxes define cases, not results. |
| `docs/context/current.md` | M | Remove history and volatile validation details. |
| `docs/development/simplicity.md` | R | Commands and simplicity rules are current. |
| `docs/development/terms.md` | R | Current authority governs new prose and changed identifiers. |
| `docs/frontend/current-contract.md` | M | Replace legacy pairing terms. |
| `docs/frontend/experience-layers.md` | X | Extract approved disclosure rules, then delete. |
| `docs/frontend/governance-inspection.md` | D | Removed routes and memory model. |
| `docs/frontend/product-design.md` | R | Current design authority matches implementation. |
| `docs/frontend/pwa.md` | M | Replace Work and `HumanInterventionCard`. |
| `docs/harness.md` | M | Remove historical memory authority. Replace Work terms. |
| `docs/harness/action-governance.md` | Q | Resolve approval policy and terminology conflicts. |
| `docs/harness/capabilities.md` | M | Replace manifest version 6 with version 9. |
| `docs/harness/memory-context.md` | D | Historical unimplemented design. |
| `docs/harness/security.md` | Q | Separate implemented and future approval or memory policy. |
| `docs/harness/web-browsing.md` | M | Replace Work and governed-action prose. |
| `docs/memory.md` | M | Remove first-slice wording. Apply stored-state terms. |
| `docs/plans/2026-08-12-reusable-oauth-applications-and-grants.md` | X | Extract open live acceptance, then delete. |
| `docs/plans/2026-08-15-proactive-event-sources.md` | Q | Confirm status, authentication, and recovery behavior. |
| `docs/project.md` | M | Repair the backup contract and home layout. |
| `docs/server-security.md` | R | Current server-security contract matches implementation. |
| `docs/sqlite.md` | M | Label its home tree as partial. Apply stored-state terms. |
| `docs/superpowers/plans/2026-08-08-noema-practical-reliability-roadmap.md` | D | Completed plan with obsolete paths. |
| `docs/validation/personal-agent-50-case-ledger.md` | M | Mark completed snapshot. Add revision and runtime profile. |
| `docs/workspaces/README.md` | M | Move to `docs/tasks.md`. Apply current Tasks terms. |
| `evals/local-models/README.md` | M | Replace the stale 21-case count. Current qualification has 31. |
| `evals/local-models/results/m5-air-16gb-tier-2026-07-15.md` | M | Add superseded status and current-result link. |
| `evals/local-models/results/m5-air-2026-07-15.md` | M | Describe absent raw reports as machine-local history. |
| `evals/local-models/results/m5-air-32gb-unified-tools-2026-07-15.md` | M | Add historical status and current-result link. |
| `evals/local-models/results/m5-air-live-noema-2026-07-15.md` | M | Say then-current 12-case suite. Add requalification status. |
| `evals/model-matrix/README.md` | M | Add a decision index and suite version status. |
| `evals/model-matrix/decisions/2026-08-07-22a4888b140c-incomplete.md` | M | Mark resolved by the suite version 8 decision. |
| `evals/model-matrix/decisions/2026-08-07-3ec95dc4969f.md` | M | Add historical status. Correct raw-evidence wording. |
| `evals/model-matrix/decisions/2026-08-07-7c5cae8f832c.md` | M | Mark partially superseded. Link later decisions. |
| `evals/model-matrix/decisions/2026-08-07-abc9d520bb82.md` | M | Mark latest applied decision before suite version 9. |
| `evals/model-matrix/decisions/2026-08-07-c927053f7825.md` | M | Mark Action Reviewer results superseded by version 8. |

## Recommended change order

1. Repair `docs/project.md` and the invalid root README examples.
2. Answer Q1 through Q4 before changing restore or security policy.
3. Delete the twelve high-confidence stale files.
4. Extract the two remaining live items and delete their completed plans.
5. Update current subsystem contracts.
6. Add historical status and provenance to retained evidence.
7. Update the audit index after the cleanup lands.

## Validation notes

The inventory used `git ls-files '*.md' '*.mdx'`.
Before this report, the untracked-file check found no project Markdown.

The local link check found one missing target.
The local anchor check found no missing target.
Cargo metadata confirmed every documented package example checked by this audit.
The Astryx lock resolves version 0.1.9.
The SQLite store schema is version 47.
The adapter compiler requires manifest version 9.
The current local-model qualification contains 31 scored cases.

Unrelated Rust test changes appeared during this audit.
This report does not include or modify those changes.
The existing untracked validation temporary file also remains outside this report.
