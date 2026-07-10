# SQLite reset protocol

Noema's pre-V1 SQLite schema is reset-only. If startup reports `ResetRequired`,
stop every Noema process and any other database writer. Move the database file
and its `-wal` and `-shm` sidecars together to a private backup location, then
start Noema to create a new database.

Do not delete or move the `providers/` directory, configuration, MCP credentials,
or other secrets. Noema never performs this reset automatically.
