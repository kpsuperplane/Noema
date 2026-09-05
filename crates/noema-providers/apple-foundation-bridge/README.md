# Noema Foundation Bridge

This Swift package is the macOS-only helper process for the `foundation_local`
provider. It requires macOS 26, Swift 6, and Apple Foundation Models. It is not
a Cargo workspace member. Linux and Windows builds do not require it.

When `bridge_path` is unset, a debug Go build uses the source-tree bridge.
If that executable is missing, the provider runs `swift build` on demand.
Configured bridge paths and release builds are never built automatically.

On macOS, `cargo dev` watches `Package.swift` and `Sources`. It runs
`swift build` after a change.

Run a manual development build from the repository root:

```bash
swift build --package-path crates/noema-providers/apple-foundation-bridge
```

For a release build, place `noema-foundation-bridge` beside the Noema
executable. Alternatively, set `foundation_local.bridge_path` to its exact
path. The Go provider adapter starts and stops the process.
