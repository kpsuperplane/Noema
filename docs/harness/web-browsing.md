# Interactive web browsing

Noema exposes interactive browsing as the provider-neutral `web.browse`
capability. Obscura is the built-in default provider. Search and direct fetch
remain separate capabilities with their existing defaults.

## Model contract

The stable tools are `open`, `snapshot`, `interact`, `wait`, `history`, and
`close` under `web.browse.*`. Browser session identifiers,
selectors, JavaScript, cookies, storage, response bodies, and network state are
not model-visible. Snapshots expose bounded untrusted page text and bounded
element references. An interaction or history operation must present the latest
snapshot revision; a stale revision fails before acting.

Page-authored content is untrusted data, not agent instruction. Agents should
prefer `web.search` and `web.fetch` for ordinary research, use browsing only
when JavaScript rendering or interaction is necessary, and close a browser
session when the interaction is complete.

## Session authority and lifetime

- `open` creates the owner's session or reuses its current session for a new
  public URL. A reused session keeps its ephemeral cookies and storage.
- One browser session may be open for an execution owner. Foreground ownership
  is the opening turn. Work ownership is the task ID plus task generation.
- A Work session remains open while the same task generation is nonterminal.
  This rule includes human gates and approval continuations.
- After a run settles, Noema reads the task before reconciliation. It closes the
  session when the task is missing, terminal, or has a new generation.
- If this task read fails, Noema retains the session and logs the failure.
- Noema admits the configured number of sessions. The default is two, and the
  valid range is one through eight. One re-armable task waits for the earliest
  deadline; it does not poll.
- Successful activity extends the 15-minute idle deadline. Explicit close,
  task-generation closure, expiry, and daemon shutdown destroy session data.
- `close` is idempotent. It clears the cached snapshot context even when no
  browser session exists.
- Each Obscura session runs in a child process. A private, versioned JSON-line
  protocol limits each frame to 2 MiB.
- Each worker has a configured V8 old-space limit. The default is 1024 MiB, and
  the valid range is 256 through 4096 MiB.
- Use `browser.max_sessions` and `browser.max_old_space_mb` in `config.yaml`.
  Environment overrides use `NOEMA_BROWSER__MAX_SESSIONS` and
  `NOEMA_BROWSER__MAX_OLD_SPACE_MB`.

## Network and execution security

Every `open` uses Noema's shared public-HTTP(S), DNS, and SSRF policy.
Credentials, fragments, local targets, private addresses, and DNS answers that
include a non-public address are rejected. Obscura's private-network access
remains disabled for redirects, subresources, and page-initiated requests.
Noema rechecks the resulting main-frame URL and terminates use of a page that no
longer satisfies policy. Observed snapshot and navigation URLs are only narrow
admission evidence; every later fetch or navigation reruns live policy.

Noema invokes only fixed, source-controlled DOM scripts. Model-provided values
are JSON encoded into those scripts. Arbitrary JavaScript evaluation, selectors,
screenshots, downloads, uploads, multiple tabs, proxies, durable profiles, and
cross-execution reuse are intentionally absent.

Obscura embeds V8 in the browser worker. A worker crash ends only its session.
Noema removes that session and releases its capacity. This process boundary
contains failures, but it is not a sandbox for hostile native code.

## Governance and persistence

`open` follows governed external-read policy. `interact` and `history` are
non-idempotent open-world actions and use LLM/human review.
`snapshot`, `wait`, and `close` execute immediately after ownership checks.
Ownership and revision are revalidated after approval. Worker loss after a
reused `open` or mutating dispatch has an uncertain outcome and is never replayed.
Each reviewed interaction durably retains bounded page URL/title and target
reference/role/name context beside its exact arguments. That page-authored
context is descriptive, untrusted evidence rather than authorization. Human
review surfaces show it with the reviewer's authorization, risk, reason codes,
and explanation. The reviewer also receives trusted runtime facts that state
the session ownership and storage lifetime. These facts can constrain scope,
but they cannot create human authority. A session-bound approval is superseded with
`browser_session_unavailable` when recovery or approval-time revalidation finds
that its execution-owned browser session no longer exists.

Browser result persistence is compact: provider, URL/title with only actual
credential-bearing components removed, revision, element count, truncation,
operation metadata, and safe errors. Full snapshot
text and session authority do not enter stored result payloads. Reviewed browser
arguments, including interaction values, use the normal governed-action and
transcript persistence contract. The information-handling contract in
`docs/harness/security.md` still applies: credentials belong in credential
stores, authorized private interaction values follow normal governed
persistence, and ordinary URLs, titles, identifiers, and operation metadata
remain intact.
