# Codebase audits

This directory contains dated, evidence-backed snapshots.
An audit does not define current product behavior.
Use the closest subsystem document and current code for present contracts.

## Documentation

- [Documentation audit](2026-08-16-documentation-audit.md) — complete Markdown
  disposition at its stated repository baseline.

## Engineering snapshots from 2026-08-08

- [Overengineering audit](2026-08-08-overengineering-audit.md) — assessment,
  ranked findings, decision gates, and protected complexity.
- [Evidence appendix](2026-08-08-overengineering-evidence.md) — baseline,
  measurements, inspected surfaces, and finding-level evidence.
- [Remediation report](2026-08-08-overengineering-remediation.md) — completion
  ledger, measured reductions, validation results, and unresolved decisions.

The remediation report replaces the deleted execution roadmap.
Git history preserves the completed plan.

## Experience snapshots

- [Codex conversation retrospective](2026-08-08-codex-conversation-retrospective.md)
  — private-source-safe evidence from the stated conversation window.
- [`.noema-dev` experience history](2026-08-08-noema-dev-user-experience-history.md)
  — incident evidence through its stated runtime cutoff.
- [Current-build disposition](2026-08-08-current-build-ux-survivors.md) — point-in-time
  recheck at source revision `be388c1e`.
- [Web UX audit](2026-08-10-web-ux-audit.md) — closed static review and
  implementation record.
- [Tool marker audit](2026-08-19-tool-marker-audit.md) — complete saved-call
  inventory, current marker behavior, and user-focused display recommendations.

The Web audit did not include browser, device, or assistive-technology checks.
Run a fresh review before treating an old finding as present behavior.

## Audit conventions

- **P0** — current correctness, data, or policy failure; repair before ordinary
  cleanup.
- **P1** — high-confidence removal or authority correction with current evidence.
- **P2** — bounded consolidation that should be independently net-negative.
- **P3** — opportunistic hygiene; perform only alongside nearby work.
- **Decision gate** — potentially large reduction that changes a stated product
  contract or supported capability and therefore needs product direction.

Line-count estimates are directional. Generated output, historical documentation,
tests, and production code are reported separately. A large file or subsystem is
never treated as overengineering merely because it is large.
