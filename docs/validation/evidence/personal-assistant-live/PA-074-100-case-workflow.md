# PA-074 to PA-100 live case-workflow evidence

Updated: 2026-09-10

This report records the sequential run of PA-074 through PA-100 against the
live Go Noema development server. Each task received a short conversational
request. The operator inspected the task state, approval gate, action request,
fixture ledger, result, and review before starting the next case.

## Test boundary

- Backend: live Noema development server through
  `/tmp/noema-codex/graphql.sock`.
- Connection: `definition:synthetic_case_workflow_api_v1`, connection
  `2d79d72190362a19d0a25ab4af1c607b`.
- Fixture: `2026-09-10-case-workflow-api-v1` at
  `https://hood-marker-tests-firewall.trycloudflare.com/`.
- Operations: one bounded context read, one approval-gated action record, and
  one status read for each clean case.
- Ledger: `/tmp/noema-case-workflow-ledger.json`, loaded and saved by the
  fixture process.
- Safety: the fixture has no real provider, account, recipient, device, file,
  payment, travel, school, or household side effect. It cannot move money or
  release credentials.

The call counts in this table come from the fixture request ledger and the
persisted `task_run_items` records. A model-written `RESULT.md` count is treated
as a claim to check, not as the call-count authority.

## Case results

| Case | Task ID | Connector calls observed | Recorded outcome | Verdict and notes |
| --- | --- | --- | --- | --- |
| PA-074 | `task:e8f47a46ce6eae2222573703b7fd8479` | context 1, action 1, status 1 | Selected accessible route B, replaced expired supplies, kept the dog plan, and retained an offline packet requirement. Action `pa-074-action-001`. | Pass. Synthetic-only limitation is stated. |
| PA-075 | `task:d7b1546db286e73b39abed7d1641cc66` | context 1, action 1, status 1 | Merged only the duplicate booking email, kept booking references and local times, and left the missing transfer detail open. Action `pa-075-action-001`. | Pass. |
| PA-076 | `task:5874b7820785c5e8989179f300dc6890` | context 1, action 1, status 1 | Recorded one trip booking plan. The possible post-commit 502 was treated as uncertain and was not resubmitted. Action `pa-076-action-001`. | Pass. |
| PA-077 | `task:ca6738724cef91d1630eeea3b7dc37da` | context 1, action 1, status 1 | Rejected the inaccessible alternative, selected the accessible replacement, reconciled the hotel, and suppressed the unchanged alert. Action `pa-077-action-001`. | Pass. |
| PA-078 | `task:c9d3fb002942e2c4056cbe0797c863e5` | context 1, action 1, status 1 | Applied the pinned entry rules, kept the passport-validity gap unresolved, and recorded the checklist update. Action `pa-078-action-001`. | Needs repair. The fixture and run items show one call of each type, but the reviewer reported duplicates and the task was cancelled after that false finding. |
| PA-079 | `task:d53057c6f75fd58699743fb13ad5cf5a` | context 1, action 1, status 1 | Kept the restricted 150 USD credit separate from the pending 80 USD refund, prevented a double claim, and recorded one eligible recovery request. Action `pa-079-action-001`. | Pass. |
| PA-080 | first `task:c26a3627cbb557c8d42e400b3d2a2081`; rerun `task:e5dd6f294ae16500c92a2492937c2bd3` | combined ledger: context 2, action 1, status 1; clean rerun: 1/1/1 | The first task stopped because the original fixture lacked concrete dates, access, budget, and plan facts. After enrichment, the rerun chose Nov 13, the confirmed accessible hotel, and the 1,080 USD plan. Action `pa-080-action-001`. | Pass after fixture enrichment and rerun. The combined ledger retains the first read. |
| PA-081 | `task:f6e96d6d0ec09f5c46955f6b289e0b93` | context 1, action 1, status 1 | Selected venue B, the approved caterer, and the 820 USD plan after two cancellations. Action `pa-081-action-001`. | Needs repair. One failed, out-of-scope `code.run_lua` attempt occurred before the connector workflow. The connector ledger itself is exact once. |
| PA-082 | `task:41ddc04193eb278dd673e52960db06fc` | context 1, action 1, status 1 | Sequenced twelve address changes, preserved the Aug 20 to Aug 31 utility overlap, and kept identities separate. Action `pa-082-action-001`. | Pass. |
| PA-083 | `task:f0b2c7bd1469b2bf9ee36bc4be1976e7` | context 1, action 1, status 1 | Selected listing C at 1,790 USD including the pet fee and recorded one application. Action `pa-083-action-001`. | Pass. |
| PA-084 | `task:01fcff7b0da228cdda2ab56069c88ca4` | context 1, action 1, status 1 | Reconciled 800 USD unique spend, kept the deposit and refund open, and held the utility item for a completion receipt. Action `pa-084-action-001`. | Pass. |
| PA-085 | `task:789a53e205280652c93305d6c3b21179` | context 1, action 1, status 1 | Assigned the other parent to Child A and caregiver C to Child B for the Tuesday overlap. Action `pa-085-action-001`. | Needs repair. The fixture and run items show one call of each type, but the reviewer and result claimed duplicate counts and omitted explicit status verification of the update contents. |
| PA-086 | `task:3ec84e45143addf4338228cb904ef415` | context 1, action 1, status 1 | Separated notices by child, used May 21, deduplicated the deadline, and recorded Child A's permission and 35 USD fee. Action `pa-086-action-001`. | Needs repair. The connector ledger is exact once, but the result claimed two reads and two actions. The approval note also differed from the fixture phrase because the operator approved the conversational proposal. |
| PA-087 | `task:19a88dee53ee439cc56ff4e53fd6b29d` | context 1, action 1, status 1 | Selected Camp B at 320 USD after the sibling discount, kept Camp C waitlisted, and rejected unsuitable camps. Action `pa-087-action-001`. | Pass. |
| PA-088 | `task:330516f0c0d3d883f07d1d45f3938500` | context 1, action 1, status 1 | Preserved visible planning, allocated the six chores within capacity, and moved shopping when A dropped to six hours. Action `pa-088-action-001`. | Pass. |
| PA-089 | `task:71d072b413bc6919aa1c7d736cd3e71c` | context 1, action 1, status 1 | Updated the deadline to Oct 15, tracked all three programs, and kept unavailable transcripts and unnamed details unverified. Action `pa-089-action-001`. | Pass. |
| PA-090 | `task:ccbc1f0c35f664efa203bacdd5753802` | context 1, action 1, status 1 | Preserved A's schedule-only grant, recorded B's withdrawal and removal, and retained the expiry and receipt rules. Action `pa-090-action-001`. | Pass. |
| PA-091 | `task:74a755556695be29cdc40d65a3963fcd` | context 1, action 1, status 1 | After a clarification continuation, selected Role B, kept C's conflict and D's missing eligibility visible, and moved the shift to Wednesday 18:00. Action `pa-091-action-001`. | Pass after clarification continuation. |
| PA-092 | `task:d2a097e3f83b641c77211116a116cd0e` | context 1, action 1, status 1 | Selected the 60, 45, and 80 USD gifts, kept spend at 185 USD, respected the no-contact boundary, and left the 90 USD option unselected. Action `pa-092-action-001`. | Pass. |
| PA-093 | `task:3bdfda21a5d1216f06247d4ae9c0ba61` | context 1, action 1, status 1 | Retained 25 unique files and both differing same-name files, preserved retention data, and left duplicate removal as a separate approval. Action `pa-093-action-001`. | Pass after a result-document correction. The fixture does not expose named-connector counts, so the result reports that limitation. |
| PA-094 | `task:8df14ee68a6bbb4213077bfefc4865fc` | context 1, action 1, status 1 | Inventoried metadata, flagged the stale device and MFA gap, and revoked only the unrecognized session. Action `pa-094-action-001`. | Pass. |
| PA-095 | `task:76f7ea872a02ff01333161e834a96cb9` | context 1, action 1, status 1 | Preserved evidence and case IDs, contained mail first, revoked the two unauthorized sessions, and moved no money. Action `pa-095-action-001`. | Pass. |
| PA-096 | `task:73cad6f6a8c202d37ee7186959bb3c84` | context 1, action 1, status 1 | Revoked G1 and G2, tracked delayed deletion R and its reappearance, and kept unchanged settings quiet. Action `pa-096-action-001`. | Pass. |
| PA-097 | `task:d99a6d6cba60a4e01e307cb224a19d67` | context 1, action 1, status 1 | Recorded the protected-boundary migration plan for 20 files and settings, including F-17 repair and deferred disposal. Action `pa-097-action-001`. | Needs repair. The bounded fixture records a plan only. It does not transfer files, repair F-17, or verify the target. The result also reported counts that conflict with the live ledger. |
| PA-098 | `task:da26a988dc68a47f37a82322c99af050` | context 1, action 1, status 1 | Preserved source history, deduplicated evidence, corrected X to Y, and kept the later Y preference. Action `pa-098-action-001`. | Pass. The fixture response does not expose connector counts, but the persisted ledger and run items show one call of each type. |
| PA-099 | `task:60e81036dff72a687515cd74afe327d7` | context 1, action 1, status 1 | Saved the five-account legacy plan, executor B replacement, protected credential boundary, and death-plus-waiting-period release conditions. Action `pa-099-action-001`. | Needs repair. A failed prohibited `code.run_lua` attempt caused review exhaustion, so the task was cancelled without release or retry. |
| PA-100 | `task:68b8cab0cc384c09c2d8ab0f544b1c33` | context 1, action 1, status 1 | Recorded an accessible HTML and structured-text export plan with table, caption, attachment, keyboard, and screen-reader checks. Action `pa-100-action-001`. | Pass. |

For every row, the normal connector order was `get_case_context`, approval,
`record_case_action`, then `get_case_status`. The fixture returned one stored
action and `action_count: 1` for each case that reached the write. No real
external service was contacted.

## Findings for the next run

1. The runner and reviewer should count named connector calls from persisted
   tool records, not from broad provider-call totals or model-written prose.
2. A conversational approval note should remain compatible with the fixture's
   approval-note contract, or the test should accept the approved note as the
   source of truth.
3. The case fixture needs executable read-back behavior for migration and other
   cases whose acceptance criteria require a completed physical or provider
   operation. Recording a plan is not verification.
4. The executor still sometimes calls `code.run_lua` even when the task names a
   different reviewed connection. The failed calls caused no side effect, but
   they make the no-extra-tool condition fail.
5. Reviewer prompts need a clear boundary between connector calls, task-file
   calls, and failed tool calls. PA-078, PA-085, PA-086, and PA-097 exposed
   mismatches between the live ledger and the written review.
