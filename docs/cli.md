# Noema CLI

The `noema` executable runs the server and provides local client commands.
Client commands use the running server's private Unix socket. They need no login or token.
Linux and macOS enable this socket by default. Windows does not support local client commands.

## Connection

Run commands as the Noema service account, or another account with access to its private socket.
The socket has mode `0600`. Its parent directory has mode `0700`.
Public web authentication remains separate.

The client selects its socket in this order:

1. `--socket PATH`
2. `NOEMA_SOCKET`
3. `${NOEMA_HOME}/run/graphql.sock`

Without `NOEMA_HOME`, Noema uses `~/.noema`.
Client commands do not create this directory, open the database, or start the server.

```sh
noema status
NOEMA_HOME=/var/lib/noema noema tasks list
noema --socket /tmp/noema-codex/graphql.sock status
```

Set `web.local_graphql_socket: false` in `config.yaml` to disable the socket.
The environment override is `NOEMA_WEB__LOCAL_GRAPHQL_SOCKET=false`.
An explicit disable setting remains effective after an upgrade.
Restart the server after changing this setting.

`--timeout` defaults to `30s`. It bounds requests, connection setup, and Chat subscription readiness.
An established stream continues until completion, failure, or interruption.

## Output and errors

Commands print JSON to stdout. Streams print one JSON object per line.
GraphQL results retain their `data`, `errors`, and `extensions` fields when present.
`schema` prints GraphQL source text. Help and completion commands use their standard text formats.

Diagnostics go to stderr. Exit codes are:

| Code | Meaning |
| --- | --- |
| `0` | Success, including Chat that requires human input |
| `1` | Connection, server, or Chat failure |
| `2` | Invalid command arguments |
| `130` | Interrupted command |

GraphQL errors produce a failure exit code even when the response contains partial data.
The client preserves that partial data in stdout.
Commands never automatically repeat writes or reconnect streams.
After a disconnect, read the current state before repeating a write.
Interrupting a client command does not cancel the server's Task or Chat turn.

## General API

```sh
noema schema
noema api '{ localStatus { primaryAgentDisplayName } }'
noema api --file query.graphql --variables '{"id":"task:example"}'
noema api --file query.graphql --variables-file variables.json --operation-name Inspect
noema api --file - < query.graphql
noema api --subscribe --file subscription.graphql
```

Use one document argument or `--file`. A file value of `-` reads stdin.
Variables must be one JSON object. Only one input can read stdin.
Local GraphQL POST requests have a 64 KiB limit, including JSON encoding.
Input files also have a 64 KiB limit.

Queries, mutations, and subscriptions use the existing server API.
Browser-session operations retain their existing restrictions.
This CLI does not add artifact downloads, account login, or remote network access.

## Tasks

```sh
noema tasks list --scope ALL --first 25
noema tasks list --project project:example --status HUMAN_GATE
noema tasks list --after CURSOR
noema tasks get task:example
noema tasks create "Prepare the report" --file TASK.md
noema tasks run task:example
noema tasks cancel task:example --reason "No longer needed"
noema tasks watch task:example --after EVENT_CURSOR
```

Task creation captures an Inbox Task. Use `tasks run` to authorize execution.
A scheduled Task uses the server's existing run-now action.
Other Tasks use the existing queue action when available.

List and create commands default to `workspace:personal` and accept `--workspace`.
List commands fetch one page. They preserve the server's next-page cursor.
The scope values are `ACTIVE`, `TERMINAL`, and `ALL`.
The `--status` flag accepts workflow behavior values from the schema.

Run and cancel commands read the current revision and execution generation before writing.
If the Task changes, the server rejects the stale write.
The CLI reports that conflict without retrying.

Task mutations generate a request ID. Use `--request-id` to supply an explicit value.
The existing server rules control request reuse.
Use the general API for Task actions without a named command.

## Chat

```sh
noema chat read --limit 40
noema chat read --cursor CURSOR
noema chat send "Summarize my open Tasks"
noema chat send --file message.txt --time-zone Europe/Paris
noema chat send "Create a research Task" --no-wait
noema chat --conversation conversation:example watch
```

Chat commands default to the primary conversation.
Reads never create a conversation. A send creates the primary conversation when necessary.
The existing API does not provide a general conversation list.

`chat send` waits for subscription readiness before sending the message.
It prints acceptance, then conversation events, and stops at the matching turn's completion.
A new pending human request produces a final `needs_input` object with request details.
Previously pending requests do not stop the stream.
Conversation events can include other activity; completion uses the sent message's correlation ID.

`--no-wait` returns acceptance without subscribing.
`--request-id` supplies the client message correlation ID. It does not guarantee duplicate-message prevention.
Use `chat read` to recover saved output after a disconnect.
Use `chat watch` to observe future events.

## Server and help

```sh
noema serve --listen 127.0.0.1:3737
noema --help
noema tasks --help
noema completion --help
```

Running `noema` without a command still starts the server.
Existing desktop and internal worker entrypoints remain available.
The CLI uses `urfave/cli/v3` for commands, flags, help, and shell completion.
