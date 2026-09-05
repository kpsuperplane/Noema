import assert from "node:assert/strict";
import { test } from "node:test";
import http from "node:http";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, stat, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createRequire } from "node:module";
const require = createRequire(new URL("../apps/web/package.json", import.meta.url));
const { WebSocket, WebSocketServer } = require("ws");

test("protected inspection relay", { timeout: 15_000 }, async t => {
  const directory = await mkdtemp(join(tmpdir(), "noema-relay-test-"));
  const socketPath = join(directory, "graphql.sock");
  const credentialPath = join(directory, "inspection-credential.json");
  let calls = 0;
  const backend = http.createServer((request, response) => {
    calls++;
    assert.ok(!request.headers["x-noema-inspection"] && !request.headers.authorization && !request.headers.cookie,
      "relay credentials and browser credentials must not reach the backend");
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ method: request.method, path: request.url, trace: request.headers["x-trace-id"] }));
  });
  const websocket = new WebSocketServer({ noServer: true });
  backend.on("upgrade", (request, socket, head) => {
    calls++;
    assert.ok(!request.headers["x-noema-inspection"], "relay credential must not reach WebSocket backend");
    websocket.handleUpgrade(request, socket, head, client => {
      client.on("message", () => client.send('{"type":"connection_ack"}'));
    });
  });
  backend.listen(socketPath);
  await once(backend, "listening");
  const child = spawn(process.execPath, [new URL("./noema-inspection-relay.mjs", import.meta.url).pathname,
    socketPath, credentialPath], { stdio: ["ignore", "pipe", "pipe"] });
  let output = "";
  child.stdout.on("data", chunk => { output += chunk; });
  child.stderr.on("data", chunk => { output += chunk; });
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) { const exited = once(child, "exit"); child.kill(); await exited; }
    for (const client of websocket.clients) client.terminate();
    websocket.close();
    await new Promise(resolve => backend.close(resolve));
    await rm(directory, { recursive: true, force: true });
  });
  await once(child.stdout, "data");
  const credential = JSON.parse(await readFile(credentialPath, "utf8"));
  const origin = `http://127.0.0.1:${credential.port}`;
  const headers = { "x-noema-inspection": credential.token };

  await t.test("unauthorized HTTP and WebSocket requests never reach the private socket", async () => {
    for (const method of ["GET", "POST"]) {
      const response = await fetch(origin + "/graphql", { method, headers: { "x-noema-inspection": "invalid" } });
      assert.equal(response.status, 401);
    }
    const client = new WebSocket(origin.replace("http:", "ws:") + "/graphql/ws");
    const rejected = await new Promise((resolve, reject) => {
      client.once("unexpected-response", (request, response) => {
        resolve(response.statusCode); response.resume(); request.destroy();
      });
      client.on("error", () => {});
      client.once("open", () => reject(new Error("Unauthenticated WebSocket opened.")));
    });
    assert.equal(rejected, 401);
    assert.equal(calls, 0);
  });
  await t.test("authorized HTTP and WebSocket preserve ordinary data without forwarding credentials", async () => {
    const response = await fetch(origin + "/graphql?ordinary=secret-field-name", {
      method: "POST", headers: { ...headers, authorization: "synthetic", cookie: "synthetic",
        "x-trace-id": "ordinary-id", "x-unix-socket": "/tmp/unlisted.sock" }
    });
    assert.deepEqual(await response.json(), { method: "POST", path: "/graphql?ordinary=secret-field-name", trace: "ordinary-id" });
    const client = new WebSocket(origin.replace("http:", "ws:") + "/graphql/ws", "graphql-transport-ws", { headers });
    await once(client, "open");
    const answer = once(client, "message");
    client.send('{"type":"connection_init"}');
    assert.deepEqual(JSON.parse(String((await answer)[0])), { type: "connection_ack" });
    const closed = once(client, "close"); client.close(); await closed;
    assert.equal(calls, 2);
  });
  await t.test("credential stays protected and shutdown removes live access", async () => {
    assert.equal((await stat(credentialPath)).mode & 0o777, 0o600);
    assert.ok(!output.includes(credential.token), "credential must not enter process output");
    const exited = once(child, "exit"); child.kill(); await exited;
    await assert.rejects(stat(credentialPath), { code: "ENOENT" });
    await assert.rejects(fetch(origin + "/auth/status", { headers }));
  });
});
