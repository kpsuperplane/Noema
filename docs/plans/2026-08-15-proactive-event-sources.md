# Proactive Event Sources Plan

- **Status:** Active approved plan; implementation not started
- **Mode:** Implement in ordered units
- **Date:** 2026-08-15
- **Approval date:** 2026-08-16
- **Initial scope:** Native HTTP adapters, primary agent, Web settings, and existing client notifications
- **Production proofs:** Gmail, Slack, and Dropbox
- **Compatibility target:** Eleven common HTTP services
- **Deferred transport:** Salesforce Pub/Sub

## 1. Outcome

Noema will receive external events and send configurable proactive messages.

A human can connect a reviewed event source and give one triage instruction.
The primary agent can ignore the event or send one ordinary assistant message.

The release must provide these outcomes:

1. Receive Gmail changes without mailbox listing or message polling.
2. Reconcile Gmail changes from an API-issued checkpoint.
3. Detect a missed Gmail callback within six hours while its checkpoint remains valid.
4. Receive signed direct events through the same HTTP ingress.
5. Support one callback endpoint for several connected accounts.
6. Configure rules by source, account, event type, instruction, and enabled state.
7. Save the agent message before any client notification.
8. Explain source health, gaps, and rule history.
9. Let reviewed custom adapter definitions declare HTTP event behavior.

This work makes proactive event handling a production path.
It does not add another general integration substrate.

## 2. Boundaries

- No event can run tools or external actions.
- Do not infer behavior from OpenAPI `webhooks` objects.
- Do not add a general event workflow or transport framework.
- Do not give Luau network, file, storage, or secret access.
- Do not compare provider checkpoints or require event ordering.
- Do not add universal automatic hydration.
- Do not implement Graph rich notifications or Salesforce streaming.
- Do not add iOS management in this release.
- Do not add special labels or styles to proactive chat messages.
- Do not separate chat messages and client notifications into rule outcomes.
- Do not retire the dormant adapter scheduler without a separate decision.

## 3. Core Contract

### 3.1 HTTP ingress

The first release supports public HTTPS callbacks.
Gmail uses Pub/Sub before its callback reaches Noema.

Use one concrete HTTP ingress.
A future transport can submit events through the same stored delivery boundary.

### 3.2 Accepted results

| Result | Meaning |
| --- | --- |
| `event` | The callback contains a useful event payload. |
| `wake` | The callback reports possible changes. A reviewed sync operation retrieves them. |

An `event` has a stable key, type, time, bounded payload, and optional resource references.
A `wake` identifies event bindings and can include a diagnostic hint.
The hint never becomes stored cursor authority.

Only an API response can advance an opaque sync cursor.
Store page progress separately.
Commit the sync cursor after the final page.

Increment one local generation for each accepted wake.
Run another sync pass when the generation changes during processing.

### 3.3 Governed output

Run deterministic eligibility checks before inference.
Use one structured triage request without tools.

The model can return only `ignore` or `send_message`.
`send_message` saves one ordinary final assistant message in the primary chat.
The existing final-message notification projection then runs unchanged.

## 4. Compatibility Contract

| Service | Event form | Required behavior | Position |
| --- | --- | --- | --- |
| [Gmail](https://developers.google.com/workspace/gmail/api/guides/push) | Pub/Sub wake | OIDC identity, renewal, early callback, reconciliation, and history sync | First proof |
| [Microsoft Graph](https://learn.microsoft.com/en-us/graph/change-notifications-delivery-webhooks) | Webhook wake | Query challenge, `clientState`, lifecycle, and delta links | HTTP compatible |
| [Google Calendar](https://developers.google.com/workspace/calendar/api/guides/push) | Empty wake | Channel token, renewal overlap, and sync tokens | HTTP compatible |
| [Slack](https://docs.slack.dev/apis/events-api/) | Direct event | Application endpoint, signed challenge, and HMAC | Second proof |
| [GitHub](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks) | Direct event | HMAC, delivery key, and no automatic retry | HTTP compatible |
| [Stripe](https://docs.stripe.com/webhooks) | Direct event | Timestamped HMAC, duplicates, and no ordering | Snapshot compatible |
| [Notion](https://developers.notion.com/reference/webhooks-events-delivery) | Metadata event | Protected token capture and later HMAC | HTTP compatible |
| [Dropbox](https://www.dropbox.com/developers/reference/webhooks) | Application wake | GET challenge, HMAC, fan-out, and cursor | Third proof |
| [HubSpot](https://developers.hubspot.com/docs/api-reference/latest/webhooks/guide) | Event batch | Application endpoint, canonical HMAC, and batch split | HTTP compatible |
| [Jira](https://developer.atlassian.com/cloud/jira/software/webhooks/) | Direct event | HMAC, renewal, retry key, and failure status | HTTP compatible |
| [Linear](https://linear.app/developers/webhooks) | Direct event | HMAC, replay time, delivery key, and HTTP 200 | HTTP compatible |
| [Salesforce](https://developer.salesforce.com/docs/platform/pub-sub-api/references/methods/subscribe-rpc.html) | gRPC stream | OAuth metadata, Avro, flow control, and replay | Deferred driver |

Compatibility means that a reviewed definition can express the service.
This plan does not bundle every compatible definition.

## 5. Stored Authorities

Use four SQLite authorities.

| Authority | Owned state |
| --- | --- |
| Event endpoint | Public route, scope, setup status, verifier references, and safe health data |
| Event binding | Connection, routing key, subscription, expiry, cursors, deadlines, generation, and worker claim |
| Proactive rule | Human, primary agent, governed scope, source selection, event types, instruction, and enabled state |
| Proactive delivery | Event identity, payload, rule revision, processing state, decision, and saved conversation item |

One application endpoint can serve several bindings.
Provider display labels never route callbacks.

The first schema must reject non-primary agents.
A rule cannot raise any applicable proactivity limit.
Existing policy must allow the agent to create a proactive message.

Use one delivery uniqueness key across retries and replacement subscriptions.

## 6. Adapter and Runtime Contract

### 6.1 Reviewed adapter definition

Advance the strict adapter manifest version with the first complete vertical path.
Do not publish an event field before its runtime behavior exists.

One reviewed event source declares:

- Stable source identity and endpoint scope.
- Manual or managed setup.
- Optional runtime-only subscribe, renew, and stop operations.
- Exact lifecycle and sync scope requirements.
- Fixed success status and optional handshake transform.
- Required receive transform.
- Direct-event or wake-and-sync behavior.
- Sync operation, cursor arguments, and recovery behavior.

Keep lifecycle and sync operations outside the model catalog.
Keep OpenAPI webhook import rejected.

Reject missing operations, unsupported scopes, incomplete verification, and wrong definitions.
Reject wake sources without sync and direct sources without stable event keys.

### 6.2 Bounded Luau profile

Add one bounded `InboundEvent` Luau profile.
Provide immutable request data and receipt time.

Expose only these secret helpers:

- `auth.matches(candidate)`
- `auth.verify_hmac_sha256(message, candidate, encoding)`

The profile can also use existing bounded JSON and text helpers.
Secret helpers return only Boolean values.
The host records successful verification.

Keep Google Pub/Sub OIDC verification in the host.
Do not expose bearer tokens, claims, or public-key handling to Luau.

Luau can return `challenge`, `capture_setup_secret`, `accept`, or `reject`.
Unsigned challenges can run only during one current pending setup attempt.

Never expose secret bytes, HMAC output, general signing, network, files, randomness, or persistent state.

### 6.3 Callback sequence

1. Resolve the endpoint from an unguessable route identity.
2. Apply method and body limits.
3. Run the declared native verifier.
4. Run the exact reviewed Luau transform.
5. Require the declared verification effect.
6. Resolve application-scoped account keys exactly.
7. Insert deliveries or increment wake generations.
8. Commit the transaction.
9. Return the fixed success response.

Do not acknowledge an ordinary delivery before durable insertion.
Keep the path within the shortest supported provider deadline.

### 6.4 Gmail callback authentication

Require an authenticated Google Pub/Sub push subscription.
Reject an unsigned Gmail callback before event-data decoding.
Follow [Google's push-authentication contract](https://cloud.google.com/pubsub/docs/authenticate-push-subscriptions).

The host must verify these JWT properties:

- A signature from Google's current public keys.
- An issuer of `accounts.google.com` or `https://accounts.google.com`.
- The exact configured audience.
- Current `exp` and plausible `iat` values.
- `email_verified` set to true.
- The exact configured push service-account email.

Require the envelope subscription resource to match the binding.
Treat the envelope and decoded Gmail data as private, untrusted input.

Never pass the JWT to Luau.
Never store or log the authorization header, JWT, or decoded claims.
Cache Google public keys only for their advertised lifetime.
Fail closed when key refresh or claim verification fails.

### 6.5 Wake sync

1. Claim one binding revision with a bounded lease.
2. Read its committed cursor and page progress.
3. Invoke one exact runtime-only sync operation.
4. Run the existing source input check.
5. Normalize the response through reviewed Luau.
6. Commit deliveries and next-page progress atomically.
7. Commit the sync cursor only after the final page.
8. Release the claim or process the next wake generation.

One claim processes one provider page.
Provider continuation URLs must pass same-origin and path checks.

### 6.6 Gmail reconciliation and recovery

Store the last successful reconciliation and next deadline with each Gmail binding.
Set the deadline to six hours after each successful reconciliation.

Start checkpoint sync after a verified wake, an overdue deadline, a late startup, or an activated replacement watch.
Start each run from the committed Gmail `historyId`.
Use `history.list` and the same page transaction.
This call is not a mailbox listing or message poll.

Use bounded retries with jitter for transient provider failures.
Keep the old checkpoint until the final page commits.
Show overdue reconciliation as degraded health.

If Gmail rejects an expired `historyId`, enter `RecoveryNeeded`.
Do not advance the checkpoint or triage later events.
Show the possible gap and require a human-started resync.

One resync can inspect seven days, 20 pages, or 500 messages.
Stop at the first limit and keep the source paused.
After a complete resync, read a new provider checkpoint and commit it last.
Preserve the gap window and selected recovery bound in the audit record.

### 6.7 Lifecycle

Create the endpoint before the provider subscription.
This order closes immediate-callback races.

Renew from provider expiry data.
Renew each Gmail watch daily and before its provider expiration.

Create replacement subscriptions before retiring old subscriptions.
Keep the committed cursor and deduplicate across the overlap.

Apply only the declared recovery behavior after cursor expiry.
Show the human when recovery can lose events.

## 7. Governance and Product Surface

Store signing secrets and verification tokens only in protected adapter generations.
Treat private event bodies as private information.
Keep authorized payloads intact in governed delivery records.

Provider identifiers, URLs, and cursors are not secrets by name.
Treat all external payload text as untrusted content.

Before inference, require an active grant, binding, current rule, exact scope, and sufficient proactivity level.
Existing notification settings decide whether the saved message produces an alert.
Do not log raw bodies, signatures, setup secrets, or model payloads.

Use the existing Adapter Settings hierarchy.
Show setup, health, callbacks, sync, expiry, recovery, gaps, and rules.

Create or edit one rule in a dialog.
Include source, account, event types, instruction, and enabled state.

Reuse existing Web Push and APNs settings.
Render the saved item as an ordinary assistant message without provenance markers.

## 8. Execution Units

Complete each unit with one commit.
Use one read-only adversarial review and one correction pass for each server unit.

| Unit | Outcome | Production budget | Required proof |
| --- | --- | ---: | --- |
| Gmail wake and sync | One governed proactive message | 1,200–1,800 Rust lines | OIDC, durable receipt, restart, cursor, missed wake, recovery, renewal, revocation, projection |
| Web configuration | Gmail setup and one rule | 250–500 TypeScript lines | Empty, pending, active, failed, and revoked states |
| Slack direct event | Signed event through one application endpoint | 250–500 Rust lines | HMAC, replay time, routing, challenge, and deduplication |
| Dropbox shared wake | Independent progress for each account | 250–450 Rust lines | Challenge, batches, duplicates, cursor expiry, and HTTP 200 |
| Compatibility closure | Proven protocol variations only | Net-negative where possible | Compiler and sandbox cases at authoritative boundaries |

Gmail can add at most ten Rust tests.
Slack and Dropbox can each add four to six focused Rust tests.
Do not add UI tests unless requested.

Use `noema-product-ui` before Web design work.
Request browser-inspection permission before visual validation.

Stop a unit if it needs another substrate, a workflow engine, or another connection authority.
Stop when a new public abstraction or product decision becomes necessary.

## 9. Risk and Proof Matrix

| Risk | Authority and proof |
| --- | --- |
| Forged callback creates output | Verifier; invalid OIDC, token, and HMAC cases |
| Gmail callback is dropped | Reconciliation deadline; missed-wake case |
| Gmail checkpoint expires | Recovery state; bounded resync and visible-gap cases |
| Early acknowledgement loses data | Callback transaction; restart after receipt |
| Page crash skips changes | Binding checkpoint; mid-page restart |
| Renewal loses position | Binding lifecycle; overlap case |
| Retry repeats a message | Delivery key; provider retry case |
| Shared endpoint misroutes data | Provider account key; mixed batch case |
| External text expands authority | Triage runtime; no-tool policy case |
| Revoked state still runs | Binding and rule revisions; race case |
| Secret reaches an ordinary sink | Protected generations; exclusion and preservation cases |
| Private payload is removed | Delivery record; private-data preservation case |
| Cursor is changed or compared | Sync boundary; opaque cursor fixture |
| Notification text differs | Conversation item; projection identity case |

Test each risk at its first authoritative boundary.
Do not repeat the same behavior through every layer.

## 10. Validation and Release Gates

Use the unit budgets with the Rust size report at each milestone.
Stop when growth exceeds its estimate by 50 percent or 500 lines.

Run the standard Rust gates before each server commit.
Use `cargo validate` for focused unit tests.
Do not run smoke or fixture tests without approval.

For Web work, run `bun run lint` and `bun run build` from `apps/web`.
Inspect status, staged names, and staged statistics before every commit.
Preserve unrelated worktree changes.

Release acceptance requires these proofs:

1. Authenticated Gmail changes produce one proactive message.
2. Wrong OIDC claims, signatures, accounts, or subscriptions fail closed.
3. A dropped Gmail callback is reconciled within six hours.
4. Restarted pagination loses no event and repeats no message.
5. An expired checkpoint pauses triage and shows the gap.
6. Slack proves signed direct events.
7. Dropbox proves setup and account-specific cursors.
8. Disabled rules and revoked connections stop later processing.
9. Saved chat text exactly matches notification text.
10. Settings explains renewal, sync, recovery, and gaps.
11. Secrets stay protected, and authorized private event data stays intact.
12. Required Rust and Web gates pass, except documented baseline failures.

Live acceptance needs isolated test accounts and human provider access.
Do not claim live compatibility from fixtures alone.

## 11. Deferred Work

Add a provider definition only when a human requests it.
Do not add provider branches to the shared HTTP runtime.

Add exact hydration only after one direct source proves that metadata is insufficient.
Implement Graph rich notifications only after it becomes a production proof.

Implement Salesforce only after an explicit product decision.
Use one native `salesforce_pubsub` driver that feeds proactive deliveries.
