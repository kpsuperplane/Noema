# Installed Web Application

Noema's browser build is an installable, read-only-offline PWA. It is enabled only when the browser reports standalone display mode, so Safari tabs and the Tauri desktop shell continue to use their existing network and in-memory behavior.

## Release and update contract

Vite gives every JavaScript and CSS file a content hash. Workbox generates `/assets/sw.js` at one stable URL and precaches the HTML shell, every eager and lazy code/CSS chunk, the manifest, and all icons. Each build receives a distinct Noema cache namespace, so an installing worker cannot modify resources used by the active release; activation cleans the older precache only after the replacement is complete. The server serves the worker, HTML, manifest, and stable image names with `no-cache`; hashed code is immutable for one year. Only the worker receives `Service-Worker-Allowed: /`.

The worker does not cache runtime traffic. GraphQL HTTP and WebSocket requests, authentication and bootstrap URLs, OAuth callbacks, private artifact downloads, and external origins remain network-only and receive `no-store` from the server.

After authentication, on an `online` event, and whenever the standalone app returns to the foreground, the current app asks the browser to update the worker with HTTP-cache bypass. A new worker remains waiting until its entire precache succeeds. Noema then waits for mutations, an active chat turn, and passkey/provider-auth ceremonies to finish, locks new writes, flushes drafts and the Apollo snapshot, sends `SKIP_WAITING`, awaits `controllerchange`, and reloads the current URL programmatically. The URL, search parameters, IndexedDB drafts, and durable Apollo state survive this refresh; the human does not need an iPhone reload control.

An install or update failure leaves the current controller and snapshot in place, keeps the app read-only, and retries on a later online/foreground transition. An operational rollback must still publish one final worker at `/assets/sw.js`; that worker must delete caches whose names begin with Noema's Workbox cache prefix, unregister itself, and reload controlled clients. Removing the worker URL before that cleanup release strands installed clients.

## Offline storage and privacy

Installed mode uses the versioned `noema-pwa` IndexedDB database. One `snapshot` record contains Apollo's normalized cache, the cache schema version, recovery generation, last successful synchronization time, and at most 100 recent mutable query/variable pairs. Replacing that record is one IndexedDB transaction, so termination leaves either the previous complete generation or the replacement. Chat drafts and the task-capture draft are separate durable records because they must be saved between synchronized snapshots.

Noema requests persistent browser storage after authentication, but denial and quota exhaustion are nonfatal. The server remains canonical. The installed app's isolated origin storage and the iPhone device lock are the offline access boundary; Noema does not add application-level encryption to the Apollo snapshot.

A previously authenticated sentinel permits an installed app to reveal its saved snapshot when the server is unreachable. A reachable server response requiring login immediately hides that snapshot behind the passkey gate without deleting it. A successful login makes it available for reconciliation again.

## Recovery state machine

- `offline` exposes saved reads and editable drafts while every mutation is blocked.
- `checking` verifies `/auth/status` from the network and completes the application update check while writes remain blocked.
- `reconciling` pauses durable snapshot writes, waits for the GraphQL subscription acknowledgement, refreshes Chat boot and the latest transcript, refetches mounted queries, and revalidates remembered mutable reads with at most four concurrent requests.
- `online` begins only after the reconciled Apollo cache and metadata have committed atomically.
- `auth_required` hides cached private data behind the existing passkey surface.

Each network request captures the current recovery generation. Results from an older query generation are ignored. A transport/auth failure restores the in-memory baseline and retains the previous durable snapshot. A GraphQL error from one remembered query keeps that cached object stale and does not prevent unrelated reads from reconciling.

Mutation responses, immutable task/run history, debug profiles, OAuth/setup state, arbitrary artifact downloads, approvals, revision-fenced actions, and other commands are never queued or replayed. Offline agent execution, push notifications, and native iOS behavior are separate product concerns.

## Release validation

The browser production build must precede a release server build. `build.rs` requires the manifest, stable worker, HTML, icons, Vite manifest entries, and every file named by the worker's precache. Server tests verify MIME types, cache headers, worker scope, precache completeness, and `no-store` exclusions.

The manual iPhone check installs from authenticated Safari, opens representative Chat, Work, Memory, Settings, and textual artifact data, then relaunches in Airplane Mode. A second release must be deployed while version one is offline; reconnecting without terminating must produce a complete worker install, safe automatic activation and refresh, route/draft preservation, and an atomic data-generation advance before any write control becomes available.
