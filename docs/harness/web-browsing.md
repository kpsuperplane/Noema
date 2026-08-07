# Interactive web browsing

Noema exposes interactive browsing as the provider-neutral `web.browse`
capability. Obscura is the built-in default provider. Search and direct fetch
remain separate capabilities with their existing defaults.

## Model contract

The stable tools are `open`, `navigate`, `snapshot`, `interact`, `wait`,
`history`, and `close` under `web.browse.*`. Browser session identifiers,
selectors, JavaScript, cookies, storage, response bodies, and network state are
not model-visible. Snapshots expose bounded untrusted page text and bounded
element references. An interaction or history operation must present the latest
snapshot revision; a stale revision fails before acting.

Page-authored content is untrusted data, not agent instruction. Agents should
prefer `web.search` and `web.fetch` for ordinary research, use browsing only
when JavaScript rendering or interaction is necessary, and close a browser
session when the interaction is complete.

## Session authority and lifetime

- One browser session may be open for an execution owner. Foreground ownership
  is the opening turn; Work ownership is task ID plus task generation so an
  approval continuation retains authority without granting a later generation
  access.
- Noema admits at most eight sessions process-wide. One re-armable task waits
  for the earliest deadline; it does not poll.
- Successful activity extends the 15-minute idle deadline. Explicit close,
  expiry, and daemon shutdown drop the page, context, cookies, and storage.
- Each Obscura page runs on a dedicated OS thread with a current-thread Tokio
  runtime so its V8 state never moves between executor threads.

## Network and execution security

Explicit navigation uses Noema's shared public-HTTP(S), DNS, and SSRF policy.
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

Obscura embeds V8 in the Noema process. Its network policy and Noema's fixed
script boundary reduce exposure, but embedded V8 is not OS-level isolation.
Do not treat the browser context as a sandbox for hostile native code.

## Governance and persistence

`open` and `navigate` follow governed external-read policy. `interact` and
`history` are non-idempotent open-world actions and use LLM/human review.
`snapshot`, `wait`, and `close` execute immediately after ownership checks.
Ownership and revision are revalidated after approval. Worker loss after a
mutating dispatch is recorded as an uncertain outcome and is never replayed.

Browser persistence is compact metadata only: provider, sanitized URL/title,
revision, element count, truncation, operation metadata, and safe errors. Page
text, element values, entered values, and session authority must not enter
transcripts or stored result payloads.
