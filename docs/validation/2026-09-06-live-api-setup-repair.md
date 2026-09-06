# Live API setup repair

The live Gmail setup case has not passed acceptance.
An agent-generated connection is installed. Its profile read succeeded through Noema.

## Findings

Rust setup at `8a135658` supplies complete examples and specific proposal errors.
Go supplied four short instructions and discarded operation validation details.
Go provider conversion also closes object schemas that omit an explicit open-field declaration.

Commit `653ae03c` restores proposal examples and explicitly permits proposal object fields.
The existing parser and compiler still validate submitted definitions.
The large experimental nested schema was removed before this commit.

The next correction includes the operation index and response size limit in compiler errors.
It changes no response limits or authentication rules.

## Live evidence

Conversation: `conversation:9a8adece587d96886957b3a308c694b1`.

| Turn | Observed result |
| --- | --- |
| `turn:a870baad755ae47860d17e66f9fb3497` | Agent read the restored template, but submitted only a source URL. |
| `turn:88bc0c6182c0cfa07f9b4caea9b49054` | Another source-only attempt failed. |
| `turn:cbe82575ace7353fb6164bd5afa7a61f` | Agent submitted credential setup and five complete operations. Operation validation rejected oversized responses. |
| `turn:1fcf0a553270574fc8dab00d2b103938` | Agent used the specific size error and created pending proposal `e82c88fa717d58fc5215178946f7a131bbc06e5ff36763e67a41de8b4a134664`. |

The full proposal also omitted message bodies from its response transform.
The pending proposal also drops thread messages and silently truncates attachment data at 4,096 bytes.
The operator requested a correction before approval.
The agent created revision `131c0ae252fcbdbed39202ab79ff5176d70051dcd911b29d9246170dc61b79d6`.
It returns bounded email bodies, up to three thread messages, and explicit completeness fields.
The operator approved it as reviewed digest `760dea9cf57226af959352727677777ead26a0ac63086a208d45442e896c0e2d`.
Protected credential setup succeeded for synthetic account A.
The operator then asked the agent to identify its mailbox and retrieve trip messages.
A proposal approval alone will not establish the Gmail case pass.
The fixture currently uses bearer credential setup; the plan's OAuth requirements remain untested.

## Checks

- `go test ./internal/adapter` passes with the template and compiler error changes.
- Broad Go tests and vet passed before the explicit open-field schema change.
- After that schema change, broad tests failed in `TestMemoryGraphQLWritesPreferenceAndPublishesLiveUpdate`.
  That test passed in the earlier run. Its cause remains unconfirmed. Vet passed.
- The template patch adds 76 production lines and 33 test lines. Generated GraphQL is unchanged.
- The compiler correction adds 3 production lines and 11 test lines after deletions.
- Broad Go tests and vet pass with both corrections. The Memory failure remains an intermittent observation.
- The measured Go total after that patch is 187,548 lines, including 28,075 test lines.
  Generated GraphQL contributes 79,556 lines. Production contributes 79,917 lines.
  Production is 45.93 percent of the Rust baseline. The inclusive ratio is 78.20 percent.

Continue with the live proposal response, review its exact operations, and verify data through the accepted connector.

## First connected reads

The operator enabled automatic synthetic data reads with unsafe actions set to `always_ask`.
The browser attempt was declined. Its decision request timed out, but the transcript confirms the saved decline.
The connector returned `alex@example.test`, seven messages, and three threads at transcript cursor `conversation_item:1483`.
The fixture independently recorded authenticated account-A HTTP 200 responses for profile and message-list requests.

List calls failed because Go rejected a missing next-page token.
Rust `8a135658:crates/noema-capabilities/adapters/src/response.rs` treats an absent token as completion.
The Go correction restores that behavior for absent object members and tests ordinary result preservation and invalid-token rejection.
This slice changes one production line and adds seven test lines.

Before that correction, the agent proposed a v3 revision based on an incorrect diagnosis about preserving the raw token.
Do not approve that revision without inspecting it. The backend correction needs a live list retry.
Full message, attachment, multi-page, OAuth, and account-isolation acceptance remain open.

The next retry found that the pending v3 revision hid the approved v2 tools.
Go marked an approved definition superseded by an unapproved replacement.
The correction preserves approved tools until another approved definition replaces them.
The existing revision test now verifies this intermediate state.
This slice changes one production line and adds four test lines.

## Live retry after the fixes

Turn `turn:4a0f1be24bd8662562d53df073dee1ca` completed successfully through the approved v2 connector.
The fixture recorded account-A HTTP 200 responses for profile, message list, trip thread, and message `a-msg-007`.
The assistant identified the mailbox, summarized three trip messages, and reported the fourth body unavailable.
Its final response is stored at `conversation_item:1550`.

The fourth fixture message places text data directly on a multipart payload.
The proposed transform searches text MIME parts, so this body remains unavailable.
Inspect fixture MIME fidelity before treating this as a product defect.
The summary also inferred calendar dates from weekday-only message text without labeling that inference.
These observations prevent a full content-quality pass.

Broad Go tests and vet pass with all four repair commits' code changes.
The final pending-review correction is included in this document's commit.
No production build or test changes followed those successful checks.

## Remaining setup checks

Turn `turn:fbc6a5766f01e3d64c0a2f722398b50e` retrieved all seven IDs in batches of three, three, and one.
The fixture recorded `page-2` and `page-3` requests under account A.
The assistant preserved weekday-only dates without adding calendar dates.
It correctly identified missing attachment IDs in the approved message operation.

The trace exposed duplicate `maxResults` arguments from caller input and configured pagination.
The request encoder now uses the configured page size only when the caller supplies none.
Its regression check verifies both explicit and default page sizes. Broad Go tests and vet pass.

Fixture v2 corrects multipart text placement and caps each page at three records.
Its two protocol checks pass; the public process still reports v1.
See [the pinned fixture contract](gmail-fixture-contract.md).

The attachment revision turn `turn:b30caf3e06f08b3b4f88244f4d1ea13f` was cancelled during the development rebuild.
Stored turn state confirms cancellation. Its proposal did not pass validation.
The revision template returned stored operations instead of proposal-shaped operations, which needs correction.

The revision template now removes stored behavior and retry fields, supplies the four proposal behavior flags,
and marks the response as custom. A regression check resubmits the returned operation and preserves its response contract.
The patch adds 15 production lines and 21 test lines after deletions.
The broad test run passed all other packages but exposed a nil-transform assumption in the new test.
After that test-only correction, `CGO_ENABLED=0 go test ./internal/adapter` and broad vet pass.
The other broad package results are reused because production code did not change.
Current Go totals: 79,936 production, 28,194 tests, 79,556 generated GraphQL, and 187,686 inclusive lines.
Both migration ratios remain below 80 percent.

## Attachment and empty search

The agent generated attachment revision `164cd0d3761119c419ec24853b36ad2b209f7e10808dacf238dbba0e3986cd7e`.
The operator inspected and approved it as `c66bc1c200165d3faee667ed63e1a14a5d72156a24f424a8a5dbbbe9cf55a2b4`.
Connection `7e1a192dfe8cb6c687de09a01a2e1e94` retained credential revision 1 and its existing policy.

Turn `turn:49bffff30f3ba1200efa3ba44592de0b` passed attachment and empty-search checks.
Cursor `conversation_item:1607` contains the attachment ID and filename from the message response.
Cursor `conversation_item:1609` contains an empty message list for the submarine query.
Cursor `conversation_item:1612` contains the complete 31-byte attachment body in base64url form.
Its decoded bytes are `passport\ncharger\nwalking shoes\n`.
The final answer at `conversation_item:1614` correctly reports all three items and no search matches.
The packing-list email body remains unavailable while the public fixture runs its malformed v1 MIME record.

A background preparation Task has been requested to test reuse of the saved connection outside Chat.

Task `task:9420acd0191f79b948b39921ec5bb7ea` fetched the trip messages and attachment through the saved connection.
Its saved `work/output/preparation-note.md` preserves the weekday wording, all three packing items, and the missing-body caveat.
The first Executor failed because the progress check forced terminal submission before `RESULT.md` existed.
The runtime now permits required result saving before an early progress-check finalization.
Hard execution limits retain their existing behavior.
One regression test reaches result saving and review after that early decision.
The patch adds eight production lines and 56 test lines.
The operator requested a retry of the same Task with its saved work.

Turn `turn:eb3077a7fc7252e4c0d0d44ea676bde5` requested account B's `b-msg-001` through account A.
The fixture recorded account A and HTTP 404. Noema reported the missing record without switching accounts.
This verifies that read boundary, not full dual-account setup or cross-account cursor isolation.

The retried Task reached Done.
Executor `run:9be3217a8f06fac6aaf5f425e05a749f` completed with eight tool calls.
Reviewer `run:3f7f910d682fbb1452ed9723e3def498` completed with seven tool calls.
The operator inspected the saved preparation note, RESULT.md, and REVIEW.md.
They preserve the three packing items, original weekday wording, and unavailable-message caveat.
The retry reused the saved note without repeating mailbox reads.
The reviewer approved the result. Broad Go tests and vet pass with the finalization correction.

Setup case 1 remains incomplete.
Required next checks: deploy fixture v2, recover the packing email body, implement and test mock OAuth,
test expiry and rate-limit recovery, and verify both account setups and cursor isolation.
The fixture also has three threads while the plan calls for two; reconcile the seed before final acceptance.
The public fixture runs outside this command's process namespace. Its system service bus is unavailable here.
The public health endpoint still reports fixture v1. No live v2 pass is claimed.

### Service-independent OAuth setup

OAuth application import now accepts optional `profileDocumentJson` beside the protected client document.
The operator must review the public profile and supply its matching `profileDigest`.
The profile selects authorization and token endpoints, credential normalization, parameters, and grant audience.
Authorization, code exchange, and refresh use the saved profile. Google remains a built-in profile.
The supported flow remains PKCE with `client_secret_post`; unsupported profile settings are rejected.
This does not add an agent tool for profile registration or change credential storage.

Focused adapter and GraphQL tests pass. Two new tests cover selected-provider authorization,
profile persistence, digest mismatch, credential exclusion, ordinary client ID preservation, and unsupported settings.
These tests do not establish live token exchange or refresh against the mock service.
The private dev socket still reports authenticated access.

The patch adds 158 authored production lines and removes 49. Tests add 77 lines and remove six.
Generated Go adds 15 lines and removes two. The net tracked Go change is 193 lines.
Full server checks pass: `CGO_ENABLED=0 go test ./cmd/... ./internal/...`
and `CGO_ENABLED=0 go vet ./cmd/... ./internal/...`.
These checks cover the final authored and generated Go changes in this unit.
The inclusive tracked Go count is 187,943, below the 191,860-line limit.
Mock OAuth service implementation and deployment remain the next live setup steps.

### Callback field and deployment check

The live hosted OAuth setup returned `redirectUri: null` while instructing the operator to register that address.
The existing GraphQL field now receives the configured callback from the adapter service.
A live query confirmed `https://noema.kevinpei.com/adapter/oauth/callback` after the rebuild.
This matches the v3 fixture's registered redirect. No schema or sign-in behavior changed.
Focused adapter and GraphQL tests pass. Full Go tests and vet pass with this final patch.
The patch adds eight production lines and removes four, with no generated or test changes.

The v3 fixture binary is built at `/tmp/noema-provider-fixtures-v3`.
The public endpoint still reports v1. Both fixture ports, 3742 and 3743, have existing listeners.
Their processes are outside this command session's process view. The host service bus returns `No data available`.
Deployment therefore requires a host-terminal restart of the existing fixture on port 3742.
Supply `NOEMA_FIXTURE_CLIENT_SECRET` from a protected environment; never print its value or put it in Chat.
Keep the Noema backend and its existing home running. Do not replace either service with a test home.
