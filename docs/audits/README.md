# Codebase audits

This directory contains dated, evidence-backed audits of the current repository.
An audit is a snapshot, not a second architecture authority. Current product and
engineering contracts remain in the closest subsystem document, while accepted
cleanup work should be tracked in an implementation plan only for as long as that
work is active.

## Current audit

- [Overengineering audit](2026-08-08-overengineering-audit.md) — assessment,
  ranked findings, decision gates, and protected complexity.
- [Evidence appendix](2026-08-08-overengineering-evidence.md) — baseline,
  measurements, inspected surfaces, and finding-level evidence.
- [Reduction roadmap](2026-08-08-overengineering-roadmap.md) — independently
  shippable reduction slices, budgets, tests, and stop conditions.
- [Remediation report](2026-08-08-overengineering-remediation.md) — durable
  finding-by-finding dispositions, measured reductions, validation results,
  blockers, retained complexity, and open product decisions.

The 2026-08-08 audit superseded the removed codebase audit tracker as the current
codebase-reduction assessment. It does not silently close or implement items in
that older tracker; retiring the tracker is itself a documented cleanup action.

## Current UX audit

- [Web UX audit](2026-08-10-web-ux-audit.md) — full static review across
  accessibility, layout, writing, typography, color, and interface behavior.

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
