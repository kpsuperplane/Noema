# Runtime and Governance Contract Index

The implementation and the focused documents below are the current authorities.
Historical architecture plans live in Git history, not in the working tree.

- [Security and information handling](harness/security.md)
- [Capability boundaries](harness/capabilities.md)
- [Governed actions and approvals](harness/action-governance.md)
- [Memory context](harness/memory-context.md)
- [Web browsing](harness/web-browsing.md)

## Runtime boundaries

- SQLite is the canonical structured store. Concrete rows and forward-only
  migrations own durable state.
- Conversation items own durable transcript history. Live runtime and
  subscription state coordinate work but do not replace persisted truth.
- Work commands mutate current state transactionally and append events for
  audit and invalidation; events are not a second state-reconstruction system.
- `noema-host` composes services, `noema-runtime` executes conversations and
  Work, `noema-api` owns GraphQL, and shells remain thin.
- Provider, MCP, native adapter, artifact, and memory boundaries keep their own
  transport and persistence contracts. A new universal abstraction requires
  multiple concrete production consumers and a net simplification.

## Change discipline

Use [development/simplicity.md](development/simplicity.md) for budgets and
validation. Durable subsystem decisions belong in the closest focused document;
completed execution history belongs in Git.
