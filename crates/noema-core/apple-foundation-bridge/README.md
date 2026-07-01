# Noema Foundation Bridge

This Swift package is the macOS-only helper process for the `foundation_local`
provider. It is not a Cargo workspace member and is not required for Linux or
Windows builds.

Manual development build on a supported macOS toolchain:

```bash
cd crates/noema-core/apple-foundation-bridge
swift build
```

The Rust daemon owns this process lifecycle. Users should not start this bridge
manually in normal Noema use.
