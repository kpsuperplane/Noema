# Cruft Audit Full Campaign Decisions

These decisions resolve the open questions needed before attempting the full
low-, medium-, and high-risk cruft audit campaign.

Date: 2026-07-02

## Execution Shape

- Execute as a long subagent-driven campaign on `main`.
- Do not create a dedicated branch or worktree.
- Use focused validation per task and milestone validation after meaningful
  groups of changes.
- Commit after each completed unit of work.

## Ordering

- Resolve high-risk decisions before cleanup that depends on those boundaries.
- Then execute cleanup/refactor work with those decisions treated as settled
  campaign constraints.

## Provider And Memory Semantics

- Prefer minimal functionality changes while cleaning up.
- Long-term direction: memory operations should have their own provider setting.
- For this campaign, do not add a broad new memory-provider feature unless it is
  required to avoid an existing privacy/correctness bug.
- Model override without provider override should not be possible. A model
  override must be attached to an explicit provider override.

## Capability Gateway

- Delete staged capability policy, quarantine examination, and owner-extraction
  code instead of wiring it into the live gateway in this campaign.

## Compatibility

- No backwards-compatibility requirement for pre-V1 APIs.
- Remove old public Rust surfaces when no in-repo caller remains and the audit
  verified they are legacy.

## GraphQL Interfaces

- Aim for clean interfaces.
- Remove or replace stale GraphQL fields even if that requires frontend or CLI
  type churn.

## Foundation Local Bridge

- Support replay, cancel, close-session, and shutdown lifecycle APIs rather than
  cutting the protocol surface.

## Web Asset Packaging

- Implement a clean generated-asset flow instead of continuing to track generated
  web bundles under Rust source.

## Placeholder Surfaces

- Remove reachable placeholders that are not backed by data.
- Delete the CLI and raw daemon socket surface entirely.

## Evidence Vocabulary

- Trim evidence authorities to currently supported values.

## Validation

- Run focused validation per task.
- Run milestone validation after related groups of changes.
