# Provisional redistribution and licensing inventory

Status: **provisional; not release-ready**. Captured 2026-07-10 from commit
`5fc7cbf42def0560aa78a41da95cc40972d0479f`. Milestone 5 must freeze and inspect
the actual target-specific binaries, frontend bundles, Python environment,
downloaded local models, installers, updater, and signing/notarization inputs.
An item marked `UNKNOWN` or **review required** is a release blocker. This is an
engineering inventory, not legal advice.

## Evidence and classification

The primary evidence is `Cargo.toml`, crate manifests, `Cargo.lock`,
`cargo metadata --locked`, `crates/noema-core/web/package.json`, `bun.lock`,
matching installed Bun package metadata, the sidecar `pyproject.toml` and current
repo-local virtualenv distribution metadata, the Swift package and imports,
Tauri configuration, and Git-tracked assets. Lock hashes at capture time:

| Input | SHA-256 |
| --- | --- |
| `Cargo.lock` | `a45f82220281f1786042be8a7308846d578857d49d891bfe222f8d1c42f311ee` |
| `crates/noema-core/web/bun.lock` | `c2d95766b07b186c95afb99ca82f891cbc14263a8d746f28d65e9a3663647cc8` |
| `crates/noema-core/mnemosyne-sidecar/pyproject.toml` | `4f6cf0b9cc05abc6467f9486c15c5d64fe2009ea57403efc67fce1c318339cd8` |
| `crates/noema-core/apple-foundation-bridge/Package.swift` | `14965e5ba01883760917aad8ca4bb076142496e25f955275f9d0d6d04b7c7751` |
| `crates/noema-desktop/tauri.conf.json` | `19757fb158c139c1eba9e7e5ae3188896d15888f61fecafe4bafe07d1960f31a` |

Classification is conservative. A Rust normal-dependency closure is a
target-conditional linked-code candidate, not proof that every locked crate is
in every binary. A Bun production closure is a bundle source candidate, not
proof that tree-shaken source ships. Build, development, test, and
system-provided inputs are not described as bundled. Milestone 5 must derive the
inventory from final artifacts.

## Inventory summary

| Ecosystem/input | Runtime or bundle candidates | Build/dev/test/system only | Distribution status |
| --- | ---: | ---: | --- |
| Rust crates | 556 | 29 build-only; 0 unique dev-only | Candidate code for core/server and Tauri binaries; target pruning pending |
| Bun lock entries | 64 | 517 development/build-only | Production candidates feed generated JS/CSS; exact chunk inclusion pending |
| Python distributions in current virtualenv | 44 | 5 environment/dev-only | Environment is not locked and is not an approved release bundle |
| Swift packages | 0 third-party | 3 Apple system/toolchain modules | Bridge source may be built; Apple modules are system-provided |
| Repository assets | 2 source assets | 0 font/model/installer artifacts | SVG is copied into web bundles; PNG is a Tauri bundle icon |

## Rust core/server and Tauri desktop

Source: workspace/crate manifests plus `Cargo.lock` and
`cargo metadata --locked --format-version 1`. Resolved third-party total: 585
packages from crates.io. The 556-package normal closure is a union across
configured target conditions and is candidate linked code (`bundled/distributed:
target-dependent`). The 29-package build closure is source/build tooling only
(`bundled/distributed: no`). Exact license expressions below come from locked
crate metadata. The common obligation is to preserve applicable license,
copyright, attribution, NOTICE, and source terms in the final distribution;
exact texts and target membership are **review required**.

| Direct input | Version | License expression | Class | Source | Bundled/distributed | Notice/source obligation |
| --- | --- | --- | --- | --- | --- | --- |
| async-graphql | 7.2.1 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| async-stream | 0.3.6 | MIT | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| base64 | 0.22.1 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| bytes | 1.12.0 | MIT | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| cap-std | 4.0.2 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright, exception, and NOTICE if supplied |
| figment | 0.10.19 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| futures-util | 0.3.32 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| http | 1.4.2 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| libc | 0.2.186 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| readabilityrs | 0.1.3 | Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license, copyright, changes, and NOTICE if supplied |
| reqwest | 0.13.4 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| ring | 0.17.14 | Apache-2.0 AND ISC | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve both licenses/copyright and NOTICE if supplied |
| rmcp | 2.0.0 | Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license, copyright, changes, and NOTICE if supplied |
| rusqlite | 0.37.0 | MIT | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| scraper | 0.27.0 | ISC | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| serde / serde_json | 1.0.228 / 1.0.150 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| thiserror | 2.0.18 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| time | 0.3.51 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| tokio | 1.52.3 | MIT | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| ts-rs | 12.0.1 | MIT | runtime/build generation | crates.io/Cargo.lock | linked/generated-source input | Preserve license/copyright if code is included |
| url | 2.5.8 | MIT OR Apache-2.0 | runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| open | 5.3.6 | MIT | desktop runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve license/copyright |
| tauri | 2.11.3 | Apache-2.0 OR MIT | desktop runtime | crates.io/Cargo.lock | target-dependent linked code | Preserve chosen license/copyright; Apache NOTICE if supplied |
| tauri-build | 2.6.3 | Apache-2.0 OR MIT | build-only | crates.io/Cargo.lock | no | Preserve if redistributed separately; not in product by classification |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | test direct; also runtime transitive | crates.io/Cargo.lock | target-dependent because runtime closure also reaches it | Preserve chosen license/copyright; Apache NOTICE if supplied |

Transitive review findings: no Cargo package has a missing license field.
Eleven runtime-candidate crates declare MPL-2.0 and two declare
`MIT OR Apache-2.0 OR LGPL-2.1-or-later`; license choice, source-form
availability, modification status, target inclusion, and reciprocal obligations
are **review required**. Cargo also contains legacy non-SPDX separators such as
`MIT/Apache-2.0`; normalization without changing meaning is **review required**.
No incompatibility was established from metadata alone.

## Bundled frontend JavaScript, CSS, and static assets

Source: `package.json`, `bun.lock`, and matching local package metadata. The 64
production-closure lock entries are bundle source candidates; Vite-generated
JavaScript/CSS and the SVG are distributed by the Rust server and Tauri
`frontendDist`. Tree shaking means source-only candidates must not be claimed as
shipped until Milestone 5 analyzes final chunks.

| Direct production input | Resolved version | License expression | Source | Bundled/distributed | Notice/source obligation |
| --- | --- | --- | --- | --- | --- |
| @apollo/client | 4.2.3 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| @astryxdesign/core / theme-neutral | 0.1.2 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| @stylexjs/stylex | 0.19.0 | MIT | npm/Bun lock | compiled CSS/JS candidate | Preserve license/copyright if included |
| @tanstack/react-router | 1.170.17 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| @tanstack/react-virtual | 3.14.5 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | npm/Bun lock | desktop bundle candidate | Preserve chosen license/copyright and NOTICE if supplied |
| @xyflow/react | 12.11.1 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| boring-avatars | 2.0.4 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| d3-force | 3.0.0 | ISC | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| graphql | 16.14.2 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| graphql-ws | 6.0.8 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |
| lucide-react | 1.21.0 | ISC | npm/Bun lock | compiled icons candidate | Preserve license/copyright if included |
| react / react-dom | 19.2.7 | MIT | npm/Bun lock | compiled candidate | Preserve license/copyright if included |

The 517 development/build-only entries are not product contents by source
classification. Their direct roots are Astryx CLI, GraphQL Codegen, StyleX
Babel plugin, TanStack router tooling, TypeScript/type packages, Vite, ESLint,
and React plugins. Development metadata still proves an attribution requirement
for `caniuse-lite@1.0.30001799` (CC-BY-4.0), and source/build obligations require
review for `lightningcss@1.32.0` (MPL-2.0) and `argparse@2.0.1` (Python-2.0).

Release blockers: `css-mediaquery@0.1.2` is a runtime source candidate whose
local metadata says only ambiguous `BSD`. Fifty-nine platform-specific build
entries have `UNKNOWN` because matching metadata was not locally installed:
25 `@esbuild/*@0.28.1`, 24 `@rollup/rollup-*@4.62.2`, and 10
`lightningcss-*@1.32.0`. They are not asserted to ship, but must be resolved for
each release build target. No incompatible frontend license was established.

## Managed Mnemosyne Python sidecar

`pyproject.toml` is a range manifest, not a lock. The current ignored repo-local
Python 3.12 virtualenv is development evidence only and must not be treated as a
reproducible release environment. Direct runtime inputs are:

| Package | Manifest range | Current version | License expression | Source | Bundled/distributed | Notice/source obligation |
| --- | --- | --- | --- | --- | --- | --- |
| noema-mnemosyne-sidecar | 0.1.0 | 0.1.0 | UNKNOWN | repository/installed metadata | source present; release packaging undecided | Add explicit package license metadata; blocker |
| fastapi | >=0.115,<1 | 0.139.0 | MIT | PyPI/current virtualenv | provisional runtime candidate | Preserve license/copyright |
| uvicorn[standard] | >=0.30,<1 | 0.51.0 | BSD-3-Clause | PyPI/current virtualenv | provisional runtime candidate | Preserve license/copyright |
| mnemosyne-memory[embeddings] | >=3.11,<4 | 3.11.1 | MIT | PyPI/current virtualenv | provisional runtime candidate | Preserve license/copyright |
| pydantic | >=2,<3 | 2.13.4 | MIT | PyPI/current virtualenv | provisional runtime candidate | Preserve license/copyright |

The installed graph has 44 runtime-reachable distributions and 5
environment/dev-only distributions (`pip`, `pytest`, `iniconfig`, `pluggy`,
`Pygments`). Installed `pytest@9.1.1` does not satisfy the manifest dev range
`>=8,<9`, confirming the environment is not a release lock. Runtime-reachable
`annotated-types@0.7.0`, `loguru@0.7.3`, `py_rust_stemmers@0.1.8`, and
`tokenizers@0.23.1` report `UNKNOWN`; several other distributions report
non-SPDX labels such as `Apache License`, `Apache 2.0`, or full license text.
All are release blockers pending a locked environment and license-file audit.

No downloaded embedding/local-model weights are committed or present in the
repo-local virtualenv. The current `onnxruntime` distribution contains three
sample `.onnx` datasets; a release must prune or inventory them if it packages
the virtualenv wholesale. Actual FastEmbed model selection, weights, model-card
license, source, and redistribution terms are not frozen and are a release
blocker.

## Apple Foundation Swift bridge and system frameworks

The Swift package resolves no third-party Swift packages and has no
`Package.resolved`. It imports `Foundation` and conditionally imports
`FoundationModels`; `PackageDescription` is a build-only toolchain module.

| Input | Version | License expression | Source | Class | Bundled/distributed | Obligation |
| --- | --- | --- | --- | --- | --- | --- |
| NoemaFoundationBridge | unversioned (repository) | MIT | repository/root LICENSE | runtime executable source | bridge binary boundary not frozen | Include Noema MIT license |
| Foundation | OS/toolchain version | UNKNOWN (Apple system terms) | Apple system framework | system-provided runtime | no | Review platform terms; blocker |
| FoundationModels | macOS 26+ system version | UNKNOWN (Apple system terms) | Apple system framework | system-provided runtime | no | Review platform/model availability and terms; blocker |
| PackageDescription | Swift tools 6.0 | UNKNOWN (Apple toolchain terms) | Apple Swift toolchain | build-only | no | Review toolchain terms if redistributed; blocker |

## Fonts, icons, images, installers, updater/signing, and models

| Input | Version/hash | License expression | Source | Bundled/distributed | Obligation/status |
| --- | --- | --- | --- | --- | --- |
| `web/public/noema-mark.svg` | SHA-256 `02599d…93b2d3` | MIT | repository/root LICENSE | yes, copied into web assets | Include Noema MIT license |
| `noema-desktop/icons/icon.png` | SHA-256 `8c3027…47a7af` | MIT | repository/root LICENSE | yes, Tauri icon | Include Noema MIT license |
| Font files | absent | n/a | CSS uses system/local font-family fallbacks; no `@font-face` asset | no | Re-audit final CSS/assets |
| Other committed images | absent | n/a | Git inventory | no | Re-audit final bundle |
| Installer artifacts | absent | n/a | Tauri config targets `app` and `dmg` but no artifact is committed | no current artifact | Freeze generated installer contents at Milestone 5 |
| Updater inputs | absent | n/a | no updater configuration found | no | Inventory if added |
| Signing/notarization inputs | absent | n/a | Tauri `signingIdentity` is null | no | Production identity, entitlements, notarization, and terms remain blockers |
| Local model assets | absent from repository/current sidecar venv | UNKNOWN until selected | future model cache/release bundle | no current asset | Freeze each model/version/license/source before release |

## Notices decision and release blockers

MIT, ISC, BSD, Apache-2.0, CC-BY-4.0, MPL-2.0, and other recorded licenses
establish preservation, notice, attribution, change-marking, or source-review
work. Therefore a provisional root `THIRD_PARTY_NOTICES.md` exists now. It is
not a substitute for the final exact per-artifact notice set.

Milestone 5 must block release until all of the following are closed:

- Freeze target-specific Rust binaries and choose/document multi-license paths;
  audit MPL/LGPL candidates, license files, copyright, NOTICE, native/system
  libraries, and generated/link-time contents.
- Analyze final frontend chunks; resolve ambiguous `BSD` and all 59 `UNKNOWN`
  build entries; include exact applicable notices and source offers/links.
- Create a deterministic Python lock and release environment; resolve all
  missing/non-SPDX metadata and prune or inventory incidental package data.
- Freeze every local model and its model-card/license/source terms.
- Freeze Swift bridge packaging and review Apple system/toolchain terms.
- Freeze `.app`/DMG contents, signing, entitlements, notarization, updater, and
  any newly added fonts, icons, images, native libraries, or installer inputs.
- Replace the provisional notices with an artifact-derived, exact notice set.
