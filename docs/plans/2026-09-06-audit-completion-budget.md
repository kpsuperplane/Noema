# Audit completion within the remaining budget

Mode: plan only. Implementation remains a separate phase.

## Outcome and limits

Complete every automatically testable case and fix demonstrated failures.
Leave only cases that require human intervention for later human testing.
Keep the original acceptance requirements, including required variants.
Do not count controlled checks as proof of physical-device or real-provider behavior.

The user reports approximately 10% of the weekly token budget remains.
Its absolute token count is unavailable. Percentages below refer to that remaining budget.
Completion is the target, not a guarantee that unknown defects will fit.

Current ledger: 241 cases; 92 Pass, 72 Partial, 52 Not run, 22 Fail,
one blocked, and two Not applicable. The backlog contains 22 defects.
Some incomplete rows contain human-only variants. Separate these before estimating automatic work.

## Budget allocation

| Phase | Remaining budget | Deliverable |
| --- | ---: | --- |
| Reconcile evidence and dependencies | 5% | Exact automatic gaps and explicit human exceptions |
| Finish the automatic first pass | 20% | Every automatic variant attempted; failures have useful reproductions |
| Fix grouped causes and recheck | 55% | All demonstrated automatic failures resolved |
| Final validation and case reconciliation | 15% | Current evidence for all automatic passes |
| Reserve | 5% | Unexpected blockers and final corrections |

Use budget checkpoints after each phase. Report completed cases, unresolved causes, and remaining capacity.
If budget telemetry is unavailable, report that limitation rather than inventing a token balance.
At a checkpoint, an expected shortfall must be reported immediately. Do not lower acceptance requirements.

## 1. Reconcile once

- Read the case table and backlog once. Thereafter, read only affected rows and source files.
- Review the four unfinished evidence files. Use only completed, attributable assertions.
- Reuse earlier passing checks when their code, configuration, environment, and test inputs remain applicable.
- For each Partial row, identify the exact missing assertion instead of repeating the whole case.
- Move actual device, platform, credential, or consent requirements into the existing Human variant column.
- Keep all feasible controlled portions in the automatic scope.
- Record exact human steps and the reason automation cannot complete them.
- Do not classify a difficult implementation or broken automatic setup as human-only work.

## 2. Complete the first pass with shared setups

Run related cases together. Reuse existing services, scripts, profiles, and saved reproductions.
Do not build another general audit framework or one new driver for each case.

| Shared setup | Case families and missing work |
| --- | --- |
| Isolated Linux homes and browser profiles | HOME, AUTH, SETUP, OPS; restart, restore, optional access, and package gaps |
| Existing deterministic model services | MODEL, AGENT, CHAT, MEM, INFO; routing, failures, context, and information preservation |
| Existing OAuth, MCP, API, and receipt services | API, MCP, ACTION; lifecycle, account boundaries, decisions, and uncertain outcomes |
| Authenticated desktop and phone browser contexts | CLIENT, PWA, NOTE, UX, DIAG; remaining visible states and recovery |
| Existing populated Task, Project, and Artifact setup | Remaining RUN and FILE variants; JOURNEY-01, 05, 06, 07, 09, and 11 |

Use noema.kevinpei.com for live web checks. Use isolated homes for destructive lifecycle checks.
Keep fake services available for their related checks, then remove the controlled setup once.
Use deterministic responses for protocol and recovery cases. Use live models only where synthesis itself needs verification.
One journey can cover several cases only when each case has its own explicit assertion.

After a failure, capture its cause or smallest useful reproduction and continue independent cases.
Do not repeatedly retry the same failed journey. Fix blockers early only when they prevent further testing or risk test data.

## 3. Fix by cause, then recheck dependent cases

Priority determines order, not which defects can remain unresolved.

| Order | Batch | Existing defects | Closure check |
| --- | --- | --- | --- |
| 1 | Browser authority and privacy | AUDIT-11, 12, 13 | Changed targets and declined effects cannot execute; secret values stay out of snapshots; ordinary values remain intact |
| 2 | Chat continuation | AUDIT-10, 22 | Recheck after choice simplification; fix remaining causes; complete browser and packet journeys |
| 3 | Task execution and gate controls | AUDIT-01, 02, 03, 04, 05, 09 | Correct retry role, answer context, permitted reads, downloads, and visible human response controls |
| 4 | OAuth and native callback lifecycle | AUDIT-06, 07, 08, 19 | Revoked-grant deletion, expanded operations, registered-client reuse, and browser cancellation callback |
| 5 | Notifications and client recovery | AUDIT-14, 15, 16, 17, 18 | Approval notifications, permission state, cross-client settings, preserved drafts, and reconnect |
| 6 | Remaining Task UI failures | AUDIT-20, 21 | Query retry and forward keyboard navigation at both widths |

Add newly discovered defects to the existing backlog and the nearest cause group.
Do not assume AUDIT-10 and AUDIT-22 have the same cause or are already fixed.
Use the smallest production correction. Preserve live capabilities and avoid adjacent refactors.
Reuse saved regression reproductions. Add a unit test only for a distinct uncovered risk.
Do not add frontend tests. Inspect changed UI states at desktop and phone widths.

Run focused checks while editing. Run required broad checks once for each completed coherent code batch.
Reuse successful checks with unchanged inputs. Repeat affected checks after a correction.
Commit each completed batch. No push is planned.

## 4. Final acceptance

- Every required automatic variant has a passing assertion tied to the applicable code and environment.
- No automatic Fail, Not run, Blocked, or unexplained Partial remains.
- Every backlog defect is resolved and links to a successful affected-case recheck.
- Human exceptions state the unavailable requirement and remaining procedure.
- Changed code passes required language checks. Changed frontend code passes lint, build, and visual inspection.
- Final journeys cover repaired dependencies without repeating unrelated passing journeys.
- Go production and inclusive size ratios remain below 80%.
- Results retain only necessary context and the case table. The backlog retains fix status separately.

## Execution discipline

Keep tool output bounded. Extract failures and changed case counts instead of printing entire logs or documents.
Run independent commands together when useful. Do not introduce agents or delegate work without authorization.
Use concise progress updates. Store detailed evidence in files, not conversation prose.
Avoid repeated source surveys, repeated broad validation, and new narrative reports.
Reserve the final 20% for validation and corrections; do not spend it polishing the audit tooling.

If that reserve is threatened, report the exact unresolved cases and causes immediately.
Keep failures visible. A budget limit never turns an unverified case into a pass or a human exception.
