# Live API setup repair

The live Gmail setup case has not passed acceptance.
No agent-generated connection has fetched mailbox data in this test.

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

The full proposal also omitted message bodies from its response transform.
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
