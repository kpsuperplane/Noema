# Noema Foundation Bridge

This Swift package is the macOS-only helper process for the `foundation_local`
provider. It is not a Cargo workspace member and is not required for Linux or
Windows builds.

In local source builds, the Rust `foundation_local` provider runs `swift build`
on demand when the default source-tree bridge binary is missing.

Manual development build on a supported macOS toolchain:

```bash
swift build --package-path crates/noema-providers/apple-foundation-bridge
```

The Rust provider adapter owns this process lifecycle. Users should not start
this bridge manually in normal Noema use.
