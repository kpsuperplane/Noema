# Obscura worker

The Go server owns browser sessions, authorization, routing, and process limits.
This worker owns Obscura execution and the bounded version 1 line protocol.
It preserves URL checks, trusted interactions, screenshots, and exact page revisions.

Normal Go use downloads pinned Noema worker releases.
Cargo is needed only to build those releases or change worker source.

Build with `cargo build -p noema-obscura-worker`.
Use the release workflow to package the worker for each supported native target.
The worker accepts `--noema-browser-worker-v1 <heap-mb> <generation>`.
Go passes commands over stdin and receives bounded JSON lines over stdout.

Normal workspace tests run unit tests only.
The retained embedded-browser fixture uses the `browser-fixtures` feature.
Enable that feature only when fixture execution is requested.
