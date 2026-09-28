# Visual browser inspection

UI requests grant permission for browser inspection unless the user explicitly restricts it.
This includes audits, mocks, implementation, and visual validation. Do not request permission again.

Use available browser tools first. Prefer the running development app over reconstructed screens.
If browser tools cannot reach an authenticated app, use Playwright through the private Unix socket.
Keep product data unchanged unless the user also authorized data changes.

## Private socket access

Follow [Noema Development Access](../../AGENTS.md#noema-development-access) for startup, exact paths, and access failures.
Use `/tmp/noema-codex/graphql.sock` with the `noema-build` profile.
The service-owned `${NOEMA_HOME}/run/graphql.sock` is the source socket, not the restricted profile's access path.
The home view at `/tmp/noema-codex/home` provides file reads only; it does not forward socket connections.
See [development permissions](../development/codex-permissions.md) for profile settings and verified checks.

When another profile requires the Codex network proxy, use the authenticated inspection relay.
A Unix socket allow entry alone does not enable socket forwarding on this Linux version.
The [Codex permission reference](https://learn.chatgpt.com/docs/permissions) describes the proxy controls.

The development supervisor starts `scripts/noema-inspection-relay.mjs` beside the existing Unix relay.
It binds only `127.0.0.1` and forwards to the fixed private socket.
Every HTTP request and WebSocket upgrade requires its generated credential.
The protected store is `/tmp/noema-codex/inspection-credential.json`, with mode `0600` inside the root-owned `0700` directory.
The relay removes its credential during normal shutdown.

When `HTTP_PROXY` or `http_proxy` is set, `routeBrowserInspection` uses this relay through the Codex proxy.
The helper loads the credential internally. It never passes that credential to the browser or the Noema backend.
Do not print the credential file or copy its values into commands, screenshots, logs, or conversation context.
A profile with domain filtering must allow `127.0.0.1` for this route.

Run an inspection script inside the actual profile:

```sh
codex sandbox -C /root/noema -P noema-build -- node /path/to/inspection.mjs
```

The relay unit checks use `node --test scripts/noema-inspection-relay.test.mjs`.
They cover denied access, HTTP/WebSocket forwarding, ordinary-data preservation, credential protection, and shutdown.

With `noema-build`, check direct socket availability:

```sh
curl --unix-socket /tmp/noema-codex/graphql.sock http://localhost/auth/status
```

The response contains `"state":"authenticated"` when socket web access is available.
The socket serves app pages, assets, GraphQL POST requests, and GraphQL WebSockets.
It does not create a browser session. Some credential-management queries can remain unavailable.
See [server security](../server-security.md#development-bypass-and-local-access).

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
Report the routes, widths, observed results, and remaining limits with the change.
