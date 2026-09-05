// Route a Playwright context through the private development socket.
// This read-only helper creates no TCP listener and never reads browser credentials.
import http from "node:http";
import { createRequire } from "node:module";
const require = createRequire(new URL("../apps/web/package.json", import.meta.url));
const { parse, getOperationAST } = require("graphql");
const WebSocket = require("ws");

function readOnlyOperation(payload) {
  try {
    const operation = getOperationAST(parse(payload.query), payload.operationName);
    return operation?.operation === "query" || operation?.operation === "subscription";
  } catch { return false; }
}

export async function routeBrowserInspection(context, socketPath = "/tmp/noema-codex/graphql.sock") {
  await context.route("http://noema.local/**", async route => {
    const request = route.request();
    const url = new URL(request.url());
    const isQuery = request.method() === "POST" && url.pathname === "/graphql";
    let allowed = request.method() === "GET";
    if (isQuery) {
      try { allowed = readOnlyOperation(request.postDataJSON()); } catch { allowed = false; }
    }
    if (!allowed) {
      await route.fulfill({ status: 403, contentType: "application/json", body: JSON.stringify({ errors: [{ message: "Browser inspection is read-only." }] }) });
      return;
    }
    try {
      const response = await new Promise((resolve, reject) => {
        const upstream = http.request({ socketPath, path: url.pathname + url.search, method: request.method(),
          headers: { host: "noema.local", "content-type": "application/json" } }, response => {
          const chunks = [];
          response.on("data", chunk => chunks.push(chunk));
          response.on("error", reject);
          response.on("end", () => resolve({ status: response.statusCode, headers: response.headers, body: Buffer.concat(chunks) }));
        });
        upstream.on("error", reject);
        upstream.setTimeout(30_000, () => upstream.destroy(new Error("Socket request timed out.")));
        upstream.end(request.postDataBuffer());
      });
      delete response.headers["transfer-encoding"];
      await route.fulfill(response);
    } catch { await route.abort("failed"); }
  });
  await context.routeWebSocket("ws://noema.local/graphql/ws", socket => {
    const upstream = new WebSocket(`ws+unix://${socketPath}:/graphql/ws`, "graphql-transport-ws", {
      headers: { host: "noema.local", origin: "http://noema.local" }
    });
    const pending = [];
    socket.onMessage(message => {
      try {
        const payload = JSON.parse(String(message));
        if (payload.type === "subscribe" && !readOnlyOperation(payload.payload)) {
          socket.send(JSON.stringify({ type: "error", id: payload.id, payload: [{ message: "Browser inspection is read-only." }] }));
          return;
        }
      } catch { socket.close({ code: 1008, reason: "Invalid inspection message" }); return; }
      if (upstream.readyState === WebSocket.OPEN) upstream.send(message);
      else pending.push(message);
    });
    upstream.on("open", () => pending.splice(0).forEach(message => upstream.send(message)));
    upstream.on("message", (data, binary) => socket.send(binary ? data : data.toString()));
    upstream.on("error", () => socket.close({ code: 1011, reason: "Development socket unavailable" }));
    upstream.on("close", () => socket.close());
    socket.onClose(() => upstream.close());
  });
}
