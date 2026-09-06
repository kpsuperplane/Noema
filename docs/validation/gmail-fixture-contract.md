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
