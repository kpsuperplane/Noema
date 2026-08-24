# Public Internet Security Audit

- Date: 2026-08-21
- Report revision date: 2026-08-22
- Repository revision: `7a79763533677007e0df68c7903efa0dc2b9bddb`
- Runtime reviewed: `.noema-dev`, started through `./attach`
- Mode: read-only adversarial review

## Decision

**Do not expose the instance until the remaining manual edge steps pass.**

H-03 remains open until Cloudflare applies explicit global and route request limits.

The operator must also keep recovery off the public route or accept its rotation behavior. The Tunnel must route through Caddy.

The requested local deployment controls now pass. No critical finding was confirmed.

## Post-audit remediation

The audit evidence below describes the original runtime.

- H-01 is partly remediated for the chosen live development process. Noema now listens on loopback as `noema-dev`.
- The process has no Linux capabilities and uses `NoNewPrivileges`. It remains a debug build.
- The instance home moved from the repository to `/var/lib/noema-dev` with mode `0700`.
- H-02 is remediated. Caddy 2.11.4 now uses Go 1.27.0 without the two listed TLS findings.
- H-03 remains open until the public Cloudflare route has explicit request limits.
- L-01 is remediated. The workspace now resolves `h2` 0.4.16.
- M-01 is remediated locally. Caddy uses a Caddy-only Unix administration socket and a restricted systemd sandbox.
- M-02 is remediated. Runtime logs retain request paths but remove OAuth query strings and the known custom session header.
- The journal retains at most 512 MiB for at most 30 days.
- M-03 is remediated. Diagnostics use mode `0600`, bounded events, and one bounded rotated file.
- L-02 is remediated with a one-year HSTS policy without `includeSubDomains` or preload.
- L-03 is remediated for this instance. All Noema data directories and files now deny group and other access.
- The later Tunnel connector runs as a dedicated `cloudflared` user without Linux capabilities and with a systemd sandbox.

## Severity model

| Severity | Meaning |
| --- | --- |
| Critical | A practical path gives unauthenticated control, secret disclosure, or destructive access. |
| High | A public-release blocker can cause host compromise, durable denial, or untrusted administrator access. |
| Medium | A significant containment, recovery, or authenticated-resource weakness exists. |
| Low | A limited weakness or defense-in-depth gap exists. |

## Scope

The review covered these areas:

- The live Noema and Caddy processes.
- TCP and Unix listeners.
- Host firewall and current DNS exposure.
- Caddy TLS, modules, administration, logging, and service containment.
- Browser authentication, passkeys, recovery, sessions, and native OAuth.
- HTTP, GraphQL, WebSocket, artifact, favicon, and callback routes.
- MCP setup, stdio execution, outbound HTTP, and action governance.
- Browser storage, service-worker behavior, CSP, and common script injection sinks.
- Noema home permissions, diagnostics, database state, and secret handling.
- Rust, Go, and browser dependency advisories.

The operator confirmed that they are the only authenticated user and own all current credentials.

This revision therefore excludes credential provenance and intentional authenticated-user abuse from finding scope.

The source revision changed during the review. I repeated the live and source checks against the revision above.

Several unrelated tracked and untracked files remained at cutoff. They included an in-progress authenticated Task workspace reader.

I reviewed its owner, traversal, symlink, and size controls. No new unauthenticated path was found.

The operator chose the live development process. Its debug build remains an accepted H-01 residual risk.

## Observed deployment

| Layer | Observed state | Assessment |
| --- | --- | --- |
| DNS | The public name resolved only to a Tailscale address. | The current shield appears active. |
| Firewall | Incoming and routed traffic used a default deny policy. | No external validation was possible. |
| Caddy | Caddy listened on ports 80 and 443. | Public edge when DNS changes. |
| Noema HTTP | `noema_web` listens on `127.0.0.1:3737`. | Correct private listener. |
| Noema process | The debug binary runs as `noema-dev` without capabilities and with `NoNewPrivileges`. | Accepted development residual. |
| Caddy upstream | `127.0.0.1:3737`. | Correct proxy destination. |
| Caddy admin | `/run/caddy/admin.sock` has mode `0200` inside a mode `0750` Caddy directory. | `noema-dev` cannot connect. |
| GraphQL socket | `.noema-dev/run/graphql.sock`, mode `0600`. | Private under a `0700` directory. |
| Browser auth | `login_required`, mode `required`. | Authentication bypass is now off. |

The current shield depends on DNS, Tailscale, and host firewall state. This review did not inspect Tailscale ACLs.

## Finding summary

| ID | Severity | Finding | Public release |
| --- | --- | --- | --- |
| H-01 | High | The target was a root debug process on a wildcard listener. | Partly remediated; debug risk accepted |
| H-02 | High | Caddy used a Go runtime with reachable TLS denial defects. | Remediated |
| H-03 | High | Missing explicit edge admission limits permit authentication-state exhaustion. | Blocked |
| M-01 | Medium | Caddy local control and containment widened compromise impact. | Remediated locally |
| M-02 | Medium | Proxy errors could record OAuth authorization codes. | Remediated |
| M-03 | Medium | The diagnostic log had no total size or retention bound. | Remediated |
| L-01 | Low | Rust `h2` has an unbounded empty-frame advisory. | Remediated |
| L-02 | Low | HTTPS responses omitted HSTS. | Remediated |
| L-03 | Low | Database and data modes did not match the production contract. | Remediated for this instance |
| L-04 | Low | MCP setup infers secret status from English key names. | Correct in normal hardening |
| L-05 | Low | Malformed passkey state can block browser recovery. | Correct in recovery hardening |

## Detailed findings

### H-01: The target is a root debug process on a wildcard listener

The live process was `target/noema-dev/debug/noema_web`. It ran as UID and GID zero.

It had all effective Linux capabilities. `NoNewPrivs` and seccomp were disabled.

The process listened on `0.0.0.0:3737`. A reachable port 3737 would bypass Caddy TLS and edge controls.

The launcher behavior explains this state:

- [`attach`](../../attach#L12-L15) starts `cargo dev`.
- [The development workflow](../../crates/noema-dev/src/workflow.rs#L126-L139) starts `cargo run`.
- The same workflow forces `NOEMA_WEB__HOST=0.0.0.0`.
- [The listener](../../crates/noema-server/src/web/mod.rs#L151-L160) binds the configured address.
- [The root check](../../crates/noema-server/src/bin/noema_web.rs#L24-L48) applies only to release builds.
- [Debug assets](../../crates/noema-server/src/web/assets.rs#L98-L115) are read from disk for each request.

The host firewall currently reduces direct reachability. It does not contain a defect inside Noema.

Any process-execution defect would become root host execution. It could also read Caddy's DNS credential.

Required remediation:

1. Stop using `./attach` as the public service.
2. Build the release binary from a reviewed revision.
3. Use the [provided service unit](../../deploy/systemd/noema.service#L6-L25).
4. Run Noema under its dedicated unprivileged account.
5. Keep the application listener on `127.0.0.1`.
6. Keep port 3737 closed at the host firewall.
7. Disable the local GraphQL socket unless development work needs it.
8. Verify that only Caddy owns public ports 80 and 443.

Acceptance evidence:

- `ps` shows the dedicated Noema account and a release binary.
- `/proc/<pid>/status` shows no effective capabilities and `NoNewPrivs: 1`.
- `ss -lntp` shows only `127.0.0.1:3737` for Noema.
- An external scan cannot connect to port 3737.

### H-02: Caddy uses a Go runtime with reachable TLS denial defects

The live Caddy binary reports version 2.11.4. It was built with Go 1.26.0.

Current `govulncheck -mode=binary` reported 42 symbol-level advisories. Symbol presence alone does not prove every execution path.

Two TLS findings are directly relevant to the public edge:

- [GO-2026-6090](https://pkg.go.dev/vuln/GO-2026-6090) lets a client force indefinite TLS key derivation. Go 1.26.6 fixes it.
- [GO-2026-4870](https://pkg.go.dev/vuln/GO-2026-4870) lets TLS 1.3 KeyUpdate traffic retain connection resources. Go 1.26.2 fixes it.

Caddy terminates TLS 1.3 for unauthenticated clients. Both affected symbol groups exist in the binary.

An attacker does not need a Noema credential. Repeated crafted TLS traffic can consume edge resources.

Required remediation:

1. Rebuild Caddy with a current supported Go patch.
2. Update Caddy modules during the rebuild.
3. Keep the required Cloudflare DNS module.
4. Verify the resulting binary with `go version -m`.
5. Run current `govulncheck -mode=binary` against the result.
6. Review all remaining symbol findings for reachable edge paths.

The [Go download page](https://go.dev/dl/) currently lists Go 1.26.7 and Go 1.27.0 as supported releases.

Acceptance evidence:

- The binary reports Go 1.26.7, Go 1.27.0, or a newer supported patch.
- `govulncheck` no longer reports the two TLS findings.
- Caddy configuration validation passes.
- TLS certificate and HTTP/2 checks still pass after reload.

### H-03: Missing explicit edge admission limits permit authentication-state exhaustion

The active site configures only TLS and `reverse_proxy`. It lacks explicit, contract-compliant admission and route limits.

The installed Caddy binary has no rate-limit module. This state conflicts with the [edge contract](../server-security.md#L520-L528).

Noema has useful application limits:

- HTTP concurrency is 256.
- GraphQL bodies are 64 KiB.
- Recovery bodies are 1 KiB.
- OAuth forms and queries are 8 KiB.
- WebSocket connections are capped at 64.

These limits do not prevent all pre-authentication work. The concurrency layer also sits inside session and setup middleware.

Three practical exhaustion paths exist:

1. Login starts create browser bindings before the ceremony limit check.
2. Native authorization stores resume data in unauthenticated sessions.
3. Every parsed recovery candidate performs a durable config rotation.

The browser session store holds 1,024 entries. Entries have a 24-hour idle expiry.

[The store](../../crates/noema-server/src/web/session_store.rs#L17-L20) rejects new sessions at capacity. It does not evict existing sessions.

[Passkey state](../../crates/noema-server/src/web/passkey.rs#L24-L26) permits 64 ceremonies for five minutes. New cookie jars can fill this state.

[Native authorization](../../crates/noema-server/src/web/native_oauth.rs#L336-L350) stores a resume query before authentication.

[Recovery](../../crates/noema-host/src/config/recovery.rs#L73-L106) rotates the code after every candidate. One invalid request therefore replaces the known code.

Edge limits reduce repeated pressure. They cannot prevent the first invalid request from replacing the known recovery code.

The current security contract accepts this tradeoff because the operator can read the newest local code. Continuous attempts still create availability risk.

An attacker can block new browser sessions for many hours. Recovery spam can also cause continuous serialized filesystem writes.

Required remediation:

1. Apply global connection, request, header, and body limits at the edge.
2. Add strict limits for recovery and ceremony-start routes.
3. Add strict limits for native OAuth authorization and token routes.
4. Use the [Nginx example](../../deploy/nginx/noema.conf.example#L1-L34) as the minimum baseline.
5. Move application concurrency admission before setup and session work.
6. Separate unauthenticated session capacity from authenticated session capacity.
7. Give OAuth resume state a short, small quota.
8. Keep recovery off the public edge, or record explicit acceptance of its rotation behavior.

Do not load-test the live recovery route. Every test candidate rotates the current recovery code.

Acceptance evidence:

- Edge configuration shows global and route-specific limits.
- Slow headers and oversized bodies fail before reaching Noema.
- A safe test instance resists session and ceremony exhaustion.
- Authenticated sessions remain available during rejected unauthenticated traffic.
- Edge logs contain no cookies, bearer values, recovery candidates, or OAuth codes.

### M-01: Caddy local control and containment widen compromise impact

Caddy exposes its administration API on `127.0.0.1:2019`. An unauthenticated local request returned the active configuration.

The returned configuration contains the placeholder `{env.CLOUDFLARE_API_TOKEN}`. It does not contain the exact token value.

The API accepts configuration changes. A compromised local process could replace routes or use environment placeholders in a hostile configuration.

Caddy's [API guidance](https://caddyserver.com/docs/api) recommends a permissioned Unix socket when untrusted code can run locally.

The Caddy service uses a non-root account and one effective bind capability. However, `NoNewPrivileges` and seccomp are disabled.

The capability bounding set is broad. The Cloudflare token scope and zone restrictions were not available for review.

Required remediation:

1. Bind the Caddy admin API to a permissioned Unix socket.
2. Allow only the Caddy account and root to access that socket.
3. Disable the API if safe reloads do not need it.
4. Enable `NoNewPrivileges` and tighten the capability bounding set.
5. Restrict writable paths and home access.
6. Restrict the Cloudflare token to required DNS changes and zones.
7. Rotate the token if any actual disclosure is suspected.

Acceptance evidence:

- The Noema account cannot read or change Caddy configuration.
- The Noema account cannot retrieve or expand the DNS credential.
- Caddy renewal and reload still work through the protected control path.

### M-02: Proxy errors can record OAuth authorization codes

Since 2026-08-01, the Caddy journal recorded 52,379 upstream connection failures. Eleven records included complete `/oauth/authorize` query strings.

The observed query values contained OAuth state and PKCE challenges. Those values are not credentials under Noema's information classes.

The same Caddy error path records the complete request URI. Callback requests can contain authorization codes, which are secrets.

No callback query error was found in the reviewed journal window. The risky sink remains present.

Access logging is disabled. Caddy also masks common sensitive headers.

Required remediation:

1. Remove query strings from Caddy error logs for every OAuth route.
2. Keep cookies and authorization headers masked.
3. Define a bounded journal retention policy.
4. Test an upstream outage with a non-secret sentinel query.

Acceptance evidence:

- The sentinel query value does not appear in Caddy logs.
- The route path, status, and useful ordinary diagnostics remain available.

### M-03: The diagnostic log has no total size or retention bound

The live `.noema-dev/errors.log` file was 43,937,416 bytes. It used mode `0644` inside a mode `0700` home.

[The logger](../../crates/noema-home/src/diagnostics.rs#L43-L53) appends JSON lines without rotation or a total size cap.

[Diagnostic events](../../crates/noema-home/src/diagnostics.rs#L62-L79) can carry uncapped, unredacted raw payloads.

Exact checks found neither the current recovery code nor the Caddy DNS token in this log. No committed secret value was found.

The missing bound can still cause disk exhaustion. Raw private information can also outlive its useful diagnostic period.

Required remediation:

1. Add a total log size and retention limit.
2. Enforce mode `0600` when opening the log.
3. Bound every raw payload before serialization.
4. Preserve useful ordinary values in protected diagnostics.
5. Keep secrets out through typed provenance and sink policy.

### L-01: Rust `h2` has an unbounded empty-frame advisory

`cargo audit` found `h2` 0.4.15 at [`Cargo.lock`](../../Cargo.lock#L3324-L3325).

[RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258) covers unbounded empty DATA frames. Version 0.4.16 fixes the issue.

Noema's public listener uses HTTP/1 behind Caddy. The inbound Noema server path is therefore not directly exposed to this issue.

Outbound clients can use HTTP/2. Their current response-draining paths reduce reachability but do not remove the affected dependency.

Required remediation:

1. Upgrade `h2` to 0.4.16 or newer.
2. Run focused outbound HTTP tests.
3. Rerun `cargo audit`.

### L-02: HTTPS responses omit HSTS

Live HTTPS responses contained CSP, no-referrer, `nosniff`, and a narrow Permissions Policy. They lacked `Strict-Transport-Security`.

Caddy redirects HTTP to HTTPS. A first visit can still be downgraded before the browser learns an HTTPS-only policy.

Secure cookies and passkey origin binding reduce the impact. They do not create an HSTS policy.

Required remediation:

1. Add HSTS at the edge after the final HTTPS name is stable.
2. Choose `includeSubDomains` only after checking every affected name.
3. Choose preload only after the full preload commitment review.

### L-03: Database and data modes do not match the production contract

The outer `.noema-dev` directory uses mode `0700`. The configuration and GraphQL socket use mode `0600`.

The database directory uses mode `0755`. SQLite, WAL, and SHM files use mode `0644`.

Many task and artifact directories also use `0755`. Their files commonly use `0644`.

The private outer directory currently prevents traversal by other users. Copies or deployment moves can remove that protection.

[Home initialization](../../crates/noema-home/src/initialization.rs#L19-L52) corrects only the home, run directory, and configuration.

[Store opening](../../crates/noema-store/src/runtime.rs#L54-L80) creates database parents but does not correct their modes.

Required remediation:

1. Use the service unit's `UMask=0077` setting.
2. Set every data directory to `0700`.
3. Set databases, WAL, SHM, credentials, and configuration to `0600`.
4. Verify permissions after startup and SQLite WAL creation.

### L-04: MCP setup infers secret status from English key names

MCP setup has separate safe and secret input types. This structure is a useful control.

[Transport validation](../../crates/noema-capabilities/mcp/src/setup.rs#L317-L363) also rejects keys containing English terms such as `token` or `cookie`.

A neutral key can still carry a credential into ordinary configuration. An ordinary key can also be rejected because of its spelling.

This check conflicts with Noema's explicit secret-classification contract. The concrete setup path limits the current impact.

Required remediation:

1. Remove key-name matching as the secret authority.
2. Use the existing explicit secret input provenance.
3. Assert that representative ordinary values remain unchanged.
4. Assert that actual secret values never enter safe configuration.

### L-05: Malformed passkey state can block browser recovery

[The setup barrier](../../crates/noema-server/src/web/passkey.rs#L176-L196) checks only whether a passkey row exists.

[The store query](../../crates/noema-store/src/human_passkeys.rs#L88-L103) does not validate the WebAuthn credential structure.

Browser login and registration later deserialize every stored passkey. One malformed but valid JSON row makes both operations fail.

The [registration path](../../crates/noema-server/src/web/passkey.rs#L252-L278) cannot add a replacement because it parses all old rows first.

Existing native bearer credentials can still cross the setup barrier. [A current test](../../crates/noema-server/src/web/router/tests.rs#L1037-L1066) demonstrates this split state.

This condition needs database corruption, incompatible state, or local modification. No remote creation path was found.

Required remediation:

1. Detect malformed stored credential state before normal login.
2. Let a valid recovery grant replace malformed credentials.
3. Add one focused regression test for this recovery path.

## Verified controls

The following controls worked during this review:

- Browser authentication reports `login_required` and mode `required`.
- A wrong Host returns `400`.
- Browser GraphQL without Origin returns `403`.
- Browser GraphQL with the exact Origin reaches authentication and returns `401`.
- A hostile CORS preflight receives no permissive credentialed CORS headers.
- Browser cookies are private, `HttpOnly`, `SameSite=Strict`, and `Secure` under HTTPS.
- Session identifiers rotate after authentication.
- Sessions have capacity, idle, absolute, targeted revocation, and WebSocket termination controls.
- Passkey ceremonies are random, browser-bound, one-use, five-minute, and capped.
- Stored recovery codes use canonical base64url. Canonical candidates use constant-time comparison, and every attempt durably rotates the code.
- Native OAuth uses exact clients, S256 PKCE, one-use codes, hashed tokens, and family replay revocation.
- Authentication occurs before GraphQL body parsing.
- GraphiQL is disabled on `/graphql`.
- The GraphQL schema route requires authentication.
- MCP stdio defaults to disabled and checks its startup gate before spawning.
- The local GraphQL socket is POST-only and has no Caddy route.
- CSP blocks external scripts, framing, alternate bases, and alternate form destinations.
- Responses also set no-referrer and `nosniff`.
- Private and network responses default to `Cache-Control: no-store`.
- The PWA excludes authentication, OAuth, GraphQL, callbacks, and artifact downloads from navigation fallback.
- No browser bearer-token storage was found.
- No dangerous React HTML sink or direct JavaScript evaluation sink was found.
- No public source maps were found.
- Artifact paths reject traversal and symbolic-link escapes.
- Direct downloads validate public targets, pin DNS, disable proxies, and recheck redirects.
- Favicon fetches use the same public-target controls and bound decoded images.
- Document parsing uses a separate worker with time and memory bounds.
- Stdio MCP clears inherited environment values and does not use shell parsing.
- Reviewed governed actions bind exact arguments, schemas, destinations, and revisions.
- Approved governed actions are one-use. Uncertain writes are not retried.
- Current DNS remains Tailscale-only.
- Current firewall policy denies public incoming and routed traffic.
- The TLS certificate matches the configured name and remains valid through 2026-10-30.
- TLS 1.0 and 1.1 were rejected. TLS 1.2 and 1.3 were accepted.
- Strong repository scans found no committed high-confidence secret value.
- Exact live checks found no recovery code or Caddy token in `.noema-dev` outside protected configuration.

## Dependency disposition

### Rust

`cargo audit` reported four vulnerabilities and 24 allowed warnings.

- `h2` is tracked as L-01.
- Two `quick-xml` findings enter through desktop-only Tauri plist handling.
- The `rsa` timing finding enters through Web Push dependencies.
- Noema uses ES256 VAPID keys and performs no RSA private-key operation there.

The [existing review](../server-security.md#L548-L570) already records the `quick-xml`, `rsa`, and browser-tool dispositions.

### Go and Caddy

Binary `govulncheck` reported 42 symbol-level findings. H-02 confirms direct reachability for two incoming TLS findings.

The other reports need a new review after the Caddy rebuild. Updating Go and modules should remove many results first.

### Browser packages

`bun audit` in `apps/web` reported 12 advisories. The affected packages were `brace-expansion`, `js-yaml`, `nanoid`, and `postcss`.

Most paths are build, lint, code-generation, Vite, or editor dependencies. No deployed exploit path was demonstrated.

Update these dependencies through normal frontend maintenance. Rebuild and repeat the static sink review afterward.

## Residual risks and unverified controls

These items need separate evidence or product decisions:

- This review did not inspect Tailscale ACLs or device membership.
- This review did not verify Cloudflare token scope or zone restrictions.
- No complete encrypted off-host backup or recent restore record was visible locally.
- A complete backup must include the full Noema home, not only SQLite.
- Kernel browser private-network controls depend on the external provider's documented network isolation.
- Remote browser subresources and DNS rebinding were not tested.
- Installed PWAs retain Apollo state and drafts in IndexedDB after normal logout.
- Use the explicit erase action before transferring or retiring a browser profile.
- The LLM action reviewer receives untrusted content and classifies actions.
- Deterministic policy auto-executes substantive low-risk and medium-risk classifications.
- Keep sensitive integrations on `AlwaysAsk` until adversarial prompt-injection evaluation passes.
- Native iOS and macOS clients were not built or inspected dynamically.

## Public enablement checklist

Do not change public DNS or firewall access until every blocking item passes.

1. Deploy a reviewed release binary under the dedicated Noema service account.
2. Confirm loopback-only Noema HTTP and no public port 3737.
3. Rebuild Caddy with patched Go and updated modules.
4. Add global and route-specific edge resource limits.
5. Protect the Caddy admin API with Unix permissions or disable it.
6. Remove OAuth query strings from edge error logs.
7. Correct all Noema home file modes.
8. Add diagnostic size and retention controls.
9. Add HSTS after the public name is final.
10. Create an encrypted, off-host, whole-home backup.
11. Complete one full restore test while Noema is stopped.
12. Verify provider-side private-network denial for Kernel, or disable Kernel.
13. Run the repository validation commands.
14. Run the edge acceptance tests from a non-Tailscale network.

Required repository validation:

```text
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
cargo audit
cd apps/web && bun audit
```

Required edge validation:

```text
sudo /bin/sh -c 'set -a; . /etc/caddy/cloudflare.env; exec /usr/local/bin/caddy validate --config /etc/caddy/Caddyfile'
go version -m /usr/local/bin/caddy
go run golang.org/x/vuln/cmd/govulncheck@latest -mode=binary /usr/local/bin/caddy
```

External acceptance must confirm these results:

| Check | Required result |
| --- | --- |
| TCP 80 | Redirects to the canonical HTTPS name. |
| TCP 443 | Serves only the reviewed Caddy edge. |
| TCP 3737 | Closed or filtered from every external interface. |
| Unknown public Host | Rejected at the edge without forwarding. |
| Wrong Host sent directly to Noema | `400`. |
| Missing Origin mutation | `403`. |
| Unauthenticated GraphQL | `401`. |
| GraphQL GET | `404` while GraphiQL remains disabled. |
| HSTS | Present with the approved policy. |
| Oversized headers or bodies | Rejected at the edge. |
| Excess unauthenticated requests | Rate-limited before Noema state changes. |
| OAuth outage sentinel | Absent from edge logs. |
| Session exhaustion test | Authenticated sessions remain usable. |

Use an isolated database clone for recovery and exhaustion tests. Do not use the live recovery code in a load test.

## Review limitations

The review used static source inspection and safe live probes. It did not change application, proxy, firewall, DNS, or Tailscale state.

It did not submit a recovery candidate. It did not complete a passkey ceremony or issue a new credential.

It did not stop the daemon. It did not run destructive load tests, fuzzing, or exploit payloads.

It did not use an external non-Tailscale scanner. Public reachability conclusions therefore depend on local firewall and DNS evidence.

It did not run a real browser or service-worker update test. PWA conclusions come from static inspection.

It did not read or print recovery codes, tokens, private keys, session cookies, authorization codes, or PKCE verifiers.

No application tests ran because this task was an audit-only review. The commands above define the required post-remediation validation.

## Final assessment

The application has strong authentication, authority, session, SSRF, path, and secret-boundary controls. These controls reduce common public attack paths.

The local deployment controls now implement the chosen live-development boundary. The debug build remains an accepted residual risk.

Explicit Cloudflare limits remain the decisive blocker. Recovery exposure and Tunnel routing remain manual operator steps.
