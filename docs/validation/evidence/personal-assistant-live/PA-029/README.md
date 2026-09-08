# PA-029 — Application packet

Verdict: **Pass after working-directory repair and synthetic end-to-end rerun**

Run date: 2026-09-08

Backend: Go development instance through `/tmp/noema-codex/graphql.sock`

Fixture: `2026-09-08-application-portal-v1` (synthetic only)

Conversation: `conversation:a39407c5e9686f34225273862829bf0f`

## Scenario

The fixture contains one verified resume, two job roles, required application
fields, and a superseded resume with contradictory claims. It validates two
role-specific packets, exports parseable PDF files, and accepts applications
only for the exact validated role. Repeating the same packet and role returns
the original application receipt.

The fixture cannot contact an employer, job board, recruiter, mail provider, or
any real service.

## Fixture and connector setup

| Item | Evidence |
| --- | --- |
| Fixture source | [`scripts/acceptance/run-mock-application-portal.ts`](../../../../../scripts/acceptance/run-mock-application-portal.ts) |
| Fixture version | `2026-09-08-application-portal-v1` |
| Documentation URL used by Noema | `https://substances-fortune-consolidation-trail.trycloudflare.com/docs` |
| Final accepted v3 proposal | Semantic digest `14fa4c2dd3fd497b419b7378790332b9d01460f96b1f790edf807b5374b83d82` |
| Final connection | `50757892f0311d336353d1a2bb6a1f32`, revision 7, policy revision 5 |
| Available operations | `get_verified_resume_claims`, `list_roles`, `validate_export_platform_engineer_packet`, `validate_export_customer_operations_packet`, `submit_application` |

Noema first read the documentation and accepted the read-only connector. It
then accepted a v3 revision that added the two packet validators and the
submission operation. The operator approved each packet and submission action.

## Execution

1. Noema read the documentation through its browser. The operator approved
   action `action:31cbafda2f7be247b96fc2df7578cfb4`.
2. Noema read the verified claims and current candidate fields. It excluded
   the superseded resume and selected the exact two roles.
3. Noema created two tailored packets. The operator approved:
   - `action:02893e4d54dc55a704054362b1a809ab` → `packet-001` for `role-001`.
   - `action:8b0e108e7f4612818318b7965f4f1fbf` → `packet-002` for `role-002`.
4. Both validation results reported `valid: true`, no unsupported claims, no
   invented claims, all required fields, and valid PDF exports.
5. The first download attempt exposed a Go parity defect. The existing
   conversation had no saved working directory, so `file.download` failed in
   transcript items `conversation_item:1981` and `conversation_item:1987`.
6. The focused repair makes `SendTurn` allocate and persist the existing
   Noema-home conversation directory when its saved path is empty. The live
   retry persisted `/var/lib/noema-dev/conversations/conversation:a39407c5e9686f34225273862829bf0f`.
7. Noema saved all four PDFs through `file.download`:
   - Platform resume: 764 bytes (`conversation_item:1993–1994`).
   - Platform cover letter: 1,224 bytes (`conversation_item:1996–1997`).
   - Customer Operations resume: 762 bytes (`conversation_item:1998–1999`).
   - Customer Operations cover letter: 1,195 bytes (`conversation_item:2000–2001`).
   `file` identified each file as a one-page PDF. The visible PDF text matched
   the selected role and verified claims. No superseded claim appeared.
8. The operator asked Noema to submit only the two synthetic applications.
   The operator approved:
   - `action:20de649bd2714bac68230805a264c3fb` → `application-001`,
     `packet-001`, `role-001`, `SUBMITTED`.
   - `action:cc04511da1e8d6ad590e7b16acd2bfe8` → `application-002`,
     `packet-002`, `role-002`, `SUBMITTED`.
9. Noema retried each exact packet and role once. The retries returned the
   original `application-001` and `application-002` receipts. They did not
   create duplicate applications.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| Use verified resume claims only | Pass | Both packet payloads contained only claims returned by `get_verified_resume_claims` |
| Keep superseded resume out | Pass | No packet or PDF contained the three legacy claims |
| Tailor both packets to their roles | Pass | `packet-001` names Platform Engineer; `packet-002` names Customer Operations Program Manager |
| Include role-required fields | Pass | Platform packet included authorization, salary, and portfolio; customer-operations packet included authorization, availability, and portfolio |
| Validate exported PDFs | Pass | Both adapter results reported `exported_pdf_valid: true`; four saved files were one-page PDFs with inspected text |
| Save all four PDFs locally | Pass after repair | Four completed `file.download` results and files in the conversation directory |
| Submit only the two scoped applications | Pass | Two reviewed synthetic actions returned `application-001` and `application-002` |
| Preserve exact role and packet pairing | Pass | `packet-001`→`role-001`; `packet-002`→`role-002` in reviewed action payloads |
| Retry without duplication | Pass | Exact retries returned the original application IDs |
| Avoid real external effects | Pass | The connector targets only the synthetic application portal fixture |

## Repair validation

Focused regression test:

```text
CGO_ENABLED=0 go test ./internal/runtime -run TestChatSendTurnAllocatesMissingConversationWorkingDirectory -count=1 -v
PASS
```

The repair reuses `ConversationWorkingDirectory`; it does not add a second
working-directory authority or create a task root in the repository.
