// Authenticated loopback access to one private Noema development socket.
import http from "node:http";
import { randomBytes, timingSafeEqual } from "node:crypto";
import { stat, writeFile, rename, unlink, readFile } from "node:fs/promises";
import { dirname } from "node:path";

const [socketPath, credentialPath] = process.argv.slice(2);
if (!socketPath || !credentialPath) throw new Error("Socket and protected credential paths are required.");
const directory = await stat(dirname(credentialPath));
if (directory.uid !== process.getuid() || (directory.mode & 0o077) !== 0) {
  throw new Error("The credential directory must be private and owned by this user.");
}
const token = randomBytes(32).toString("hex");
const expected = Buffer.from(token);
const connections = new Set();
function authorized(request) {
  const supplied = Buffer.from(request.headers["x-noema-inspection"] ?? "");
  return supplied.length === expected.length && timingSafeEqual(supplied, expected);
}
function forward(request) {
  const headers = { ...request.headers, host: "noema.local" };
  delete headers["x-noema-inspection"];
  delete headers.authorization;
  delete headers.cookie;
  return http.request({ socketPath, method: request.method, path: request.url, headers });
}
const server = http.createServer((request, response) => {
  if (!authorized(request)) { response.writeHead(401).end(); return; }
  const upstream = forward(request);
  upstream.on("response", result => {
    response.writeHead(result.statusCode, result.headers);
    result.pipe(response);
  });
  upstream.on("error", () => { if (!response.headersSent) response.writeHead(502); response.end(); });
  upstream.setTimeout(30_000, () => upstream.destroy());
  request.on("aborted", () => upstream.destroy());
  response.on("close", () => upstream.destroy());
  request.pipe(upstream);
});
server.on("connection", socket => {
  connections.add(socket);
  socket.on("close", () => connections.delete(socket));
});
server.on("upgrade", (request, client, head) => {
  if (!authorized(request)) { client.end("HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n"); return; }
  if (request.url !== "/graphql/ws") { client.end("HTTP/1.1 404 Not Found\r\n\r\n"); return; }
  const upstream = forward(request);
  upstream.on("upgrade", (response, socket, remainder) => {
    socket.setTimeout(0);
    client.write(`HTTP/1.1 ${response.statusCode} Switching Protocols\r\n`);
    for (let i = 0; i < response.rawHeaders.length; i += 2) {
      client.write(`${response.rawHeaders[i]}: ${response.rawHeaders[i + 1]}\r\n`);
    }
    client.write("\r\n");
    if (remainder.length) client.write(remainder);
    if (head.length) socket.write(head);
    socket.on("error", () => client.destroy());
    client.on("close", () => socket.destroy());
    socket.pipe(client).pipe(socket);
  });
  upstream.on("response", () => client.end("HTTP/1.1 502 Bad Gateway\r\n\r\n"));
  upstream.on("error", () => client.destroy());
  upstream.setTimeout(30_000, () => upstream.destroy());
  client.on("error", () => upstream.destroy());
  client.on("close", () => upstream.destroy());
  upstream.end();
});
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});
const credentials = JSON.stringify({ port: server.address().port, token });
const temporary = `${credentialPath}.${process.pid}`;
await writeFile(temporary, credentials, { mode: 0o600, flag: "wx" });
await rename(temporary, credentialPath);
for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) process.once(signal, async () => {
  server.close();
  for (const socket of connections) socket.destroy();
  if (await readFile(credentialPath, "utf8").catch(() => "") === credentials) await unlink(credentialPath);
  process.exit(0);
});
console.log("Noema inspection relay ready.");
