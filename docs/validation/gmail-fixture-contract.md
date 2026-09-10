# Gmail fixture contract

Updated: 2026-09-10

Live revision: `2026-09-08-gmail-v1-notion-mcp-v8`

This fixture implements the bounded Gmail v1 contract used by the live
personal-assistant setup. It is a synthetic service. It is not Google's
account system and it cannot send real mail.

## API shape

The contract follows the [Gmail message and MIME part schema](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages)
and the [Gmail list schema](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/list).

Messages preserve immutable IDs, thread IDs, labels, timestamps, headers, and
encoded body data. Multipart messages preserve separate text and attachment
parts. Attachment reads return the declared decoded size and body.

List requests accept a search query, result count, and page token. Responses
return message and thread IDs plus an optional next-page token. The fixture
uses bounded pages so the setup test must follow pagination.

The accepted read-only setup definition exposes these operations:

- `get_profile`
- `list_messages`
- `get_message`
- `get_attachment`

The setup definition uses the `https://www.googleapis.com/auth/gmail.readonly`
scope only. A separate older synthetic definition still exists for earlier
write-path evidence, but it is not part of Setup 1 acceptance.

## Synthetic OAuth

The fixture implements a bounded authorization-code flow with PKCE under
`/oauth`. It follows the [Google web-server OAuth contract](https://developers.google.com/identity/protocols/oauth2/web-server)
for the fields used here.

The fixture offers synthetic account-A, account-B, or denial. Codes expire
after five minutes and permit one exchange. The exchange checks the client,
exact redirect, and S256 verifier. Access tokens expire after two minutes.
Refresh preserves the selected account and may omit a replacement refresh
token. OAuth routes do not record request URLs or bodies in the ordinary
fixture trace.

The launcher supplies the client secret through a protected environment
binding. Do not place that value in Chat, documents, logs, or traces.

## Fault controls

The fixture control endpoint can inject these setup conditions:

- `gmail_expire_tokens`: expire the selected token before the next read.
- `gmail_rate_limit_once`: return one HTTP 429 with `Retry-After: 0`.

The current adapter reports HTTP 429 safely and waits for a normal user
follow-up. It does not retry 429 automatically. The live setup evidence records
both the safe error and the successful follow-up.

The fixture also supports the Notion and Calendar controls described by their
own setup contracts. Its public health endpoint is
[health](https://noema.kevinpei.com/__pa-replay/health), and the public Gmail
guide is [Gmail docs](https://noema.kevinpei.com/__pa-replay/gmail/docs).

## Validation

Focused fixture tests cover consent, denial, redirect rejection, code replay,
verifier rejection, expiry, refresh, account preservation, pagination,
multipart attachment reads, and one-shot rate limiting. The live setup result
is recorded in [Setup 1 evidence](evidence/personal-assistant-live/setup-gmail/README.md).

Synthetic OAuth cannot prove compatibility with Google's real consent screen,
real account policies, or production quota behavior.
