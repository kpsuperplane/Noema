# Proactive event sources

Status: approved direction, not implemented.

The Go server has no external event ingress or proactive event rules.
This plan records the approved product scope. It does not describe current behavior.

## Outcome

A human connects an event source and sets one triage instruction.
Noema receives verified events and can send an ordinary message in the main Chat.

The first production proofs are Gmail, Slack, and Dropbox.
Use the existing HTTP adapter, Chat, and notification paths.
Do not add another integration framework.

## Required boundaries

- Event triage can only ignore an event or save a Chat message. It cannot run tools or external actions.
- Save the Chat message before sending client notifications.
- Verify each callback before accepting its payload. Reject invalid signatures, identities, accounts, and subscriptions.
- Keep credentials in protected stores. Keep authorized private payloads intact.
- Treat external text as untrusted data. It cannot change instructions or permissions.
- Require the current connection, rule, and authorization before processing.
- Persist accepted work before acknowledging delivery. Prevent repeated messages after retries or restarts.
- Support one callback endpoint with several accounts without mixing their state.

## Provider requirements

Gmail uses authenticated Pub/Sub callbacks and checkpoint-based history synchronization.
Only a provider API response can advance the checkpoint.
Commit a checkpoint after all pages complete.
Detect missed callbacks within six hours while the checkpoint remains valid.
Renew watches before expiry and preserve progress across replacement watches.

If a checkpoint expires, pause processing and show the possible gap.
Require a human-started recovery with explicit bounds.
The approved recovery bounds are seven days, 20 pages, or 500 messages, whichever comes first.

Slack proves signed direct events.
Dropbox proves signed callbacks, account routing, and independent synchronization checkpoints.
Neither callback order nor provider checkpoints establish a global event order.

## Settings and acceptance

Rules specify source, account, event types, instruction, and enabled state.
Settings must show source health, renewal, synchronization, recovery, gaps, and rule history.
Use existing notification settings for alerts after a message is saved.

Acceptance must cover forged callbacks, dropped callbacks, restart during pagination, repeated delivery, subscription replacement, and revoked access.
Live acceptance needs isolated accounts and provider access.
Synthetic fixtures alone do not prove provider compatibility.

## Deferred scope

Other providers need a requested, reviewed definition and their own proof.
Salesforce streaming, automatic resource hydration, and iOS rule management are outside the initial scope.
Choose concrete storage and runtime changes when implementation starts.
