# Development build memory limits

## Cause

At 20:56:03 UTC, Linux reported a host memory shortage.
The killed Node process used about 1.31 GiB of resident anonymous memory.
A second large Node process used about 1.01 GiB.
Several Go compiler processes together used about 2.3 GiB.
The development session stopped, and public requests received connection failures.
The host has 7.6 GiB of RAM. Its 4 GiB swap area was full after the failure.

## Change

`scripts/with-build-limits` applies the shared build limits.
Go builds compile one package at a time, with two active CPUs and a 512 MiB soft memory limit.
Node uses a 1.5 GiB old-generation heap limit.
On the Linux root host, build processes share a systemd slice.
The final slowdown threshold is 3.5 GiB. The hard memory ceiling is 4 GiB.
Build swap use cannot exceed 512 MiB.
The initial 3 GiB slowdown threshold caused excessive waiting during concurrent web builds.
A 1 GiB Node heap was too small for the application watcher. The final heap allowance is 1.5 GiB.
The server and inspection connections remain outside the build slice.

The launcher build, server build scripts, and web commands apply these limits.
Manual Go checks must use the wrapper, as specified in `AGENTS.md`.
Commands that bypass the wrapper remain outside this budget.

## Checks

The tested base was `724690a4`, with this task's script and command changes.
No Go application source changed.

- `scripts/test-build-limits` passed with the final limits.
  It checks exit status, argument and directory preservation, compiler settings, the actual Node heap allowance, and nested slice membership.
- Shell syntax checks passed for the changed launcher and build scripts.
- `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...` passed.
- `CGO_ENABLED=0 scripts/with-build-limits go vet ./cmd/... ./internal/...` passed.
  These Go results used the initial, stricter 3 GiB slowdown threshold.
  They remain valid because the later threshold change does not change Go code or test behavior.
- `scripts/run-noema-dev-server build` passed.
- A temporary child scope had a 64 MiB memory limit and no swap allowance.
  A 128 MiB allocation exited with status 137. Systemd reported `oom-kill` for that child scope.
  The authenticated server connection still returned `{"state":"authenticated"}`.
  The shared slice's recorded memory-kill count includes this deliberate check.
- `bun run build` passed with the final limits and automatic web rebuilding temporarily paused.
  The development supervisor resumed after the check.
- `scripts/with-build-limits node node_modules/eslint/bin/eslint.js src --max-warnings=0` passed from `apps/web`.
  The earlier concurrent ESLint check was stopped to reduce memory pressure.
- `bun run lint` reached an existing TypeScript error in `TaskModelPoolsSettings.tsx`.
  Its save request omits the required `TaskModelPoolEntryInput.enabled` field.
  The same omission exists at the tested base. This task does not change that component.

The patch adds no Go source or generated GraphQL changes.

Concurrent build memory peaked at 4,123,906,048 bytes, below the 4 GiB ceiling.
The deliberate small-scope allocation was the only recorded Linux memory kill during validation.
The application watcher separately rejected the initial 1 GiB JavaScript heap allowance.
Authenticated server access remained available through both failures.

A concurrent task committed `d638c140`, which changes tool status styling.
The final web build and ESLint check include that change.
No changes from that task are included in this patch.

The patch adds 20 wrapper lines and 30 shell check lines.
It changes existing build commands and development instructions.
There are no Go production, Go test, or generated GraphQL line changes.

Both automatic web rebuilds passed after the final restart.
The GraphiQL rebuild took 25.1 seconds. The application rebuild took 32.8 seconds.
Both processes inherited the 1.5 GiB heap setting and belonged to the shared build slice.
The authenticated server connection remained available.
