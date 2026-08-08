# Noema remediation plan for repeated build problems

- **Status:** Proposed plan
- **Mode:** Plan only
- **Source report:** [Codex conversation retrospective](2026-08-08-codex-conversation-retrospective.md)
- **Issue baseline:** [Current-build UX audit](2026-08-08-current-build-ux-survivors.md)
- **Source revision:** `5b16455c`
- **Language standard:** ASD-STE100 Simplified Technical English, Issue 9

## Language rules

This plan uses the current [ASD-STE100 Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf).
Issue 9 has the date 2025-01-15.

This plan applies these rules:

- Use short and clear sentences.
- Use one instruction in each procedural sentence.
- Use the imperative form for instructions.
- Use a maximum of 20 words in each procedural sentence.
- Use a maximum of 25 words in each descriptive sentence.
- Use vertical lists for complex information.
- Do not use semicolons.
- Use one term for one meaning.

Noema terms and code identifiers are technical nouns.

## Objective

Reduce failures at the boundaries between Noema systems.

Prove each correction at the first user-visible terminal state.

Keep one authority for each product fact.

## Non-goals

- Do not make a new general framework.
- Do not retire a live product function.
- Do not change an open product decision.
- Do not add compatibility without a named source.
- Do not make all corrections in one change.

## Technical terms

| Term | Meaning in this plan |
| --- | --- |
| Authority | The one source that controls a product fact. |
| Boundary | A point where one Noema system gives data to another system. |
| Vertical scenario | One user job from its start to its terminal result. |
| Proof level | A named type of evidence for a completed change. |
| Failure identity | A stable value that identifies one failure and its input. |
| Existing-state test | A test that starts with durable state from an older version. |

## Work rules

1. Complete the steps in the specified order.
2. Make each step an independent implementation unit.
3. Use one commit for each completed unit.
4. Run the size report before each commit.
5. Stop when a unit exceeds its code budget.
6. Stop when a unit needs a new public abstraction.
7. Stop when a unit changes a product decision.
8. Use one read-only review after each unit.
9. Do one correction pass for each material defect.
10. Update the current context after each major phase.

## Proof levels

Use these proof levels in each final handoff.

| Level | Evidence |
| --- | --- |
| P1 | The source change is complete. |
| P2 | The focused contract test passes. |
| P3 | The exact vertical scenario passes. |
| P4 | The applicable restart or upgrade scenario passes. |
| P5 | The applicable client view has a visual check. |
| P6 | The aggregate repository gate passes. |

Do not use the word `fixed` without P3 evidence.

Use `implemented` when only P1 or P2 evidence exists.

## Unit budgets

| Step | Production lines | Test lines | New tests | Primary stop condition |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0 to 150 | 0 to 80 | 0 to 2 | A desktop function or compiler-cache rule changes. |
| 2 | 0 | 0 | 0 | The change duplicates the simplicity workflow. |
| 3 | 200 to 600 | 140 to 300 | 6 to 10 | The change needs a new Work state machine. |
| 4 | 150 to 500 | 140 to 300 | 5 to 9 | Fresh state and upgraded state do not converge. |
| 5 | 150 to 500 | 100 to 240 | 4 to 8 | Readiness needs a second connector registry. |
| 6 | 150 to 450 | 100 to 220 | 4 to 8 | A fixture gives the inference under test. |
| 7 | 50 to 300 | 0 | 0 | The human does not authorize visual proof. |
| 8 | Net-negative to 250 | 20 to 120 | 1 to 4 | The measure does not show a material gain. |

Step 2 can add 20 to 60 documentation lines.

Each budget applies to one implementation unit.

Run Step 4 once for each selected authority.

Run Step 5 once for each integration substrate.

Run Step 8 once for each selected hot path.

## Step 1: Confirm the baseline and restore the gates

### Actions

1. Record the current Git revision.
2. Copy the `.noema-dev` database to a protected test location.
3. Copy the error log to the same test location.
4. Run the current-build UX audit again.
5. Remove each resolved issue from the active issue table.
6. Group the issues that remain by authority and boundary.
7. Reproduce the Linux Tauri feature failure.
8. Correct the feature selection in its authority.
9. Reproduce the current MCP lint failure.
10. Correct the lint failure in its authoritative crate.
11. Run the standard Rust validation commands.
12. Record the tests that Linux cannot run.
13. Define one separate macOS validation obligation.
14. Keep `sccache` enabled for all commands.
15. Do not bypass a platform build.

### Required evidence

- One current issue table
- One source revision
- One test-state revision
- One owner for each selected issue
- A green standard Linux gate
- A written macOS obligation

## Step 2: Add vertical proof to the work process

### Actions

1. Extend the existing implementation brief.
2. Add one vertical scenario to the brief.
3. Add the applicable proof levels to the brief.
4. Add one existing-state risk question.
5. Add one client-view risk question.
6. Add the proof levels to the final handoff format.
7. Do not make a separate checklist authority.
8. Use the new brief in all later steps.

### Required evidence

- One sample bug brief
- One sample feature brief
- One sample UI brief
- One sample final handoff

## Step 3: Correct Work boundaries and repeat loops

### Scope

- Work reconciliation
- Governed-action origin
- Contract and review identity
- Lease expiry
- Review evidence
- Failure retry
- Task notifications

### Actions

1. Trace one action from proposal to terminal delivery.
2. Record the authority at each boundary.
3. Remove fallback reads of unrelated latest records.
4. Preserve the exact origin through approval and restart.
5. Preserve the exact contract through reopen and recovery.
6. Make invariant failures non-retryable.
7. Give an expired lease one clear recovery state.
8. Give the reviewer evidence for the current contract.
9. Preserve the failure category through storage and display.
10. Preserve retry information through storage and display.
11. Preserve the recovery action through storage and display.
12. Add a stable failure identity.
13. Stop an identical deterministic call after one failure.
14. Suppress a notification without new progress.
15. Do not make a second action store.
16. Do not use English text as authority.

### Required tests

1. Resume an approved action after a runtime restart.
2. Deliver the result to its original conversation item.
3. Reopen a task without the old review.
4. Reject a foreign review before execution.
5. Stop one invariant failure after one attempt.
6. Recover one expired lease without duplicate work.
7. Suppress one duplicate task notification.
8. Keep a notification after a material state change.

## Step 4: Correct old-state transitions and state truth

### Scope

- Database schemas
- Adapter digests and replacement lineage
- Open approvals and browser sessions
- Task contracts and compaction summaries
- Memory provenance
- Terminal transcript status
- Runtime debug spans

### Actions

1. Name the oldest supported source for each authority.
2. Write one existing-state scenario for each selected authority.
3. Write one fresh-state scenario for each selected authority.
4. Compare the terminal states from both scenarios.
5. Add a forward-only migration for each schema correction.
6. Never change a migration that can exist in `.noema-dev`.
7. Invalidate stale state when safe recovery is not possible.
8. Keep execution values out of compaction prose.
9. Validate memory sources before the memory write.
10. Close terminal transcript items in the canonical store.
11. Close debug spans when their related work ends.
12. Do not add general compatibility.
13. Do not make another state authority.

### Required tests

1. Upgrade the oldest supported database version.
2. Build a fresh database at the same schema version.
3. Resolve adapter lineage across one digest change.
4. Reject one approval after its browser session ends.
5. Prevent a legacy summary from giving an execution value.
6. Reject one invalid memory source before persistence.
7. Close one tool call and one run at terminal state.

## Step 5: Make integration readiness accurate

### Actions

1. Define readiness with the existing connector state.
2. Verify the compiled operation contract.
3. Verify each required response transform.
4. Verify the approved authentication scope rules.
5. Verify approval-time capability resolution.
6. Verify one representative read operation.
7. Verify one representative write operation when applicable.
8. Use an isolated test account for external effects.
9. Read back each test write.
10. Report `connected` when only credentials exist.
11. Report `ready` only after the operation proof passes.
12. Remove repeated schema fallback messages from the error log.

### Required scenarios

1. Read one Gmail message through its active transform.
2. List and update one Calendar event with exact identity.
3. Complete one Google OAuth flow with combined scopes.
4. Approve and run one built-in browser action.
5. Run one Notion tool without schema fallback noise.

## Step 6: Qualify models with real user jobs

### Actions

1. Keep the original ambiguous request in each primary case.
2. Use the production tool names and schemas.
3. Use the production tool-context size.
4. Include discovery, inspection, interpretation, and action.
5. Include one safe-restraint result.
6. Keep each model effort as a separate candidate.
7. Stop a run when a provider error occurs.
8. Resume only the results that remain valid.
9. Compare finalists within a defined tie margin.
10. Use latency and cost after quality qualification.
11. Do not change defaults from a partial run.

### Required scenarios

- Add a flight from an ambiguous request.
- Reschedule a meeting with exact event identity.
- Find an email fact before a calendar write.
- Decline a write when source facts are not available.
- Continue after one recoverable tool error.

## Step 7: Make client views use one product meaning

### Actions

1. Select one semantic projection for each affected state.
2. Use that projection for loading, live, replay, and recovery.
3. Replace raw tool names with user action names.
4. Replace internal status words with product words.
5. Show one current action for one connector version.
6. Show the exact target in an approval view.
7. Show reviewer reasons when they affect the decision.
8. Remove duplicate nearby task references.
9. Use Astryx components and spacing tokens.
10. Ask for browser inspection permission before source changes.
11. Reuse existing components.

### Required visual checks

- Mobile loading to live transcript
- Desktop loading to live transcript
- Approval before and after a restart
- Waiting, recovery, and completion task references
- Connector setup after a version change
- Narrow and wide composer states

Do not add frontend tests unless the human requests them.

## Step 8: Remove hot-path work and close the program

Do Actions 12 through 19 only in the last Step 8 unit.

### Actions

1. Measure each selected hot path before source changes.
2. Classify each operation by its change frequency.
3. Move version-stable work to the version boundary.
4. Move runtime-stable work to runtime startup.
5. Keep call-time authorization checks at the call boundary.
6. Keep call-time argument checks at the call boundary.
7. Keep provider-response checks at the response boundary.
8. Remove duplicate catalog construction in one turn.
9. Reduce repeated tool schemas in model continuations.
10. Measure each selected hot path after the change.
11. Do not add a new cache authority.
12. Run the standard validation commands.
13. Run each selected vertical scenario.
14. Run each selected existing-state scenario.
15. Complete each authorized visual check.
16. Run the current-build UX audit again.
17. Compare the results with the Step 1 baseline.
18. Move durable rules to the authoritative subsystem document.
19. Remove duplicate guidance and temporary plans.

### Required measures

- Connector call latency
- Foreground GraphQL latency
- Foreground turn latency
- Tool-schema input tokens
- Incremental Rust build time
- Frontend rebuild time

## Program exit criteria

- No selected Work boundary defect is current.
- No selected deterministic failure repeats.
- No selected integration reports false readiness.
- No selected model default uses partial evidence.
- No selected UI state has two semantic projections.
- The standard Linux validation gate is green.
- Each correction has the applicable proof levels.
- Production and test budgets are within their limits.

## Deferred decisions

This plan does not select these product directions:

- The future adapter scheduler
- The long-term integration substrates
- Full parity between web and iOS
- The full breadth of Work roles
- The future of secondary product systems

Stop and ask the human before a step requires one of these decisions.

## Sequence summary

```text
1. Confirm the baseline and restore the gates.
2. Add vertical proof to the work process.
3. Correct Work boundaries and repeat loops.
4. Correct old-state transitions and state truth.
5. Make integration readiness accurate.
6. Qualify models with real user jobs.
7. Make client views use one product meaning.
8. Remove hot-path work and close the program.
```
