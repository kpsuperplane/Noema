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
