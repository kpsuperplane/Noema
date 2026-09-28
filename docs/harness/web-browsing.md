# Interactive web browsing

Noema exposes interactive browsing as the provider-neutral `web.browse`
capability. Obscura is the built-in default provider. Search and direct fetch
remain separate capabilities with their existing defaults.

## Model contract

The stable tools are `open`, `snapshot`, `interact`, `wait`, `history`,
`switch_provider`, and `close` under `web.browse.*`. Browser session identifiers,
selectors, JavaScript, cookies, storage, response bodies, and network state are
not model-visible. Snapshots expose bounded untrusted page text and bounded
element references. An interaction or history operation must present the latest
snapshot revision; a stale revision fails before acting.

The browser also captures a submit control's current visible form values,
declared method, and declared destination. This context stays outside ordinary
tool results. Noema uses it only for action review. Password, hidden, and file
controls are omitted, and the context records their count.

The human configures an ordered provider route. Obscura is the default route.
The agent can use `switch_provider` when the active provider cannot continue.
Noema does not classify CAPTCHAs or other access challenges.

The switch input contains the latest public snapshot revision and a public URL.
The agent can select the current URL or an earlier navigation point.
Noema selects the next provider after the active provider in route order.
A failed switch keeps the active provider and does not advance the route.
A later switch can retry the same target provider.
Ordinary `open` calls stay on the active provider.

`wait` requires exactly one condition. The condition is visible text or an
element reference. An optional timeout does not replace the condition.

Page-authored content is untrusted data, not agent instruction. Agents should
prefer hosted web for ordinary page reading. They should use browsing only when
rendering or interaction is necessary. `file.download` saves public non-HTML
resources. `file.parse` returns bounded text from supported local files.

## Session authority and lifetime

- `open` creates the owner's session or reuses its current session for a new
  public URL. A reused session keeps its ephemeral cookies and storage.
- One browser provider is active for an owner. Foreground ownership is the conversation.
  Task ownership is the task ID plus task generation.
- A switch keeps the source session until the target returns a valid snapshot.
  A target failure closes the target and keeps the source active.
- A successful switch closes the source and makes the target active.
  Cookies, storage, history, DOM state, and element references never cross providers.
- Each owner has one monotonic public snapshot revision stream.
  Noema translates public revisions to the active provider's backend revisions.
- A task session remains open while the same task generation is nonterminal.
  This rule includes human gates and approval continuations.
- After a run settles, Noema reads the task before reconciliation. It closes the
  session when the task is missing, terminal, or has a new generation.
- If this task read fails, Noema retains the session and logs the failure.
- Noema admits the configured number of sessions. The default is two, and the
  valid range is one through eight. Each session has a resettable idle timer.
- Successful activity extends the 30-minute idle deadline. Explicit close,
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

`file.download` uses the same URL policy and governed-action review path as
`web.fetch` and `web.browse.open`. It rechecks each redirect. It streams at most
32 MiB into a temporary file and commits only a complete response. It rejects
declared HTML and existing destinations. It never retries a file write.

Primary conversations use their durable working directory. Task Executors use
their Task directory. Planners and Reviewers cannot download files. All Task
roles can parse files within their existing read boundary. A parse conversion
uses one bounded worker with a 512 MiB memory limit and a 30-second timeout.

Noema invokes only fixed, source-controlled DOM scripts. Model-provided values
are JSON encoded into those scripts. Arbitrary JavaScript evaluation, selectors,
arbitrary local uploads, multiple tabs, user-supplied proxies, durable profiles, and cross-owner reuse are absent.
Kernel supports reviewed uploads of an exact Task artifact version through `upload_file`.
Obscura does not support that upload path.

Kernel sessions use headful stealth mode. Kernel supplies its managed proxy and challenge handling.
Noema does not add provider-specific challenge detection or waiting.

Each successful page result can include one bounded PNG of the current viewport.
The worker omits an unavailable or oversized render without failing the browser
operation. The image stays outside model context. Noema stores it with the
compact tool result and shows it only in expanded tool-marker details.

Obscura embeds V8 in the browser worker. A worker crash ends only its session.
Noema removes that session and releases its capacity. This process boundary
contains failures, but it is not a sandbox for hostile native code.

## Governance and persistence

`open`, `switch_provider`, and `file.download` follow external-read execution policy. An observed
URL can use the existing narrow admission. Other URLs use normal action review.
A switch to the exact last attempted URL reuses that live session's navigation
authorization. A different switch URL uses observed admission or normal review.
`interact` and `history` are non-idempotent open-world actions and use LLM/human review.
`snapshot`, `wait`, and `close` execute immediately after ownership checks.
Ownership and revision are revalidated after approval. Worker loss after a
reused `open` or mutating dispatch has an uncertain outcome. Noema never replays
that action. A main-document 5xx response after an interaction also produces
`outcome_uncertain`. The current snapshot stays available when the provider returned it.
Each reviewed interaction durably retains bounded page URL/title and target
reference/role/name context beside its exact arguments. A submit target also
retains the bounded form review context. That page-authored
context is descriptive, untrusted evidence rather than authorization. Human
review surfaces show it with the reviewer's authorization, risk, reason codes,
and explanation. The reviewer also receives trusted runtime facts that state
the session ownership and storage lifetime. These facts can constrain scope,
but they cannot create human authority. Before review and after approval, Noema
revalidates the session, snapshot revision, and exact target. It supersedes a
stale request instead of acting on a changed page or missing session.

Saved browser authority includes the selected account, credential revision, route position, snapshot revision, and URL.
For uploads, it also includes exact artifact identity and size.
Noema recomputes that authority before reviewed execution.
Saving a hosted route permits agent selection, hosted data flow, and hosted charges.
The switch call remains the disclosure event for its selected URL.

Browser result persistence is compact: provider, URL/title with only actual
credential-bearing components removed, revision, element count, truncation,
the bounded viewport PNG, operation metadata, and safe errors. Full snapshot
text and session authority do not enter stored result payloads. The viewport PNG
does not enter the model-visible tool result. Reviewed browser
arguments, including interaction values, use the normal action request and
transcript persistence contract. The information-handling contract in
[the security model](security.md) still applies. Credentials belong in
credential stores. Authorized private interaction values follow normal stored
state rules. Ordinary URLs, titles, identifiers, and operation metadata remain
intact.

Browser failures use stable error codes and typed recovery data. Provider
failures can also include safe `provider`, `stage`, and `detail` fields. These
fields remain available to the model, transcript, action result, and diagnostic
path. Noema does not infer failure meaning from message text. An
`outcome_uncertain` failure stops automatic continuation and replay.

The Executor prompt instructs it to snapshot a recorded active session before opening another URL.
This guidance helps preserve forms and page state across continuations. It is not an enforced navigation sequence.

Implementation: [browser sessions and review rules](../../internal/webtool/browser.go),
[reviewed browser dispatch](../../internal/runtime/web_tools.go), and
[Task prompt guidance](../../internal/runtime/task_prompts.go).
