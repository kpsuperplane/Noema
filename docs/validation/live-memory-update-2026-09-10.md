# Live Memory update

Base revision: `e7dac8e0`.

The user asked for a successful Memory update on noema-dev.
The live root was empty. Its checkpoint was zero, with 15 pending source items.
An authorized `updateMemory` mutation reproduced this error after correction:

> Memory body repeats citation marker [^1]

The prompt allows each citation group at least once.
The Memory store also permits repeated references to a group.
The runtime parser had an extra rule that rejected them.
Removing that rule makes the parser follow the existing contract.
Checks for missing groups, invalid indexes, and unauthorized sources remain.

## Live evidence

The development watcher rebuilt and started the changed server.
A second authorized `updateMemory` mutation was accepted.
The existing `memoryEvents` subscription reported completion at
`2026-09-10T22:21:11.795019828Z`.

- The root changed from an empty article to a populated article.
- The checkpoint advanced from 0 to 17.
- Pending source items changed from 15 to 0.
- Update state became `idle`, with `active: false` and no error.
- A fresh GraphQL read returned the same root hash and checkpoint.
- Both citation groups resolved to human-message sources.
- The saved root contains two references to citation 1 and one generated definition.
- The saved `.state.md` contains checkpoint 17 and the same completion timestamp.

The update used the configured model and normal publication path.
No source message, Memory article, or checkpoint was manually rewritten.
Private article text and source identifiers remain outside this report.

## Validation

The existing publication test now uses two claims supported by one human message.
It verifies both repeated references, the published source group, and the checkpoint.
The old rejection case for repeated references was removed.
Other invalid-source and invalid-index cases remain.

- `CGO_ENABLED=0 go test -p 1 ./internal/runtime -run 'TestMemory|TestRustRuntime_parser_'` passed.
- `CGO_ENABLED=0 go test -p 1 ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 go vet -p 1 ./cmd/... ./internal/...` passed.

## Size and review

Production Go shrinks by three lines. Tests shrink by one line.
Generated GraphQL is unchanged. Inclusive tracked Go shrinks by four lines.
The existing inclusive total remains above its historical 80% migration gate.

Review checked that the fix permits repeated references without accepting new sources.
The change adds no schema fields, fallback, or retry attempt.
