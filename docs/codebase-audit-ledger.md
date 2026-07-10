# Codebase Audit Coverage Ledger

This ledger assigns every checkbox in the
[codebase audit tracker](codebase-audit-tracker.md) to exactly one primary
milestone in the approved
[remediation program](superpowers/plans/2026-07-09-noema-remediation-program.md).
Assignment is ownership, not completion: deferred work remains `Pending` until
its implementation, acceptance evidence, and completion commit exist.

Overlap ownership follows the program boundaries: M1+M2 owns OAuth callback
trust and attempt expiry plus MCP schema and policy; M4 owns OAuth token
durability plus MCP cursor and transport bounds. M3 owns durable interruption
mechanics while M6 renders recovery. M4 owns memory supervisor state while M6
renders it. M7 owns fail-closed asset mechanics while M5 consumes them in
packaging.

| ID | Primary milestone | Known consumers | Automated coverage | Operational verification | Acceptance evidence | Completion commit |
| --- | --- | --- | --- | --- | --- | --- |
| SEC-001 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-002 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-003 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-004 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-005 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-006 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-007 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-008 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-009 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-010 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-011 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-012 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-013 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-014 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-015 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-016 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-017 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-018 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| SEC-019 | M1+M2 | API; daemon; desktop | Pending | Pending | Pending | Pending |
| GOV-001 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-002 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-003 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-004 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-005 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-006 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-007 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-008 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-009 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| GOV-010 | M1+M2 | Capability Gateway; MCP; memory | Pending | Pending | Pending | Pending |
| DATA-001 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-002 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-003 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-004 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-005 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-006 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-007 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-008 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-009 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-010 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-011 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-012 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-013 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-014 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-015 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-016 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-017 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-018 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-019 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-020 | M3 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-021 | M1+M2 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-022 | M4 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-023 | M4 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-024 | M4 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-025 | M4 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| DATA-026 | M6 | Store; runtime; integrations | Pending | Pending | Pending | Pending |
| RUN-001 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-002 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-003 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-004 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-005 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-006 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-007 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-008 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-009 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-010 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-011 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-012 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-013 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-014 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-015 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-016 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-017 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-018 | M4 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-019 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| RUN-020 | M3 | Runtime; daemon; integrations | Pending | Pending | Pending | Pending |
| PERF-001 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-002 | M3 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-003 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-004 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-005 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-006 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-007 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-008 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-009 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-010 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| PERF-011 | M7 | Runtime; API; frontend | Pending | Pending | Pending | Pending |
| UX-001 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-002 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-003 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-004 | M4 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-005 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-006 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-007 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-008 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| UX-009 | M6 | Web app; desktop | Pending | Pending | Pending | Pending |
| A11Y-001 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| A11Y-002 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| A11Y-003 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| A11Y-004 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| A11Y-005 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| A11Y-006 | M6 | Web app; memory views | Pending | Pending | Pending | Pending |
| REL-001 | M7 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-002 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-003 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-004 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-005 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-006 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-007 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-008 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| REL-009 | M5 | Desktop; release pipeline | Pending | Pending | Pending | Pending |
| ARCH-001 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-002 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-003 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-004 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-005 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-006 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-007 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-008 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-009 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-010 | M1+M2 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-011 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-012 | M4 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-013 | M4 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-014 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-015 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-016 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-017 | Foundation | Workspace; maintainers | Baseline capture tests and recorded timing artifacts | Clean, no-op, and representative build timing samples recorded without altering sccache | `docs/engineering/baselines/foundation.md` and `.json` | `7c92c0f2` |
| ARCH-018 | Foundation | Workspace; maintainers | Baseline capture tests and recorded timing artifacts | Representative GraphQL, store, provider, and frontend-asset edit timings recorded | `docs/engineering/baselines/foundation.md` and `.json` | `7c92c0f2` |
| ARCH-019 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-020 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-021 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-022 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-023 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-024 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-025 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-026 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-027 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-028 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-029 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-030 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-031 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-032 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-033 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-034 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| ARCH-035 | M7 | Workspace; maintainers | Pending | Pending | Pending | Pending |
| TEST-001 | M8 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-002 | Foundation | CI; maintainers | `bun run test:ci` package script | Existing focused frontend tests run through normal package validation | 63 focused tests, lint, and build passed | `f89be670` |
| TEST-003 | Foundation | CI; maintainers | Shell navigation unit tests | Memory and Usage settings expectations match canonical routes | Corrected navigation suite passed | `f89be670` |
| TEST-004 | Foundation | CI; maintainers | Default-parallel Rust workspace CI plus Foundation bridge process tests | 11 Foundation process tests passed five consecutive 16-thread runs | 55/55 repeated process-test executions passed on 2026-07-10 | `6867a287` |
| TEST-005 | M3 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-006 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-007 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-008 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-009 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-010 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-011 | M4 | MCP schema/policy (M1+M2); MCP transport (M4) | Pending | Pending | Pending | Pending |
| TEST-012 | M1+M2 | CI; maintainers | Pending | Pending | Pending | Pending |
| TEST-013 | Foundation | CI; maintainers | Generated-state verifier and CI clean-diff gate | GraphQL schema/types and route tree regenerate before `git diff --exit-code` | 28 verifier tests and real generated/import-graph/asset checks passed | `6867a287` |
| TEST-014 | M5 | CI; maintainers | Pending | Pending | Pending | Pending |
| DOC-001 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-002 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-003 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-004 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-005 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-006 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-007 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-008 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-009 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-010 | M8 | Contributors; release | Pending | Pending | Pending | Pending |
| DOC-011 | Foundation | Contributors; release | Exact MIT text and cited-path checks | Root 21-line MIT license and provisional redistribution inventory present | `LICENSE`, `docs/licensing.md`, and `THIRD_PARTY_NOTICES.md` | `582af72a` |
| DOC-012 | M5 | Contributors; release | Pending | Pending | Pending | Pending |
| MILESTONE-001 | M1+M2 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-002 | M1+M2 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-003 | M3 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-004 | M4 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-005 | M5 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-006 | M6 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-007 | M7 | Remediation program | Pending | Pending | Pending | Pending |
| MILESTONE-008 | M8 | Remediation program | Pending | Pending | Pending | Pending |
