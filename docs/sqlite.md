# Historical SQLite Schema

Noema previously used a local SQLite schema for structured state. The current
architecture uses Postgres as the canonical structured store for the always-on,
self-hosted personal server.

See [Canonical Postgres Schema](postgres.md) for the implemented schema and
storage split.
