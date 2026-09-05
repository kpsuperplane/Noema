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

Check availability without reading credentials:

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
