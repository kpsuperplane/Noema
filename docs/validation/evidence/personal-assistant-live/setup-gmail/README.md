# Setup 1 — Gmail API connection

Updated: 2026-09-10

Status: Pass with explicit rate-limit follow-up

This setup ran through the live Go Noema server over the private GraphQL
socket. It used the hosted synthetic fixture only. It did not access a real
Google account or send mail.

## Setup contract

| Item | Observed value |
| --- | --- |
| Backend | Go development server; authenticated socket request returned `{"state":"authenticated"}` |
| Fixture | `2026-09-08-gmail-v1-notion-mcp-v8`; [health](https://noema.kevinpei.com/__pa-replay/health) |
| Documentation | [Gmail fixture guide](https://noema.kevinpei.com/__pa-replay/gmail/docs) |
| Generated definition | `definition:synthetic_gmail_readonly` |
| Definition digest | `a9ae0debc419298448c0f82d63cad98f8f1f59ff9f1b835da5e69b67b672dfb4` |
| OAuth scope | `https://www.googleapis.com/auth/gmail.readonly` |
| Accepted operations | `get_message`, `get_profile`, `list_messages`, `get_attachment` |
| Accepted connection | `67fb2d7ce5cfa1a008080625568fec67` |
| Connection / grant / policy revisions | `5` / `3` / `3` |
| Noema behavior revision | `1add8c9e` |

The agent first read the guide and proposed the definition. The operator
accepted that exact reviewed revision through Noema. The initial setup turn is
`turn:dd3b8b8c740f5938e0387690826930b1`.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| First read after setup | Pass | The connected profile and trip messages were read through the accepted connection. |
| Later-turn reuse | Pass | `pa-setup-gmail-20260910-reuse-1` reused the existing connection without a new proposal. |
| Delegated reuse | Pass | `pa-setup-gmail-20260910-delegated-reuse-1`; Task `task:1e2ad099dce1782a8dfbaa6b975396fb` finished and the reviewer approved it. |
| Token expiry | Pass | `pa-setup-gmail-20260910-expiry-1` recovered a read after the fixture expired its token. |
| One-shot rate limit | Pass with limitation | `pa-setup-gmail-20260910-rate-limit-1` surfaced the fixture's 429 without changing data or retrying automatically. The natural follow-up `pa-setup-gmail-20260910-rate-limit-retry-1` succeeded. This proves safe recovery, not automatic 429 retry. |
| Pagination | Pass | `pa-setup-gmail-20260910-pagination-1` read both pages and all four matching trip messages. |
| Empty search | Pass | `pa-setup-gmail-20260910-empty-1` returned no records for an exact unmatched phrase and gave a clear final answer. |
| Attachment read | Pass | Message `a-msg-007` and attachment `a-attachment-001` returned the expected packing-list text. |
| Account boundary | Pass | `pa-setup-gmail-20260910-account-boundary-1` selected the Blair connection and returned only Blair's private note. |
| Read-only boundary | Pass | The accepted definition exposes no write operation. Fixture traces showed only GET reads during these checks. |

The fixture request trace is available at
[fixture/requests](https://noema.kevinpei.com/__pa-replay/fixture/requests).
The trace is synthetic and contains no OAuth URLs, codes, or credential
values.

## Limit

The OAuth flow, accounts, and messages are emulated. This setup does not prove
Google's real consent screen or a real mailbox. The rate-limit case also uses a
safe explicit follow-up because the current adapter does not automatically
retry HTTP 429 responses.
