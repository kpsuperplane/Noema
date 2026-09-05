# Go server verification results

Started: 2026-09-05. Status: **In progress**.

The [suite](go-server-user-verification.md) defines the cases and required variants.
A passing unit test does not prove a complete user journey.
The case table records only evidence from this run.

## Build and environment

- Linux x86_64 with full access for the current run.
- Browser checks use a disposable Go home and synthetic records.
- Release source: `af0c239869c6cd3e8624bb0a4bd905ff300eb6ca`.
- Source snapshot and temporary logs: `/var/tmp/noema-suite-run-20260905`.
- Manual device and real-consent variants remain **Human later — Not run**.
- Native macOS and Windows execution still needs suitable runners.

## Initial checks

The full-access Go unit run passed all 604 tests.
The first sandboxed run could not create a Unix socket. The full-access rerun passed that check.
The [unit result list](evidence/2026-09-05-go-unit-results.json) records each outcome and source commit.
The Go vet check passed. The frontend lint check passed. The corrected frontend unit run passed all 70 tests.
Two menu expectations omitted Notifications and Clients. The current client contract includes both pages.
The test correction adds 19 lines and removes one line. It changes no production code.
The three protected relay scenarios also passed with full access.

The packaged Linux release has SHA-256 `e278abffc1d7b00aaf338a153a76c4c91b13b2c2c43049c7e682f7d7d2e7bc6b`.
The [browser script](evidence/2026-09-05-auth-check.mjs) uses synthetic passkeys and a disposable Unicode home.
Its [results](evidence/2026-09-05-auth-results.json) describe the exact assertions.
The script contains no credentials. Its paths identify this run's local setup.

The protected inspection preflight passed HTTP, a GraphQL query, mutation denial, and WebSocket acknowledgement.
That preflight used the existing development server. It does not count as isolated release evidence.

## Case results

**Partial** means that evidence covers only the named portion.
**Not run** means that this run has no complete case evidence yet.
A human variant remains pending even when its controlled counterpart passes.

| Case | Automatic result | Evidence or remaining work | Human variant |
| --- | --- | --- | --- |
| HOME-01 | Pass · Linux/Chromium | Packaged release opens fresh passkey setup. Other native platforms remain pending. | — |
| HOME-03 | Not run | Controlled setup pending. | — |
| HOME-04 | Not run | Controlled setup pending. | — |
| HOME-05 | Not run | Controlled setup pending. | — |
| HOME-06 | Partial | Release startup, passkey setup, recovery, and restart work in a Unicode home with spaces. Files and helpers remain pending. | — |
| HOME-07 | Not run | Controlled setup pending. | — |
| HOME-08 | Not run | Controlled setup pending. | — |
| HOME-09 | Not run | Controlled setup pending. | — |
| HOME-10 | Not run | Requires a native Windows runner. | — |
| AUTH-01 | Pass · Chromium virtual passkey | Initial claim admits authenticated GraphQL. Product setup follow-through remains in SETUP. | Not run |
| AUTH-02 | Not run | Controlled setup pending. | Not run |
| AUTH-03 | Partial | Authenticated session survives server restart. Browser close and reopen remain pending. | Not run |
| AUTH-04 | Partial | Final passkey removal returns 409. Additional-key management and recent-verification checks remain pending. | Not run |
| AUTH-05 | Pass · Chromium virtual passkey | Recovery enrolls a new key. Reusing the consumed recovery code returns 401. | Not run |
| AUTH-06 | Partial | Fresh login works after logout. Invalid, expired, and replayed browser ceremonies remain pending. | Not run |
| AUTH-07 | Partial | Logout denies reads and writes in another tab. Push cleanup and rendered Chat recovery remain pending. | Not run |
| AUTH-08 | Partial | Unclaimed reads and signed-out GraphQL, artifacts, and WebSocket are denied. Unclaimed WebSocket remains pending. | — |
| AUTH-09 | Pass · Chromium | Authenticated foreign-Origin requests return 403. Foreign-Host requests return 400. | — |
| AUTH-10 | Not run | Controlled setup pending. | Not run |
| SETUP-02 | Not run | Controlled setup pending. | — |
| SETUP-03 | Not run | Controlled setup pending. | Not run |
| SETUP-04 | Not run | Controlled setup pending. | Not run |
| SETUP-05 | Not run | Controlled setup pending. | — |
| SETUP-06 | Not run | Controlled setup pending. | — |
| SETUP-07 | Not run | Controlled setup pending. | — |
| SETUP-08 | Not run | Controlled setup pending. | — |
| SETUP-09 | Not run | Controlled setup pending. | — |
| SETUP-10 | Not run | Controlled setup pending. | — |
| CHAT-01 | Not run | Controlled setup pending. | — |
| CHAT-02 | Not run | Controlled setup pending. | — |
| CHAT-03 | Not run | Controlled setup pending. | — |
| CHAT-04 | Not run | Controlled setup pending. | — |
| CHAT-05 | Not run | Controlled setup pending. | — |
| CHAT-06 | Not run | Controlled setup pending. | — |
| CHAT-07 | Not run | Controlled setup pending. | — |
| CHAT-08 | Not run | Controlled setup pending. | — |
| CHAT-09 | Not run | Controlled setup pending. | — |
| CHAT-10 | Not run | Controlled setup pending. | — |
| CHAT-11 | Not run | Controlled setup pending. | — |
| CHAT-12 | Not run | Controlled setup pending. | — |
| CHAT-13 | Not run | Controlled setup pending. | — |
| CHAT-14 | Not run | Controlled setup pending. | — |
| CHAT-15 | Not run | Controlled setup pending. | — |
| MODEL-01 | Not run | Controlled setup pending. | — |
| MODEL-02 | Not run | Controlled setup pending. | — |
| MODEL-03 | Not run | Controlled setup pending. | — |
| MODEL-04 | Not run | Controlled setup pending. | — |
| MODEL-05 | Not run | Controlled setup pending. | — |
| MODEL-06 | Not run | Controlled setup pending. | — |
| MODEL-07 | Not run | Controlled setup pending. | — |
| MODEL-08 | Not run | Controlled setup pending. | — |
| TASK-01 | Not run | Controlled setup pending. | — |
| TASK-02 | Not run | Controlled setup pending. | — |
| TASK-03 | Not run | Controlled setup pending. | — |
| TASK-04 | Not run | Controlled setup pending. | — |
| TASK-05 | Not run | Controlled setup pending. | — |
| TASK-06 | Not run | Controlled setup pending. | — |
| TASK-07 | Not run | Controlled setup pending. | — |
| TASK-08 | Not run | Controlled setup pending. | — |
| TASK-09 | Not run | Controlled setup pending. | — |
| TASK-10 | Not run | Controlled setup pending. | — |
| TASK-11 | Not run | Controlled setup pending. | — |
| RUN-01 | Not run | Controlled setup pending. | — |
| RUN-02 | Not run | Controlled setup pending. | — |
| RUN-03 | Not run | Controlled setup pending. | — |
| RUN-04 | Not run | Controlled setup pending. | — |
| RUN-05 | Not run | Controlled setup pending. | — |
| RUN-06 | Not run | Controlled setup pending. | — |
| RUN-07 | Not run | Controlled setup pending. | — |
| RUN-08 | Not run | Controlled setup pending. | — |
| RUN-09 | Not run | Controlled setup pending. | — |
| RUN-10 | Not run | Controlled setup pending. | — |
| RUN-11 | Not run | Controlled setup pending. | — |
| RUN-12 | Not run | Controlled setup pending. | — |
| RUN-13 | Not run | Controlled setup pending. | — |
| RUN-14 | Not run | Controlled setup pending. | — |
| RUN-15 | Not run | Controlled setup pending. | — |
| TIME-01 | Not run | Controlled setup pending. | — |
| TIME-02 | Not run | Controlled setup pending. | — |
| TIME-03 | Not run | Controlled setup pending. | — |
| TIME-04 | Not run | Controlled setup pending. | — |
| TIME-05 | Not run | Controlled setup pending. | — |
| TIME-06 | Not run | Controlled setup pending. | — |
| TIME-07 | Not run | Controlled setup pending. | — |
| TIME-08 | Not run | Controlled setup pending. | — |
| TIME-09 | Not run | Controlled setup pending. | — |
| TIME-10 | Not run | Controlled setup pending. | — |
| TIME-11 | Not run | Controlled setup pending. | — |
| TIME-12 | Not run | Controlled setup pending. | — |
| PROJECT-01 | Not run | Controlled setup pending. | — |
| PROJECT-02 | Not run | Controlled setup pending. | — |
| PROJECT-03 | Not run | Controlled setup pending. | — |
| PROJECT-04 | Not run | Controlled setup pending. | — |
| PROJECT-05 | Not run | Controlled setup pending. | — |
| AGENT-01 | Not run | Controlled setup pending. | — |
| AGENT-02 | Not run | Controlled setup pending. | — |
| AGENT-03 | Not run | Controlled setup pending. | — |
| MEM-01 | Not run | Controlled setup pending. | — |
| MEM-02 | Not run | Controlled setup pending. | — |
| MEM-03 | Not run | Controlled setup pending. | — |
| MEM-04 | Not run | Controlled setup pending. | — |
| MEM-05 | Not run | Controlled setup pending. | — |
| MEM-06 | Not run | Controlled setup pending. | — |
| MEM-07 | Not run | Controlled setup pending. | — |
| MEM-08 | Not run | Controlled setup pending. | — |
| MEM-09 | Not run | Controlled setup pending. | — |
| MEM-10 | Not run | Controlled setup pending. | — |
| MEM-11 | Not run | Controlled setup pending. | — |
| MEM-12 | Not run | Controlled setup pending. | — |
| ACTION-01 | Not run | Controlled setup pending. | — |
| ACTION-02 | Not run | Controlled setup pending. | — |
| ACTION-03 | Not run | Controlled setup pending. | — |
| ACTION-04 | Not run | Controlled setup pending. | — |
| ACTION-05 | Not run | Controlled setup pending. | — |
| ACTION-06 | Not run | Controlled setup pending. | — |
| ACTION-07 | Not run | Controlled setup pending. | — |
| ACTION-08 | Not run | Controlled setup pending. | — |
| ACTION-09 | Not run | Controlled setup pending. | — |
| INFO-01 | Not run | Controlled setup pending. | — |
| INFO-02 | Not run | Controlled setup pending. | — |
| INFO-03 | Not run | Controlled setup pending. | — |
| INFO-04 | Not run | Controlled setup pending. | — |
| INFO-05 | Not run | Controlled setup pending. | — |
| INFO-06 | Not run | Controlled setup pending. | — |
| API-01 | Not run | Controlled setup pending. | — |
| API-02 | Not run | Controlled setup pending. | — |
| API-03 | Not run | Controlled setup pending. | Not run |
| API-04 | Not run | Controlled setup pending. | Not run |
| API-05 | Not run | Controlled setup pending. | Not run |
| API-06 | Not run | Controlled setup pending. | Not run |
| API-07 | Not run | Controlled setup pending. | Not run |
| API-08 | Not run | Controlled setup pending. | Not run |
| API-09 | Not run | Controlled setup pending. | — |
| API-10 | Not run | Controlled setup pending. | — |
| API-11 | Not run | Controlled setup pending. | — |
| API-12 | Not run | Controlled setup pending. | — |
| API-13 | Not run | Controlled setup pending. | — |
| API-14 | Not run | Controlled setup pending. | — |
| API-15 | Not run | Controlled setup pending. | — |
| MCP-01 | Not run | Controlled setup pending. | — |
| MCP-02 | Not run | Controlled setup pending. | Not run |
| MCP-03 | Not run | Controlled setup pending. | — |
| MCP-04 | Not run | Controlled setup pending. | — |
| MCP-05 | Not run | Controlled setup pending. | — |
| MCP-06 | Not run | Controlled setup pending. | — |
| MCP-07 | Not run | Controlled setup pending. | — |
| ACP-01 | Not run | Controlled setup pending. | Not run |
| ACP-02 | Not run | Controlled setup pending. | — |
| ACP-03 | Not run | Controlled setup pending. | — |
| ACP-04 | Not run | Controlled setup pending. | — |
| WEB-01 | Not run | Controlled setup pending. | — |
| WEB-02 | Not run | Controlled setup pending. | — |
| WEB-03 | Not run | Controlled setup pending. | — |
| WEB-04 | Not run | Controlled setup pending. | — |
| WEB-05 | Not run | Controlled setup pending. | — |
| WEB-06 | Not run | Controlled setup pending. | — |
| WEB-07 | Not run | Controlled setup pending. | — |
| WEB-08 | Not run | Controlled setup pending. | — |
| WEB-09 | Not run | Controlled setup pending. | — |
| WEB-10 | Not run | Controlled setup pending. | — |
| WEB-11 | Not run | Controlled setup pending. | — |
| WEB-12 | Not run | Controlled setup pending. | — |
| WEB-13 | Not run | Controlled setup pending. | — |
| WEB-14 | Not run | Controlled setup pending. | — |
| WEB-15 | Not run | Controlled setup pending. | — |
| WEB-16 | Not run | Controlled setup pending. | — |
| FILE-01 | Not run | Controlled setup pending. | — |
| FILE-02 | Not run | Controlled setup pending. | — |
| FILE-03 | Not run | Controlled setup pending. | — |
| FILE-04 | Not run | Controlled setup pending. | — |
| FILE-05 | Not run | Controlled setup pending. | — |
| FILE-06 | Not run | Controlled setup pending. | — |
| FILE-07 | Not run | Controlled setup pending. | — |
| FILE-08 | Not run | Controlled setup pending. | — |
| FILE-09 | Not run | Controlled setup pending. | — |
| FILE-10 | Not run | Controlled setup pending. | — |
| CALC-01 | Not run | Controlled setup pending. | — |
| CALC-02 | Not run | Controlled setup pending. | — |
| ART-01 | Not run | Controlled setup pending. | — |
| ART-02 | Not run | Controlled setup pending. | — |
| ART-03 | Not run | Controlled setup pending. | — |
| ART-04 | Not run | Controlled setup pending. | — |
| ART-05 | Not run | Controlled setup pending. | — |
| ART-06 | Not run | Controlled setup pending. | — |
| ART-07 | Not run | Controlled setup pending. | — |
| ART-08 | Not run | Controlled setup pending. | — |
| ART-09 | Not run | Controlled setup pending. | — |
| NOTE-01 | Not run | Controlled setup pending. | Not run |
| NOTE-02 | Not run | Controlled setup pending. | Not run |
| NOTE-03 | Not run | Controlled setup pending. | Not run |
| NOTE-04 | Not run | Controlled setup pending. | Not run |
| NOTE-05 | Not run | Controlled setup pending. | — |
| NOTE-06 | Not run | Controlled setup pending. | — |
| NOTE-07 | Not run | Controlled setup pending. | Not run |
| NOTE-08 | Not run | Controlled setup pending. | — |
| NOTE-09 | Not run | Controlled setup pending. | Not run |
| NOTE-10 | Not run | Controlled setup pending. | Not run |
| NOTE-11 | Not run | Controlled setup pending. | Not run |
| NOTE-12 | Not run | Controlled setup pending. | — |
| CLIENT-01 | Not run | Controlled setup pending. | Not run |
| CLIENT-02 | Not run | Controlled setup pending. | — |
| CLIENT-03 | Not run | Controlled setup pending. | — |
| CLIENT-04 | Not run | Controlled setup pending. | — |
| PWA-01 | Not run | Controlled setup pending. | Not run |
| PWA-02 | Not run | Controlled setup pending. | Not run |
| PWA-03 | Not run | Controlled setup pending. | Not run |
| PWA-04 | Not run | Controlled setup pending. | Not run |
| PWA-05 | Not run | Controlled setup pending. | Not run |
| PWA-06 | Not run | Controlled setup pending. | Not run |
| PWA-07 | Not run | Controlled setup pending. | Not run |
| NATIVE-01 | Not run | Controlled setup pending. | Not run |
| NATIVE-02 | Not run | Controlled setup pending. | Not run |
| NATIVE-03 | Not run | Controlled setup pending. | Not run |
| NATIVE-04 | Not applicable | Physical device journey. | Not run |
| NATIVE-05 | Not run | Controlled setup pending. | Not run |
| NATIVE-06 | Not run | Controlled setup pending. | Not run |
| DESKTOP-01 | Not run | Controlled setup pending. | Not run |
| DESKTOP-02 | Not run | Controlled setup pending. | Not run |
| DESKTOP-03 | Not run | Controlled setup pending. | Not run |
| UX-01 | Not run | Controlled setup pending. | Not run |
| UX-02 | Not run | Controlled setup pending. | Not run |
| UX-03 | Not run | Controlled setup pending. | — |
| UX-04 | Not run | Controlled setup pending. | — |
| UX-05 | Not run | Controlled setup pending. | — |
| DIAG-01 | Not run | Controlled setup pending. | — |
| DIAG-02 | Not run | Controlled setup pending. | — |
| OPS-01 | Not run | Controlled setup pending. | — |
| OPS-02 | Not run | Controlled setup pending. | — |
| OPS-03 | Not run | Controlled setup pending. | — |
| JOURNEY-01 | Not run | Controlled setup pending. | Not run |
| JOURNEY-02 | Not run | Controlled setup pending. | — |
| JOURNEY-03 | Not run | Controlled setup pending. | — |
| JOURNEY-04 | Not run | Controlled setup pending. | — |
| JOURNEY-05 | Not run | Controlled setup pending. | — |
| JOURNEY-06 | Not run | Controlled setup pending. | — |
| JOURNEY-07 | Not run | Controlled setup pending. | — |
| JOURNEY-08 | Not applicable | Physical device journey. | Not run |
| JOURNEY-09 | Not run | Controlled setup pending. | Not run |
| JOURNEY-10 | Not run | Controlled setup pending. | — |
| JOURNEY-11 | Not run | Controlled setup pending. | — |
| JOURNEY-12 | Not run | Controlled setup pending. | — |
