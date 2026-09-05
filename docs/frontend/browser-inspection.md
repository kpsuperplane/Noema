# Visual browser inspection

UI requests grant permission for browser inspection unless the user explicitly restricts it.
This includes audits, mocks, implementation, and visual validation. Do not request permission again.

Use available browser tools first. Prefer the running development app over reconstructed screens.
If browser tools cannot reach an authenticated app, use Playwright through the private Unix socket.
Keep product data unchanged unless the user also authorized data changes.

## Private socket access

The development server must be running with `web.local_graphql_socket` enabled.
The root development supervisor normally provides `/tmp/noema-codex/graphql.sock`.
The direct socket is `${NOEMA_HOME}/run/graphql.sock`.

The Linux `noema-build` profile uses the authenticated inspection relay.
Codex 0.153.2 blocks direct Unix-socket creation in this profile.
Its Linux network proxy also reports Unix-socket forwarding as unsupported.
Adding a Unix allow entry alone does not make direct socket calls work.
The [Codex permission reference](https://learn.chatgpt.com/docs/permissions) describes the supported proxy controls.

The development supervisor starts `scripts/noema-inspection-relay.mjs` beside the existing Unix relay.
It binds only `127.0.0.1` and forwards to the fixed private socket.
Every HTTP request and WebSocket upgrade requires its generated credential.
The protected store is `/tmp/noema-codex/inspection-credential.json`, with mode `0600` inside the root-owned `0700` directory.
The relay removes its credential during normal shutdown.

When `HTTP_PROXY` or `http_proxy` is set, `routeBrowserInspection` uses this relay through the Codex proxy.
The helper loads the credential internally. It never passes that credential to the browser or the Noema backend.
Do not print the credential file or copy its values into commands, screenshots, logs, or conversation context.
The existing `127.0.0.1` domain permission permits this route without disabling the sandbox or domain checks.

Run an inspection script inside the actual profile:

```sh
codex sandbox -C /root/noema -P noema-build -- node /path/to/inspection.mjs
```

The profile check passed HTTP access, a GraphQL query, mutation denial, and WebSocket acknowledgement on 2026-09-05.
The relay unit checks use `node --test scripts/noema-inspection-relay.test.mjs`.
They cover denied access, HTTP/WebSocket forwarding, ordinary-data preservation, credential protection, and shutdown.

Outside the Codex sandbox, check direct socket availability:

```sh
curl --unix-socket /tmp/noema-codex/graphql.sock http://localhost/auth/status
```

The response contains `"state":"authenticated"` when socket web access is available.
The socket serves app pages, assets, GraphQL POST requests, and GraphQL WebSockets.
It does not create a browser session. Some credential-management queries can remain unavailable.
See [server security](../server-security.md#local-codex-web-access).

[route-browser-inspection.mjs](../../scripts/route-browser-inspection.mjs) forwards Playwright requests directly to this socket.
It creates no TCP listener. It rejects GraphQL mutations through HTTP and WebSockets.
Use a fresh browser context with service workers blocked. Do not export real session cookies or recovery codes.

Use an existing Playwright installation when available. Otherwise, install it outside the repository:

```sh
inspection_dir=$(mktemp -d /var/tmp/noema-inspection.XXXXXX)
bun add --cwd "$inspection_dir" playwright
```

Create an inspection script in that temporary directory. Adjust the repository import if its location differs:

```js
import { chromium } from "playwright";
import { routeBrowserInspection } from "/root/noema/scripts/route-browser-inspection.mjs";

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
  serviceWorkers: "block"
});
await routeBrowserInspection(context);
const page = await context.newPage();
try {
  await page.goto("http://noema.local/settings/agents");
  await page.getByRole("heading", { name: "Agent models", exact: true }).waitFor();
  await page.screenshot({ path: "agents-desktop.png", fullPage: true });
} finally {
  await browser.close();
}
```

If Chromium is missing, run the temporary installation's `playwright install chromium` command.
Some root containers require Chromium's `--no-sandbox` launch argument.
Do not change the public server's authentication or create an unauthenticated TCP proxy.

## Review procedure

Inspect desktop and phone widths. Review long content, empty states, errors, and the focal action.
For dialogs, check opening focus, keyboard navigation, dismissal, and reachable actions.
Open menus and dialogs without submitting live changes. The helper blocks mutations as an additional guard.
Keep private screenshots outside committed artifacts unless their inclusion is authorized.
Use sample data in shareable mocks. Label source-based findings when live states are unavailable.
Record the routes, widths, observed results, and remaining limits in the audit or validation note.
