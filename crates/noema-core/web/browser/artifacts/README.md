# Browser failure artifacts

This directory contains the allowlisted telemetry writer for Noema's deterministic
browser acceptance harness. It is not a general sanitizer and must not be used to
retain arbitrary browser output.

## Retention policy

The writer accepts only bounded enums, counts, predeclared safe identifiers, and
raw URLs that are immediately collapsed to endpoint enums. A failed ephemeral run
may publish `failure-trace.json`. A failed `ephemeral-public` run with no supplied
secret canaries may additionally publish `screenshot.png`. Successful runs and
all `authenticated-live` runs publish nothing. `ephemeral-sensitive` runs and all
runs with canaries never invoke screenshot capture.

Eligible screenshot bytes are branded and measured through captured intrinsic
typed-array internal-slot getters. They are copied into a fixed-size plain
`Uint8Array` through the captured intrinsic `set`; caller iterators, species,
subclass accessors, and cross-realm `instanceof` behavior are not trusted.

Runtime inputs must be exact plain own-data records and dense plain-data arrays.
Accessors, inherited state, symbol/non-enumerable extras, and malformed or missing
required fields are rejected before policy branching; caller-owned values are
snapshotted once and never reread.

The JSON schema is fixed at version 1. It contains the scenario ID, failed outcome,
failure kind and optional safe step, plus bounded events. It never contains
timestamps, hostnames, ports, dynamic route IDs, raw URLs or paths, console text,
errors or stacks, headers, cookies, browser storage, environment values, GraphQL
variables, or request/response bodies. Native Playwright traces, reports, videos,
DOM/source snapshots, storage state, stdout, and wildcard uploads are forbidden.

`finish` returns an explicit relative-file allowlist only after atomic publication
and byte-for-byte revalidation. F5c and CI may upload only those returned names.
A crash before completion has no upload manifest, so nothing is eligible for
upload.

## Filesystem boundary

The harness must create a dedicated, existing, mode-0700 directory beneath the OS
temporary directory and give it to one writer process. The writer rejects a
symlink root, canonicalizes the root once, verifies ownership where available,
and operates only on the exact safe scenario directory and scenario-owned staging
names. It rejects symlinked owned entries, writes validated bytes with exclusive
creation and private permissions, syncs where supported, and renames an exclusive
staging directory to an absent final directory. Unrelated root entries are never
traversed or removed.

Same-UID replacement of the already validated root or its entries while the
writer is running is outside the JavaScript guarantee. The harness must enforce
exclusive single-writer ownership of its temporary root.

## Canary defense and limits

Before the first artifact write, the writer scans retained bytes and owned names
for every supplied canary in raw UTF-8, JSON escaping, upper/lower UTF-16
`\\uXXXX` (including surrogate pairs), upper/lower percent and form-space
encoding (including fully byte-encoded unreserved bytes), padded/unpadded base64
and base64url, and upper/lower hexadecimal. NFC and NFD variants are transformed
independently. Malformed Unicode/lone surrogates are rejected. Rejected values and
callback errors are never echoed in errors.

This is defense in depth, not proof that an arbitrary secret is absent. It does
not detect split, compressed, encrypted, differently normalized, transformed, or
unknown representations. PNG byte scanning cannot prove that visible pixels do
not render secret text; screenshot safety therefore comes from the hard-coded
synthetic-public profile policy, not image inspection. Callers must still avoid
placing real credentials or personal data into synthetic-public scenarios.

## Completion consumer contract

Consumers must:

1. create a fresh private OS-temp root for this process;
2. predeclare safe scenario and step IDs;
3. supply every bounded secret canary known to the deterministic run;
4. avoid all native browser artifact/report/storage capture;
5. call `finish` once and retain only its `files` result;
6. post-run validate those exact files and never wildcard the root, staging paths,
   browser output, reports, storage, or stdout.
