# Noema for iPhone and iPad

`apps/ios` is the native SwiftUI client for iOS and iPadOS 26 or newer. The
`Noema` scheme builds the `dev.noema.app.ios` application and uses the shared
GraphQL schema at `../../graphql/schema.graphql`.

## Requirements

- Xcode with the iOS 26 SDK or newer
- A Noema server exposed through a system-trusted HTTPS `web.public_origin`
- Apollo iOS 2.3.0 and MarkdownUI 2.4.1, resolved through Swift Package Manager

The development server at `http://127.0.0.1:3737` is useful for validating the
web shell. It cannot create native connection links. Native OAuth requires a
non-localhost, system-trusted HTTPS public origin.

## Generate the GraphQL API

Apollo operation documents live in `Noema/Operations`; generated Swift sources
are committed under `Noema/Generated`.

The Xcode project locks Apollo iOS 2.3.0. Open the project and resolve its Swift
packages. In the project navigator, right-click the Noema project and run the
`Install CLI` package plugin. Grant its write and network permissions. The
plugin installs the matching CLI at `apps/ios/apollo-ios-cli`.

```sh
cd apps/ios
./apollo-ios-cli generate --path apollo-codegen-config.json
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

The app stores one origin, client identifier, and rotating refresh credential
in Keychain. It uses `WhenUnlockedThisDeviceOnly`. Access tokens remain in
memory. Before refresh, the app saves a request identifier with the current
refresh credential. An interrupted refresh reuses both saved values.

One connection service owns access state, refresh timing, and refresh
serialization. Refresh work runs only while the app is active and the Keychain
is available. HTTP requests read the current access token. WebSocket headers
change in place, so rotation does not rebuild Apollo or its subscriptions.

Apollo stores normalized reads in a protected per-client SQLite cache.
The cache filename includes a client-schema version, so incompatible normalized data is removed.
The cache is excluded from backup. Mutations are disabled while disconnected.
Subscriptions pause in the background. Foreground recovery refetches durable
transcript and Tasks event cursors before it accepts later live events.

An unsatisfied system network path pauses subscription and health-check retries.
The next usable path resumes them immediately. Server failures retry after 1,
2, 4, 8, 16, and then 30 seconds. The 30-second delay remains until recovery.

The shell distinguishes a missing network path from an unavailable Noema
server. Its banner remains visible until the authenticated WebSocket and a
network-only GraphQL health check both succeed.

Create a connection link in Settings → System → Clients. The link contains
only the server origin. The native app accepts the `noema://connect` URI from
VisionKit, a deep link, or the pasteboard. The app opens the system browser for
OAuth. Authorization requires PKCE and recent passkey approval. Disconnect
revokes the OAuth family before the app removes its Keychain profile.

## Task Live Activities

Noema shows one aggregate Tasks Live Activity for the active task set. The
server starts, updates, and ends it through direct APNs. The widget extension
does not hold a native OAuth credential or run GraphQL requests.

Enable Push Notifications for `dev.noema.app.ios` before a signed device build.
Register and provision the `dev.noema.app.ios.liveactivity` extension App ID.
Both targets need valid signing profiles. Configure the server APNs provider in
browser Settings. Live Activity pushes use the fixed topic
`dev.noema.app.ios.push-type.liveactivity`.

Settings → Notifications includes a local Live Activity test when the feature
is enabled. The test bypasses APNs and does not affect server reconciliation.
Real ActivityKit sessions report snapshots through the native-client
registration. Device traces include states and identifiers, but never tokens.

Implementation: [connection and refresh](Noema/Features/Pairing/PairingService.swift), [GraphQL transport](Noema/Core/Networking/NoemaGraphQLClient.swift), and [network recovery](Noema/Core/Networking/NoemaConnectionRecovery.swift).
