# Gmail fixture contract

Selected API: Gmail v1. Sources inspected on 2026-09-06:

- [Message and MIME part schema](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages), updated 2026-05-06.
- [Message list schema](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/list).

Messages preserve immutable IDs, thread IDs, labels, timestamps, headers, and encoded body data.
A multipart message contains separate text and attachment parts.
An attachment part supplies its filename and attachment body reference.

List requests accept a maximum result count and a page token.
Responses contain message IDs, thread IDs, an estimated total, and an optional next-page token.
The fixture caps pages at three messages to require traversal of all seven records.
The last response omits the next-page token.

Fixture v2 corrects the earlier multipart body placement and caps the page size.
Two focused protocol tests check complete pagination and separate MIME parts.
The public process must report v2 before these changes count as live evidence.

OAuth, expiry, scope errors, and rate-limit injection are still absent from this fixture.
The current bearer credential setup does not satisfy the plan's OAuth acceptance requirement.
# Mock OAuth implementation

Fixture v3 adds authorization and token endpoints under `/oauth`.
The request and response fields follow [Google's web-server OAuth contract](https://developers.google.com/identity/protocols/oauth2/web-server).
This is a synthetic subset, not Google's account system or full consent interface.

The launcher must supply `NOEMA_FIXTURE_CLIENT_SECRET` through a protected environment.
Never place its value in Chat, documentation, command output, or request traces.
The client ID is `noema-synthetic-gmail`.
The registered redirect is `https://noema.kevinpei.com/adapter/oauth/callback`.
The operator must confirm that Noema advertises this callback before importing the client document.

Consent offers Alex, Blair, or denial. Codes expire after five minutes and permit one exchange.
Code exchange checks the client, exact redirect, and S256 verifier.
Access tokens expire after two minutes. Refresh preserves the selected account and omits a replacement refresh token.
OAuth routes do not record request URLs or bodies in the fixture trace.
Existing static test tokens remain available for the earlier read-only connection evidence.

Two focused tests verify consent, denial, redirect rejection, code replay rejection, incorrect verifier rejection,
expiry, refresh, account preservation, and omission of OAuth requests from ordinary traces.
`CGO_ENABLED=0 go test ./cmd/noema-provider-fixtures` and the matching `go vet` command pass.
No application suite was repeated because this unit changes only the fixture and its tests.

The public health endpoint still reports v1. Deployment and live Noema OAuth acceptance remain pending.
