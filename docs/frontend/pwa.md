# Installed Web Application

Noema's browser build is an installable, read-only-offline PWA. It is enabled only when the browser reports standalone display mode, so Safari tabs and the Tauri desktop shell continue to use their existing network and in-memory behavior.

## Release and update contract

Vite gives every JavaScript and CSS file a content hash. Workbox generates `/assets/sw.js` at one stable URL and precaches the HTML shell, every eager and lazy code/CSS chunk, the manifest, and all icons. Each build receives a distinct Noema cache namespace, so an installing worker cannot modify resources used by the active release; activation cleans the older precache only after the replacement is complete. The server serves the worker, HTML, manifest, and stable image names with `no-cache`; hashed code is immutable for one year. Only the worker receives `Service-Worker-Allowed: /`.

The worker does not cache runtime traffic. GraphQL, authentication, callbacks, artifact downloads, and external services remain network-only.
Noema marks its private responses `no-store`; it cannot set cache headers for external services.

After authentication, on an `online` event, and whenever the standalone app returns to the foreground, the current app asks the browser to update the worker with HTTP-cache bypass. Returning while already online verifies the browser session and performs that check in the background without entering recovery or locking writes. A new worker remains waiting until its entire precache succeeds. Noema then waits for mutations, an active chat turn, and passkey/provider-auth ceremonies to finish, locks new writes, flushes drafts and the Apollo snapshot, sends `SKIP_WAITING`, awaits `controllerchange`, and reloads the current URL programmatically. The URL, search parameters, IndexedDB drafts, and durable Apollo state survive this refresh; the human does not need an iPhone reload control.

An install or update failure leaves the current controller and snapshot in place and retries on a later online/foreground transition. A failed update check does not prevent recovery from continuing with the active release.
Writes remain blocked until data reconciliation completes. An operational rollback must still publish one final worker at `/assets/sw.js`; that worker must delete caches whose names begin with Noema's Workbox cache prefix, unregister itself, and reload controlled clients. Removing the worker URL before that cleanup release strands installed clients.

## Offline storage and privacy

Installed mode uses the versioned `noema-pwa` IndexedDB database. One `snapshot` record contains Apollo's normalized cache, the cache schema version, recovery generation, last successful synchronization time, and at most 100 recent mutable query/variable pairs. Replacing that record is one IndexedDB transaction, so termination leaves either the previous complete generation or the replacement. Chat drafts and the task-capture draft are separate durable records because they must be saved between synchronized snapshots.

Noema requests persistent browser storage after authentication. Denial and
quota exhaustion are nonfatal. The server remains the source authority. The
installed app uses isolated origin storage and the iPhone device lock as its
offline access boundary. Noema does not add application-level encryption to
the Apollo snapshot.

A previously authenticated sentinel permits an installed app to reveal its saved snapshot when the server is unreachable. A reachable server response requiring login immediately hides that snapshot behind the passkey gate without deleting it. A successful login makes it available for reconciliation again.

## Recovery state machine

- `offline` exposes saved reads and editable drafts while every mutation is blocked.
- `checking` verifies `/auth/status` from the network and completes the application update check while writes remain blocked.
- `reconciling` pauses durable snapshot writes, waits for the GraphQL subscription acknowledgement, refreshes Chat boot and the latest transcript, refetches mounted queries, and revalidates remembered mutable reads with at most four concurrent requests.
- `online` begins only after the reconciled Apollo cache and metadata have committed atomically.
- `auth_required` hides cached private data behind the existing passkey surface.

The shell presents reconciliation as a compact neutral `Syncing` status rather than a warning; offline, authentication, and release-update states retain their stronger treatment.

Chat uses its normal composer presentation during a cold launch instead of exposing a transient startup label. An iOS background suspension may reconnect the GraphQL WebSocket while the PWA remains `online`; an established conversation keeps that composer available for drafting during the reconnect, while sending waits for the socket acknowledgement. After acknowledgement, Chat remains usable while it backfills the durable transcript and refetches active queries without entering offline recovery or locking unrelated writes.

Each network request captures the current recovery generation. Results from an older query generation are ignored. A transport/auth failure restores the in-memory baseline and retains the previous durable snapshot. A GraphQL error from one remembered query keeps that cached object stale and does not prevent unrelated reads from reconciling.

Mutation responses, immutable task/run history, debug profiles, OAuth/setup state, arbitrary artifact downloads, approvals, revision-fenced actions, and other commands are never queued or replayed. Offline agent execution and native iOS behavior are separate product concerns.

## Notifications

Web Push is available only to an installed standalone browser app served from the configured HTTPS `web.public_origin`. Permission is requested only from the Enable action in Chat or Settings. The browser subscription is registered through GraphQL; the server keeps its VAPID private key in the protected `notifications/web-push-vapid.json` file.
SQLite stores browser subscriptions, notification progress, and the delivery queue. A 404 or 410 response removes the expired subscription. Network, 429, and server failures retry after 1, 5, and 30 minutes before becoming terminal.

Noema notifies for durable `final_answer` items in the local human's primary
conversation. It also notifies for new `HumanIntervention` projections.
Existing items seed the projector without an alert. Chat messages use normal
urgency and a one-hour TTL. Interventions use high urgency and a one-day TTL.
Payloads contain the agent or safe intervention title, a normalized preview,
and navigation to `/`. They exclude arguments, results, action details, badges,
and notification actions.

Suppression is per browser subscription. A focused primary Chat route holds a
visibility lease for that installed client. Noema suppresses new deliveries for
that exact subscription. Other devices still receive the notification. Tasks,
Settings, backgrounded, offline, and disconnected clients do not assert Chat
visibility. This behavior avoids an alert where the human reads the message.
It does not silence the human's other clients.

The daemon always sends the declarative Web Push JSON shape so current WebKit can display and navigate even if the service worker has been removed or fails. The generated worker imports a small fallback handler that renders the same payload and handles clicks on browsers without declarative support. Notification content crosses the browser vendor's push service encrypted by standard Web Push; device-level preview controls remain authoritative.

## Release validation

[scripts/build-go-server](../../scripts/build-go-server) builds web assets, checks required entry files, and embeds them in the Go release.
[Vite configuration](../../apps/web/vite.config.ts) owns the worker and precache definition.
[Asset handling](../../internal/web/assets.go) owns server cache headers and worker scope.
Build success does not prove offline updates work on a physical device.

The manual iPhone check installs from authenticated Safari. It opens Chat,
Tasks, Memory, Settings, and textual artifact data before an Airplane Mode
relaunch. A direct tap must enable notifications. Focused Chat must suppress
only that iPhone while another client still alerts. Background Chat, Settings,
and a closed app must receive a final reply and a new intervention.

Deploy a second release while version one is offline. Reconnect without
terminating the app. The app must install and activate the complete worker,
then refresh automatically. It must preserve the route and drafts. Stored data
must advance atomically before any write control becomes available.
