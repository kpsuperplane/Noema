# Bundled Supermemory

Release packaging places the platform `supermemory-server` executable in this
directory or in the equivalent app resource directory selected by the runtime
bundle.

The managed Supermemory resolver checks candidates in this order:

1. `NOEMA_SUPERMEMORY_SERVER`
2. this bundled resource directory
3. `supermemory-server` on `PATH`

Managed mode binds the server to a runtime-selected loopback port and keeps that
endpoint private to the Noema runtime host. SQLite stores readiness status and
diagnostics, not the managed endpoint.
