import SwiftUI

@main
struct NoemaApp: App {
  @State private var model = NoemaAppModel()
  @Environment(\.scenePhase) private var scenePhase

  var body: some Scene {
    WindowGroup {
      NoemaRootView(model: model)
        .preferredColorScheme(.light)
        .task {
          await model.bootstrap()
        }
        .onOpenURL { url in
          model.ingestPairingURL(url)
        }
    }
    .onChange(of: scenePhase) { _, phase in
      model.scenePhaseChanged(phase)
    }
  }
}

struct NoemaRootView: View {
  @Bindable var model: NoemaAppModel

  var body: some View {
    switch model.state {
    case .loading:
      ProgressView("Preparing Noema…")
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(NoemaColor.surface)
    case .unpaired:
      PairingView(model: model)
    case .paired:
      if model.pairingPayload != nil || (model.pairingError != nil && !model.pairingInput.isEmpty) {
        PairingView(model: model)
      } else if model.graphQLClient == nil {
        NoemaDisconnectedBoundary {
          model.disconnect()
        }
      } else {
        NoemaShellView(model: model)
      }
    }
  }
}

struct NoemaDisconnectedBoundary: View {
  let onReconnect: () -> Void

  var body: some View {
    ContentUnavailableView {
      Label("Disconnected", systemImage: "wifi.slash")
    } description: {
      Text("No active Noema connection is available on this device.")
    } actions: {
      Button("Pair again", action: onReconnect)
        .buttonStyle(.borderedProminent)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(NoemaColor.surface)
  }
}
