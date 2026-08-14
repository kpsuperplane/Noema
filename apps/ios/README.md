# Noema for iPhone and iPad

`apps/ios` is the native SwiftUI client for iOS and iPadOS 26 or newer. The
`Noema` scheme builds the `dev.noema.app.ios` application and uses the shared
GraphQL schema at `../../graphql/schema.graphql`.

## Requirements

- Xcode with the iOS 26 SDK or newer
- A Noema server exposed through a system-trusted HTTPS `web.public_origin`
- Apollo iOS 2.3.0 and MarkdownUI 2.4.1, resolved through Swift Package Manager

The development server at `http://127.0.0.1:3737` is useful for validating the
web shell, but it intentionally cannot issue native pairing links. Pairing is
available only when Noema has a non-localhost trusted-HTTPS public origin.

## Generate the GraphQL API

Apollo operation documents live in `Noema/Operations`; generated Swift sources
are committed under `Noema/Generated`.

```sh
cd apps/ios
apollo-ios-cli generate --path apollo-codegen-config.json
```

Regeneration should leave the worktree clean unless the shared schema or native
operation documents changed. Do not edit files under `Noema/Generated` by hand.

## Generate the shared icons

The web package pins the static Lucide source used to generate iOS vector
assets and the typed Swift icon names.

```sh
cd apps/web
bun run gen:ios-icons
```

Do not edit `NoemaIcon+Generated.swift` or the `Lucide` asset group by hand.

## Build

```sh
xcodebuild \
  -project apps/ios/Noema.xcodeproj \
  -scheme Noema \
  -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO \
  build
```

The app stores one active origin, client identifier, and bearer credential in
Keychain using `WhenUnlockedThisDeviceOnly`. Apollo stores normalized reads in
a protected per-client SQLite cache that is excluded from backup. Mutations are
disabled while disconnected. Subscriptions pause in the background, and
foreground recovery refetches durable transcript and Tasks event cursors before
accepting later live events.

Pair from Settings → System → Clients in the authenticated web app. The native
app accepts the resulting `noema://pair` URI from VisionKit scanning, a deep
link, or the pasteboard. Disconnecting removes the local Keychain profile;
revoking a client from either Settings surface invalidates its HTTP, WebSocket,
and artifact access on the server.

## Task Live Activities

Noema shows one aggregate Tasks Live Activity for the active task set. The
server starts, updates, and ends it through direct APNs. The widget extension
does not share the paired credential or run GraphQL requests.

Enable Push Notifications for `dev.noema.app.ios` before a signed device build.
Configure the server APNs provider in browser Settings. Live Activity pushes use
the fixed topic `dev.noema.app.ios.push-type.liveactivity`.

Settings → Notifications includes a local Live Activity test when the feature
is enabled. The test bypasses APNs and does not affect server reconciliation.
Real ActivityKit sessions report snapshots through the existing paired-client
registration. Device traces include states and identifiers, but never tokens.
