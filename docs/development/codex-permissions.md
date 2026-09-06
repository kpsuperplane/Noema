# Codex development permissions

The repository selects `noema-build` in [`.codex/config.toml`](../../.codex/config.toml).
This profile extends `:workspace` and retains filesystem limits.
Use [Noema Development Access](../../AGENTS.md#noema-development-access) for the agent connection procedure.

## Access

- `.git` permits local Git changes and commits.
- `.agents` and `.codex` permit merges that update tracked instructions and project settings.
- Go build and module caches permit builds and dependency downloads.
- The shared sccache directory retains its existing access.
- `/var/tmp` permits temporary browser installations and inspection files.
- Direct networking permits Unix sockets, local services, and test listeners.
- `/tmp/noema-codex/home` provides read-only access to the complete development home.

The Codex network proxy is disabled for this project.
On Linux, Codex 0.153.2 blocks Unix socket creation when that proxy is active.
A socket allow entry does not fix this implementation limit.
Network access therefore has no domain filter, including access to local services.
Global Codex settings remain read-only. Project instructions and project settings are writable for repository work.
The [official permission reference](https://learn.chatgpt.com/docs/permissions) explains filesystem rules and the separate network proxy control.

## Complete home inspection

The source home belongs to the `noema-dev` service account.
Its private file permissions block direct reads after the Codex sandbox drops root capabilities.
Adding a Codex read rule does not override those operating-system permissions.

The Linux root launcher uses `bindfs` to expose the source at `/tmp/noema-codex/home`.
This is a live, read-only filesystem view. It does not copy files or change source ownership and permissions.
It covers the entire `NOEMA_HOME`, including new files, databases, memory, artifacts, configuration, and protected stores.
The source defaults to `/var/lib/noema-dev`; the launcher honors an explicit `NOEMA_HOME`.
Ordinary file tools and read-only SQLite queries work through this view.
Keep actual credential values inside their protected stores and secure bindings. Do not print them into model context or logs.

Install `bindfs` before starting the Linux root development launcher:

```sh
apt-get install bindfs
```

Start the development session with `./attach` from the host terminal.
The launcher mounts the view and starts both inspection relays.
Normal shutdown unmounts the view and removes the socket and inspection credential.
The root-owned parent directory permits access only to root.
The view also rejects root writes. It adds no access for other host users.
See the [bindfs manual](https://bindfs.org/docs/bindfs.1.html) for ownership mapping and read-only mounts.

Use the private relay socket for browser requests. Socket identity does not transfer through the filesystem view.
See [browser inspection](../frontend/browser-inspection.md) for Playwright access.
Running `go run ./cmd/noema-dev` directly does not start the relays or mount the home view.
A stale socket file does not prove that the server is running.

## Apply changes

Select `noema-build` in Codex. If an existing session retains old restrictions, start a new session in this repository.
Use the saved profile directly when checking access:

```sh
codex sandbox -C /root/noema -P noema-build -- git status --short --branch
codex sandbox -C /root/noema -P noema-build -- ls /tmp/noema-codex/home
codex sandbox -C /root/noema -P noema-build -- curl --unix-socket /tmp/noema-codex/graphql.sock http://localhost/auth/status
```

## Evidence

Recent Noema sessions showed blocked Git merges, Go cache writes, Unix socket requests, and socket tests.
No raw session transcripts are included here.
Checks on 2026-09-06 used the saved `noema-build` profile, not unrestricted shell access.

| Check | Result |
| --- | --- |
| Temporary writes in project settings, instructions, Git, caches, and `/var/tmp` | Passed |
| Git staging through a temporary index; real index unchanged | Passed |
| `git ls-remote origin HEAD` | Passed |
| `sccache --show-stats` | Passed |
| Unix socket creation, connection, and data transfer | Passed |
| `CGO_ENABLED=0 go test ./internal/web -run TestLocal -v` | Passed; private socket creation and removal |
| `node --test scripts/noema-inspection-relay.test.mjs` | Passed; admission, forwarding, credential handling, and shutdown |
| Complete home traversal and full file reads | Passed; 210 directories and 151 regular files, with no contents printed |
| Read-only SQLite schema query through the view | Passed |
| Writes to existing and new files through the view | Denied; source content, ownership, and permissions unchanged |
| New service-owned files with mode `0600` | Readable through the existing view |
| Launcher shutdown | Removed view, socket, credential, and relay directory |
| Global Codex configuration write | Denied |
| Live browser HTTP, GraphQL query, and WebSocket acknowledgement | Passed |
| Browser mutation attempt | Denied by the inspection helper |
| Agent models page at 1440px and 390px | Rendered and visually inspected |

Launcher lifecycle checks used an isolated temporary home and relay directory.
The final launcher uses `umount`; the FUSE helper failed to unmount the isolated view.
Focused unit checks passed with the final profile and its read-only home rule.
The Go invocation reused its successful cache result; the package code was unchanged.
Final checks cover this patch on top of `740f95c0`.
Browser checks remain valid after the launcher-only cleanup correction. Source UI and browser routing did not change.
Final home reads, SQLite queries, and socket access also passed after restarting the corrected launcher.
No application build suite is required for these configuration, launcher, and documentation changes.

## General CLI

Use `noema --socket /tmp/noema-codex/graphql.sock` with the existing development relay.
For example, `noema --socket /tmp/noema-codex/graphql.sock status` reads server status.
The CLI also works with normal Linux and macOS installations.
See [Noema CLI](../cli.md) for commands and socket selection.
The CLI does not use the browser inspection HTTP credential.
