# Noema Foundation Reproducibility Baseline

Captured at `2026-07-10T00:42:39Z` from commit `41af3eeedb674119667891a84eda44c2d6052092`.

> Timings are machine-specific and informational. Foundation does not enforce performance thresholds.

## Cache and wrapper policy

Cargo must use the repository-configured `.cargo/rustc-wrapper`, which delegates every Rust compilation to `sccache`. Never run `cargo clean`, clear/reset cache state, or set, unset, or modify `CARGO_BUILD_RUSTC_WRAPPER`, `RUSTC_WRAPPER`, `CC`, or `CXX`. Only `CARGO_TARGET_DIR` may be set, solely for isolated measurement targets.

Wrapper SHA-256: `358f50df990960c506d1bf525604cc11edbf08cbfe0b90e023a7b716ddd1922d`

## Machine and tools

- OS: `macOS-27.0-arm64-arm-64bit`
- Architecture: `arm64`
- CPU: `Apple M5`
- Repository dirty: `true` ({'tracked': 0, 'untracked': 2, 'staged': 0, 'unstaged': 0})

| Tool | Status | Version |
| --- | --- | --- |
| `bun` | available | 1.3.14 |
| `cargo` | available | cargo 1.96.0 (30a34c682 2026-05-25) |
| `python` | available | Python 3.9.6 |
| `rustc` | available | rustc 1.96.0 (ac68faa20 2026-05-25)<br>binary: rustc<br>commit-hash: ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96<br>commit-date: 2026-05-25<br>host: aarch64-apple-darwin<br>release: 1.96.0<br>LLVM version: 22.1.2 |
| `swift` | available | Apple Swift version 6.3.3 (swiftlang-6.3.3.1.3 clang-2100.1.1.101)<br>Target: arm64-apple-macosx28.0<br>swift-driver version: 1.148.6 |
| `tauri_cli` | available | tauri-cli 2.11.4 |
| `xcode` | available | Xcode 26.6<br>Build version 17F113 |

## Dependency locks

| Path | Present | SHA-256 |
| --- | --- | --- |
| `Cargo.lock` | true | `a45f82220281f1786042be8a7308846d578857d49d891bfe222f8d1c42f311ee` |
| `crates/noema-core/mnemosyne-sidecar/pyproject.toml` | true | `4f6cf0b9cc05abc6467f9486c15c5d64fe2009ea57403efc67fce1c318339cd8` |
| `crates/noema-core/web/bun.lock` | true | `c2d95766b07b186c95afb99ca82f891cbc14263a8d746f28d65e9a3663647cc8` |

## Rust timings

| Scenario | Command | Samples (seconds) | Median (seconds) |
| --- | --- | --- | ---: |
| `clean-workspace-build` | `cargo build --workspace --timings` | 404.348592, 600.245011, 545.101427 | 545.101427 |
| `incremental-frontend-asset-check` | `cargo check --workspace --timings` | 3.508565, 4.642386, 5.467828 | 4.642386 |
| `incremental-graphql-check` | `cargo check --workspace --timings` | 5.899912, 4.245228, 4.301040 | 4.301040 |
| `incremental-provider-check` | `cargo check --workspace --timings` | 5.961554, 4.205629, 4.761491 | 4.761491 |
| `incremental-store-check` | `cargo check --workspace --timings` | 4.804395, 5.198600, 4.996993 | 4.996993 |
| `no-op-workspace-build` | `cargo build --workspace --timings` | 3.111890, 0.701630, 0.519490 | 0.701630 |

## Frontend bundle

| Path | Kind | Raw bytes | Gzip bytes | SHA-256 |
| --- | --- | ---: | ---: | --- |
| `SettingsPage.js` | lazy | 111516 | 23748 | `642f703ef978df66497baffcdd9789eb80ca59337cd4638e4c09449bf5abde1e` |
| `agents.js` | lazy | 139 | 125 | `7e41ff09738189893f08d52073a4c85aac9cccc3fb5177c19958aeda979d6ef2` |
| `app.js` | eager | 927319 | 264423 | `31e34229d8f10dcb5fe83337efee973f43f5298dde7329fcbe917f11d21bbe74` |
| `approvals.js` | lazy | 149 | 136 | `335366cb575a1aeca6b8d8eee949e991406ba4358d4d4676890e89288aa41fad` |
| `core.js` | lazy | 2441 | 980 | `8261a5e51510afea7347ee6c40e783127639b88b158982711196012d6d50ceb3` |
| `event.js` | lazy | 1398 | 675 | `ae2e6fcbf8e723fac2233cedd10dde72e6c6eae76cb420f543e310bc636e255d` |
| `identities.js` | lazy | 150 | 134 | `a60dabd780e92ff9430a911479b5336dc2c220f3276827f2de253e73581a24bc` |
| `index.html` | eager | 425 | 260 | `9f337951d6f9ac80e89ec750549472610ae6b1b0bdff148600e59898ddcb21dc` |
| `index.js` | lazy | 86 | 100 | `8f3ea5051830a1fd1ba978bfcced0aad9d937d15b45739aeff373b2d5514934d` |
| `mcps.js` | lazy | 143 | 131 | `17314bdb10376c4b5122df86c43f317b8bd692e2bdf8010c63c3531d37e2af90` |
| `memory.js` | lazy | 23767 | 7743 | `f2541e2b616c60a9c6251ba1894a42dca98af6641697296d65ffd85def733d6f` |
| `memory2.js` | lazy | 139 | 126 | `9711d134100ce6e7a0a175ebe8b4f67e5d2ca7065a3f78a45c65c7897d566806` |
| `noema-mark.svg` | asset | 379 | 235 | `02599d2fa5fdbae1b35b2cfe0cc04d4bda50e9c997803631bd854139c793b2d3` |
| `providers.js` | lazy | 149 | 135 | `94569645beffdc4f284af5ec1f6e06b7d6158f4671fe9353873ebb388b00a301` |
| `settings.js` | lazy | 92 | 102 | `e3afaf5ccd87230fa09b518f3004470e9a130da12a8859610d830b39fadf74fe` |
| `styles.css` | eager | 152877 | 26501 | `fbd8e833e8216c71c9f474f7705391ee75771e218f479f5fc944b3178edbdd88` |
| `usage.js` | lazy | 145 | 131 | `9ab1724c526f5c4a577165c977e2325ea6c16e9e953d7c2a81d57285d5cf4b50` |
| `web.js` | lazy | 142 | 131 | `6ec78b10ccdf0bddd15fd3937647d3fa175b32e5ed099a3d83f7f5ee17488b79` |

## Command evidence

Every command records its exit status, elapsed time, allowlisted relevant environment, and stdout/stderr or artifact hashes in `foundation.json`.

| # | Command | Exit | Seconds | Relevant environment |
| ---: | --- | ---: | ---: | --- |
| 1 | `sccache --show-stats --stats-format json` | 0 | 0.017986 | `—` |
| 2 | `sysctl -n machdep.cpu.brand_string` | 0 | 0.003225 | `—` |
| 3 | `rustc -Vv` | 0 | 0.134757 | `—` |
| 4 | `cargo -V` | 0 | 0.036394 | `—` |
| 5 | `bun -v` | 0 | 0.016473 | `—` |
| 6 | `python3 --version` | 0 | 0.013431 | `—` |
| 7 | `swift --version` | 0 | 2.172011 | `—` |
| 8 | `xcodebuild -version` | 0 | 0.088385 | `—` |
| 9 | `cargo tauri --version` | 0 | 0.055756 | `—` |
| 10 | `git rev-parse HEAD` | 0 | 0.023649 | `—` |
| 11 | `git status --short --untracked-files=all` | 0 | 0.026799 | `—` |
| 12 | `bun run build` | 0 | 4.029873 | `—` |
| 13 | `cargo build --workspace --timings` | 0 | 404.348592 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/clean-workspace-build-1-3252d4ad0424` |
| 14 | `cargo build --workspace --timings` | 0 | 600.245011 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/clean-workspace-build-2-3252d4ad0424` |
| 15 | `cargo build --workspace --timings` | 0 | 545.101427 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/clean-workspace-build-3-3252d4ad0424` |
| 16 | `cargo build --workspace --timings` | 0 | 393.803494 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/no-op-workspace-build-3252d4ad0424` |
| 17 | `cargo build --workspace --timings` | 0 | 3.111890 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/no-op-workspace-build-3252d4ad0424` |
| 18 | `cargo build --workspace --timings` | 0 | 0.701630 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/no-op-workspace-build-3252d4ad0424` |
| 19 | `cargo build --workspace --timings` | 0 | 0.519490 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/no-op-workspace-build-3252d4ad0424` |
| 20 | `cargo check --workspace --timings` | 0 | 164.156215 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 21 | `cargo check --workspace --timings` | 0 | 3.508565 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 22 | `cargo check --workspace --timings` | 0 | 4.642386 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 23 | `cargo check --workspace --timings` | 0 | 5.467828 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 24 | `cargo check --workspace --timings` | 0 | 5.899912 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 25 | `cargo check --workspace --timings` | 0 | 4.245228 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 26 | `cargo check --workspace --timings` | 0 | 4.301040 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 27 | `cargo check --workspace --timings` | 0 | 5.961554 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 28 | `cargo check --workspace --timings` | 0 | 4.205629 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 29 | `cargo check --workspace --timings` | 0 | 4.761491 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 30 | `cargo check --workspace --timings` | 0 | 4.804395 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 31 | `cargo check --workspace --timings` | 0 | 5.198600 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 32 | `cargo check --workspace --timings` | 0 | 4.996993 | `CARGO_TARGET_DIR=.local/remediation-baselines/foundation/targets/incremental-checks-3252d4ad0424` |
| 33 | `sccache --show-stats --stats-format json` | 0 | 0.017961 | `—` |
