# Reusable OAuth Applications and Grants Plan

- **Status:** Implemented in source; native generation and live acceptance pending
- **Mode:** Implement
- **Date:** 2026-08-12
- **Scope:** Native HTTP API adapters, Web, and iOS
- **Compatibility:** Clean cutover; existing OAuth accounts require setup again

## 1. Outcome

Noema will store an uploaded OAuth client document once as an OAuth application.
The application can authorize multiple external accounts and compatible APIs.

The finished system must support these outcomes:

1. Add write access without another client document upload.
2. Add another account without another client document upload.
3. Connect another compatible API without another client document upload.
4. Keep covered read operations available while added access awaits consent.
5. Keep Gmail and Calendar as separate fixed-origin API definitions.
6. Let their connections share one compatible Google account grant.

This work replaces the current OAuth ownership model. It does not add a parallel connector system.

Implementation now covers the authority stores, operation scopes, runtime,
GraphQL, Web, iOS source flows, and clean cutover. Native Apollo generation,
native visual validation, and live Google acceptance still need macOS and human
provider access.

Settings receives ordered exact connection actions. These actions cover every
compatible unattached grant and active application. Web and iOS continue new
attachments into the existing connection-policy editor.

## 2. Current Failure

The current definition owns the API origin, OAuth endpoints, client setup, and one global scope set.

The current connection credential combines OAuth application details, account tokens, granted scopes, lifecycle, operations, and policy.
Credential setup therefore creates a new connection for every upload.

A definition scope change also blocks automatic adoption. Active connections require every definition scope.

The uploaded document represents an OAuth application. Noema currently treats it as one API connection credential.

## 3. Terms and Hierarchy

| Term | Meaning |
| --- | --- |
| OAuth profile | Reviewed public protocol rules for one OAuth service |
| OAuth application | One provider client registration created from an uploaded document |
| External account | One provider account identity and display label |
| Authorization grant | One application and account authorization, with scopes and tokens |
| API definition | One reviewed fixed-origin API operation set |
| API connection | One definition attached to one grant and one human policy |
| Add access | Request more scopes for an existing grant |
| Add account | Create another grant with an existing application |

Do not call the uploaded client document an account credential. It configures the OAuth application.

The object hierarchy is:

```text
OAuth profile
└── OAuth application
    ├── Grant for account A
    │   ├── Gmail connection
    │   └── Calendar connection
    └── Grant for account B
        └── Gmail connection
```

API definitions remain separate:

```text
Gmail definition    -> Gmail connections
Calendar definition -> Calendar connections
```

## 4. Authority Model

### 4.1 OAuth profile

The profile owns reviewed, non-secret protocol behavior:

- Stable identity, revision, authorization endpoint, and token endpoint
- Token endpoint client authentication
- Callback-specific application setup and document normalization
- Fixed authorization parameters
- Incremental authorization and account-selection behavior
- Token audience and grant-sharing rules
- Optional reviewed account identity resolution

Profiles are immutable and content-addressed. A profile change requires review.

Runtime reuse needs an explicit compatibility contract. It must not match domains, endpoint strings, or provider names.

### 4.2 OAuth application

The application owns one provider client registration:

- Stable application ID and OAuth profile compatibility identity
- Callback mode and redirect registration
- Public client ID and optional provider project identity
- Current protected client-secret generation
- Application revision and status

The same application can authorize many accounts and compatible API definitions.
GraphQL, models, logs, and ordinary artifacts never receive the client secret.

### 4.3 External account and authorization grant

The external account owns a stable provider identity, profile identity, display label, and identity revision.
The label is not stable authority.

When stable identity is unavailable, the random grant ID keeps accounts distinct.
The UI must then permit a human label.

The grant owns:

- Application and optional external account references
- Explicit token audience
- Exact requested and provider-returned scopes
- Desired access and grant authority revisions
- Current protected token generation
- Active, authentication-required, revoked, or blocked status

The token generation owns access tokens, refresh tokens, and expiry.
It does not own API policy.

One grant can serve several connections only when the reviewed profile permits their shared audience.

### 4.4 API definition and connection

The definition keeps the fixed-origin request boundary:

- Stable definition and adapter identities
- One HTTPS origin and reviewed operations
- Request and response transforms
- Tool behavior facts
- OAuth profile reference
- Operation-specific authorization requirements

The definition contains no OAuth client setup or account tokens.

The connection owns:

- Stable connection ID and tool namespace slug
- Exact definition digest and grant ID
- Active or suspended desired state
- Enabled operations
- Connection policy and tool overrides
- Connection and policy revisions

The connection contains no granted scopes, client details, tokens, or secret paths.

## 5. Operation Authorization

Replace the definition-wide scope list with requirements on each operation.

Each operation can declare one or more accepted scope sets.
Every set contains all scopes required together.

```text
list_messages accepts:
  - gmail.readonly
  - gmail.modify

send_message accepts:
  - gmail.send
  - gmail.modify
```

The compiler normalizes sets and rejects empty, duplicate, oversized, or ambiguous requirements.

Tool availability uses exact set inclusion against provider-returned scopes.
A connection can therefore be partially available.

The runtime does not advertise unavailable operations to models.
Settings shows them with an **Add access** action.

## 6. Stored State

Keep source data under `${NOEMA_HOME}/adapters/`.

```text
adapters/
  definitions/
  oauth-profiles/
  oauth-applications/[application_id]/
    application.json
    credentials/[generation_id].json
  external-accounts/[account_id]/account.json
  oauth-grants/[grant_id]/
    grant.json
    tokens/[generation_id].json
  connections/[connection_id]/connection.json
  quarantine/
```

Each descriptor uses a strict schema version and normalized JSON encoding.
Descriptors contain no client secrets or tokens.

Reuse the current private filesystem, staging, synchronization, immutable generation, and atomic promotion rules.
Do not introduce a generic credential vault.

SQLite remains a disposable projection.
Append one forward-only migration that:

1. Adds projections for profiles, applications, accounts, and grants.
2. Replaces connection credential columns with `grant_id` and derived availability.
3. Adds indexes for profile, account, grant, definition, and connection reads.
4. Terminalizes active adapter authentication requests that use the replaced authority.
5. Preserves completed authentication and action history.

Test an existing-version upgrade and fresh-schema convergence.
Reconcile every projection from one complete filesystem snapshot.

## 7. Application and Grant State Machines

### 7.1 Application import

Application import is independent from API connection creation.

1. Resolve the exact reviewed profile and current callback mode.
2. Normalize the transient client document through the profile.
3. Reject unsupported document shapes and callback registrations.
4. Store the secret in a protected immutable generation.
5. Publish the public application descriptor separately.
6. Return only non-secret metadata.

An exact match reuses the application.
A conflicting secret needs an explicit replacement command.

Match profile compatibility, callback mode, and public client ID.
Never deduplicate by a provider label.

### 7.2 New account and added access

Use one authorization state machine for both actions.
The attempt captures:

- Human, application, and optional existing grant identities
- Exact application and grant revisions
- Target audience and target scopes
- Callback mode, redirect URI, expiry, state, and PKCE
- Definition and operation context

For a new account, the profile supplies reviewed account-selection behavior.
The callback exchanges the code and resolves account identity.

If the same application, account, and audience already have a grant, return that grant.
Do not create a duplicate silently.

For added access, calculate the smallest target scope set that covers selected operations.
Keep the current grant active during authorization.

Denial, expiry, or network failure leaves the current token and read access unchanged.
Successful completion promotes the new token and grant descriptor atomically.

Provider-returned scopes are authoritative.
Operations remain unavailable when the provider omits their required scopes.

If the response omits `scope`, apply only the reviewed protocol rule for omitted scope values.
Do not infer scope retention from earlier grants.

If expansion omits a refresh token, retain the current refresh token only when the profile permits continuity.
The application, account, and audience must remain exact.

### 7.3 Refresh

Refresh operates under a grant lock, not a connection lock.
Concurrent dependent connections share one refresh result.

Refresh changes only the token generation.
It does not revise connection policy or invalidate reviewed work by itself.

An invalid grant gives every dependent connection derived authentication-required health.
Their desired state remains active.

## 8. Catalog, Invocation, and Recovery

Catalog compilation combines:

- Exact definition and operation digests
- Connection and policy revisions
- Grant authority revision
- Operation scope satisfaction
- Connection desired state

Operation tokens stop carrying token generation paths and client credential revisions.

Invocation must:

1. Recheck definition, operation, connection, policy, and grant authority.
2. Confirm that one accepted scope set remains satisfied.
3. Load the current protected token generation.
4. Refresh once when required.
5. Send the bearer token only to the reviewed fixed origin.

Cursor bindings use the connection, definition, operation, account, and grant authority revision.

Scope expansion changes the grant authority revision.
Old delayed actions fail their exact check and request a current decision.

Authentication recovery must reference the grant and exact missing operation access.
Successful access expansion resumes only compatible paused calls.

## 9. Lifecycle Commands

| Command | Effect |
| --- | --- |
| Delete connection | Removes one API binding and policy; keeps the grant and application |
| Disconnect account | Quarantines grant tokens and disables dependent connections |
| Delete application | Requires all dependent grants to be disconnected |
| Replace application document | Adds a protected application generation |
| Suspend connection | Stops its tools without changing account consent |
| Disable operation | Changes local policy without claiming remote scope reduction |

Remote token revocation is separate behavior.
Add it only when the reviewed profile supports an exact revocation endpoint.

## 10. GraphQL Contract

Replace connection-shaped OAuth setup with application and grant commands.

Read models must cover:

- Non-secret OAuth applications
- External accounts, grants, scopes, and dependent connections
- Definition operation access requirements
- Setup options for one definition
- Per-operation availability and missing-access reasons
- OAuth attempt status

Commands must cover:

- Import or replace an application
- Start new-account authorization
- Start grant access expansion
- Attach a definition to a grant
- Create a direct-credential connection
- Disconnect a grant
- Delete an application
- Delete or suspend a connection

Every command carries exact descriptor revisions.
No command accepts a client secret after application import.

Add an OAuth-attempt subscription for completion, failure, expiry, and supersession.
Foreground recovery refetches exact state after subscription loss.

Update the shared schema, Web operations, and iOS Apollo operations together.

## 11. Web UX

Keep `/settings/tools/apis` as the main route.
Change its hierarchy from definition-first to account-first.

### 11.1 Main view

Use this information order when data exists:

1. APIs that need review before connection
2. Provider and account groups
3. Connected APIs below each account
4. APIs that can be connected
5. OAuth clients and raw details behind an advanced disclosure

Do not show an empty section for each absent data type.
Show one **No APIs set up** state when no current API exists.
That state routes the person to Chat, where API setup starts.

Do not use reviewed definitions as user-facing status.
Do not show a success message when the reviewed-definition list is empty.

Use dense settings rows.
Do not wrap every account or API in a decorative card.

Each account row shows its label, connection count, and blocking state.
Each API row shows available tools and local suspension state.

### 11.2 Setup flow

Select the shortest safe path:

1. Reuse a compatible grant that already covers the operations.
2. Offer existing accounts that need added access.
3. Offer **Add account** when a compatible application exists.
4. Request application setup only when no compatible application exists.

Use result-specific actions: **Connect Gmail**, **Add Calendar access**, or **Add Google account**.

An added-access decision explains newly enabled operations first.
Show exact scope strings and all dependent connections under **Technical details**.

Dismissal leaves current access unchanged.
Existing available tools remain visible during authorization.

### 11.3 Management details

Call OAuth applications **OAuth clients** in user-facing copy.
Show them as advanced provider setup only after one client exists.
Display public client ID, callback mode, redirect URI, project label, and account count.

Never display the client secret.
Credential replacement is an explicit dialog with an impact summary.

Keep policy and tool controls on the current connection detail surface.
Add account identity and per-operation access states.

Use **Reconnect account** or **Add access** instead of **Authorize connection**.
Connection deletion copy states that the account remains connected.

### 11.4 Chat and Tasks

Replace the combined setup card with one current action:

- Review API definition
- Import OAuth client
- Choose or add account
- Add access
- Review connection policy

The backend supplies a structured next action.
Clients must not infer it from status text.

## 12. iOS and iPadOS UX

Use the same hierarchy and action meanings as Web.

### 12.1 Settings

Change the current definition-card list into provider and account sections.
Show connected APIs as rows below each account.

Keep blocking definition review first.
Put source and manifest data in disclosures.

On iPad, reuse the current list and detail composition.
On iPhone, use sequential sheets with the same information order.

### 12.2 Application setup

Use the existing document importer only when no compatible application exists.
Title this action **Import [provider] OAuth client**.

After one client exists, expose **Import another [provider] OAuth client** in advanced setup.
Do not expose alternate-client management in the initial empty state.

Show the required redirect URI before document selection.
Never persist selected document bytes on the device.

The server normalizes transient bytes and stores the protected application.

### 12.3 Account authorization

Reuse the current `SFSafariViewController` handoff.
Start it from **Add account**, **Add access**, or **Reconnect account**.

Subscribe to the attempt while active.
After foreground recovery or sheet dismissal, refetch the grant and connections.

Do not poll.
Keep current access when authorization fails or Safari closes.

Use one bounded decision per sheet.
Show operation benefits before exact scope strings.

Use a sequential flow when application setup, account authorization, and policy are all required.
Preserve the current native policy and tool editors.

### 12.4 Chat and Tasks

Update native intervention models and views for the same five actions as Web.

Do not select the first authentication-required connection.
Use exact application, grant, and connection IDs from GraphQL.

Regenerate Apollo sources after schema or operation changes.
Never edit generated Swift manually.

## 13. Product States and Copy

Use object-specific failures:

- **OAuth application unavailable:** Import or replace the client document.
- **Account authorization required:** Reconnect the named account.
- **Additional access required:** Approve the listed new access.
- **API not enabled:** Enable the API in the provider project.
- **Connection suspended:** Resume the API connection locally.
- **Definition unavailable:** Review or replace the API definition.

An API-enablement error must not request another upload.
Do not use one generic **credentials required** state.

## 14. Clean Cutover

Do not retain runtime readers for old OAuth connection credentials.

At first startup:

1. Move old OAuth connection directories into versioned quarantine.
2. Quarantine schedules and cursors that reference those connections.
3. Preserve definitions, source bytes, and historical SQLite records.
4. Mark affected integrations as requiring setup.
5. Remove old connection projections during reconciliation.

Direct-credential and credential-free connections can receive a bounded one-time rewrite when semantics remain exact.

Before cutover, author replacements for the active Gmail and Calendar definitions.
Both reference one reviewed Google OAuth profile.

Other version-eight OAuth definitions require a new proposal and review.
Do not keep a permanent compatibility reader.

Never delete old secret files during cutover.
Quarantine preserves rollback evidence.

## 15. Delivery Milestones

Each milestone ends with a commit, size report, focused validation, and current-context update when needed.

### 1. Contract and stores

- Add profile, application, account, grant, and revised connection models.
- Add strict stores, protected generations, paths, quarantine, and projections.
- Append and test the SQLite migration.

Rollback: no runtime uses the new objects.

### 2. Definitions and catalog

- Advance the manifest and compiler.
- Add operation authorization requirements and profile compilation.
- Compile partial operation availability.
- Replace operation token authority fields.

Rollback: no external call uses the new catalog.

### 3. OAuth runtime

- Import reusable applications.
- Create accounts and expand grants.
- Move refresh locking and token publication to grants.
- Rework authentication recovery and action resumption.

Rollback: connections remain disabled while new objects stay recoverable.

### 4. GraphQL and Web

- Replace setup queries and commands.
- Add attempt events and foreground reconciliation.
- Implement account-first Settings, Chat, and Tasks flows.
- Regenerate Web GraphQL types.

### 5. iOS and iPadOS

- Add account-first Settings and authorization flows.
- Update Chat and Tasks interventions.
- Regenerate Apollo sources.
- Validate iPhone and iPad layouts.

### 6. Cutover and removal

- Quarantine old OAuth state.
- Remove old readers, fields, forms, and generated types.
- Set up active accounts again.
- Run live Gmail and Calendar validation.

## 16. Expected Areas

| Area | Main authorities |
| --- | --- |
| Paths and stores | `noema-home`, adapter stores, private filesystem helpers |
| Definition and runtime | Adapter definition, compiler, catalog, invocation, OAuth, service |
| Projections | `noema-store` schema and adapter reconciliation |
| Recovery | Adapter authentication paths in `noema-runtime` |
| Composition | Host startup, scans, and catalog refresh |
| GraphQL | Adapter query, command, subscription, and schema files |
| Web | Adapter operations, API settings, connection detail, interventions |
| iOS | Settings and Chat operations, models, flows, views, generated Apollo sources |
| Documents | Project storage contract, frontend contract, adapter security, current context |

Prefer changing existing authorities.
Add focused application or grant store modules only when they create clear ownership.

## 17. Test Risk Matrix

More than ten Rust tests are justified by secret ownership, shared concurrency, and authorization replacement.

| Risk | Authoritative proof |
| --- | --- |
| Unsafe inferred reuse | Compiler rejects undeclared application or audience reuse |
| Secret disclosure | Application projections and debug output preserve public values only |
| Duplicate import | Exact import is idempotent under one application lock |
| Account overwrite | Two grants share one application and retain separate tokens |
| Lost current access | Failed expansion preserves the old grant |
| Missing provider scopes | Unsupported operations remain unavailable |
| Excess tool access | Catalog enables only satisfied operation sets |
| Duplicate refresh | Concurrent connections publish one grant token generation |
| Stale delayed action | Invocation rejects a changed grant authority |
| Token rotation staleness | Refresh does not change grant authority |
| Destructive deletion | Connection deletion preserves shared authorization |
| Dependency break | Application changes enforce grant dependency checks |
| Migration loss | Existing upgrade preserves history and rebuilds projections |
| Schema drift | Fresh and upgraded schemas converge |
| Executable old secrets | Cutover quarantines old connections before catalog compilation |

Use table-driven cases when inputs share one branch.
Avoid duplicate tests across store, API, and runtime layers.

Do not add Web or iOS UI tests unless separately requested.
Use generation, builds, and authorized visual inspection.

## 18. Validation and Live Acceptance

Run these gates at the applicable milestone:

- Rust formatting, focused `cargo validate` tests, workspace check, lint gate, and test gate
- Rust size report with milestone production, test, and new-test limits
- Web GraphQL generation, lint, and production build
- Apollo generation and unsigned generic iOS Simulator build
- Authorized desktop, mobile Web, iPhone, and iPad visual inspection

Live acceptance:

1. Import one Google OAuth application.
2. Connect Gmail for account A.
3. Add Gmail write access without another upload.
4. Add account B without another upload.
5. Connect Calendar for account A without another upload.
6. Deny added Calendar access and confirm Gmail reads remain available.
7. Refresh one grant while Gmail and Calendar are active.
8. Delete Calendar and confirm Gmail and the account remain connected.

## 19. Budget and Stop Conditions

Initial total estimates:

- Rust production: 1,800–2,800 net lines
- Rust tests: 700–1,100 net lines
- Web production: 500–900 net lines
- Swift production: 500–900 net lines
- New Rust tests: 12–15 focused tests
- New UI tests: none

Old connection credential code, GraphQL fields, and UI forms must be removed.
Apply explicit size limits per milestone.

Stop and request direction when:

1. Token sharing lacks an explicit reviewed audience contract.
2. Stable identity requires unreviewed token parsing or English matching.
3. Shared grants require policy to move from connections.
4. MCP or model-provider OAuth must change.
5. Cutover would delete old secrets instead of quarantining them.
6. A milestone crosses more than two architectural areas or exceeds its budget threshold.

## 20. Completion Criteria

- One Google client document creates one reusable OAuth application.
- Two Google accounts use that application through separate grants.
- Gmail and Calendar can attach to one compatible account grant.
- Added write access requires no new upload.
- Covered operations remain available during expansion.
- Refresh and reauthentication operate at grant scope.
- Web and iOS use the same account-first hierarchy and action meanings.
- No forbidden sink receives client secrets or tokens.
- Old OAuth connection credentials have no executable compatibility path.
- SQLite upgrade and fresh schema converge.
- Live Gmail and Calendar acceptance passes.
