# Codex development permissions

The repository selects `noema-build` in [`.codex/config.toml`](../../.codex/config.toml).
This profile grants full local filesystem and direct network access.
It supports building, starting, and repairing the complete development instance.
Use [Noema Development Access](../../AGENTS.md#noema-development-access) for the connection procedure.

## Access

The Linux root launcher needs capabilities for bindfs mounts, service ownership, and switching to the service account.
Restricted Codex profiles remove those capabilities. Adding writable directories alone does not restore them.
`noema-build` therefore grants `:root` write access. This is full local access, including paths outside the repository.
The server still runs as `noema-dev`. Its normal application authentication and service-account separation remain in place.
Keep credential values in protected stores and secure bindings. Do not print them into model context or logs.

The Codex network proxy is disabled for this project. Direct networking permits Unix sockets and local test listeners.
See the [official permission reference](https://learn.chatgpt.com/docs/permissions) for filesystem and network rules.
The installed CLI was also tested directly because filesystem restrictions affect Linux process capabilities.

## Complete home inspection

The source home belongs to the `noema-dev` service account.
The full-access profile can read the source directly. Prefer the read-only inspection view for ordinary inspection.

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

Start the development session with `./attach` using `noema-build`.
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

The saved profile was checked with `codex sandbox -C /root/noema -P noema-build` on September 9, 2026.
The previous profile had an empty effective capability set. It could not control the host service manager.
The revised profile retains the host capability set and can query the service manager.
A temporary write in `/run/noema-dev` succeeded. The check file was removed.
The complete `./attach` launcher started under the saved profile.
It rebuilt `/run/noema-dev/noema`, mounted the inspection view, and returned authenticated socket status.
The read-only bindfs view still rejects writes independently of the profile.

## General CLI

Use `noema --socket /tmp/noema-codex/graphql.sock` with the existing development relay.
For example, `noema --socket /tmp/noema-codex/graphql.sock status` reads server status.
The CLI also works with normal Linux and macOS installations.
See [Noema CLI](../cli.md) for commands and socket selection.
The CLI does not use the browser inspection HTTP credential.

## Frontend build output

On Linux as root, normal frontend builds write to `/run/noema-dev/web-assets`.
The development launcher serves this same directory. Builds fail if it is not writable.
They do not fall back to a separate preview directory.
Other development environments use `target/web-assets`.
Both the app and GraphiQL use the same output selection and readable file permissions.
The release packaging script explicitly selects `target/web-assets` before packaging those files.
`NOEMA_DEV_ASSET_DIR` remains an explicit output override for isolated builds.

After frontend changes, inspect the served page through the private socket.
A preview that substitutes local assets does not verify the running app.

## Build memory limits

Use `scripts/with-build-limits <command> [arguments]` for manual builds and checks.
For example, run `CGO_ENABLED=0 scripts/with-build-limits go test ./cmd/... ./internal/...`.
The launcher, server build scripts, and web build commands use this wrapper.
Do not wrap the application server or the complete development supervisor.

Go compiles one package at a time and runs Go code on at most two CPUs.
Each Go build process has a 512 MiB soft memory limit.
Each Node process has a 1.5 GiB old-generation JavaScript heap limit.
These runtime settings encourage earlier memory collection. They do not cap total process memory.
See [Go build concurrency](https://go.dev/cmd/go/) and [Node memory controls](https://nodejs.org/api/cli.html#--max-old-space-sizesize-in-mib).

On the Linux root development host, wrapped builds share `noema-build.slice`.
Linux slows memory allocation above 3.5 GiB and enforces a 4 GiB memory ceiling.
The slice also limits swap use to 512 MiB.
The wrapper applies these settings before starting a build.
Nested commands retain the same slice.
The server and inspection connections stay outside this slice.
If a build exhausts its allowance, it can fail without exhausting host memory.
See [systemd memory controls](https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html).

Other users and operating systems receive the compiler and heap settings only.
Commands that bypass the wrapper do not share its Linux memory ceiling.
Use `systemctl show noema-build.slice -p MemoryCurrent -p MemoryPeak` to inspect build memory.
