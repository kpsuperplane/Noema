import SwiftUI
import UIKit

struct PairingView: View {
  @Bindable var model: NoemaAppModel
  @State private var input = ""
  @State private var scannerPresented = false
  @State private var scannerDetent: PresentationDetent = .large
  @FocusState private var focusedField: Field?

  fileprivate enum Field: Hashable {
    case pairingLink
  }

  var body: some View {
    NavigationStack {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.xl) {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Label("Connect this device", systemImage: "link.badge.plus")
              .font(NoemaFont.title)
              .foregroundStyle(NoemaColor.content)
            Text("Authorize this device with your passkey in the system browser.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          }

          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            Text("Server address or connection link")
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.contentSecondary)
            TextField("https://noema.example", text: $input, axis: .vertical)
              .textInputAutocapitalization(.never)
              .autocorrectionDisabled()
              .font(NoemaFont.mono)
              .focused($focusedField, equals: .pairingLink)
              .padding(NoemaSpacing.md)
              .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaSpacing.sm))
              .overlay {
                NoemaSuperellipse(cornerRadius: NoemaSpacing.sm)
                  .stroke(NoemaColor.separator.opacity(0.45), lineWidth: 0.5)
              }
              .onChange(of: input) { _, value in
                model.ingestPairingText(value)
              }

            HStack(spacing: NoemaSpacing.sm) {
              Button("Use from Paste", systemImage: "doc.on.clipboard") {
                model.ingestPairingText(UIPasteboard.general.string ?? "")
                input = model.pairingInput
              }
              .buttonStyle(NoemaActionButtonStyle(variant: .secondary))

              Button("Scan", systemImage: "qrcode.viewfinder") {
                scannerPresented = true
              }
              .buttonStyle(NoemaActionButtonStyle(variant: .primary))
              .disabled(!QRScannerSheet.isAvailable)
            }
            .labelStyle(.titleAndIcon)
            .font(NoemaFont.captionEmphasized)
          }

          if let payload = model.pairingPayload {
            PairingConfirmation(payload: payload) {
              model.completePairing()
            }
          }

          if let error = model.pairingError {
            Label(error, systemImage: "exclamationmark.triangle")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.danger)
              .fixedSize(horizontal: false, vertical: true)
          }
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.vertical, NoemaSpacing.xxl)
        .frame(maxWidth: 640, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .center)
      }
      .scrollIndicators(.hidden)
      .background(NoemaColor.surface)
      .navigationTitle("Noema")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        if model.profile != nil {
          ToolbarItem(placement: .confirmationAction) {
            Button("Cancel") {
              model.cancelPairingReplacement()
            }
            .disabled(model.isPairing)
          }
        }
      }
    }
    .tint(NoemaColor.accent)
    .task {
      focusedField = .pairingLink
      input = model.pairingInput
    }
    .noemaSheet(isPresented: $scannerPresented) {
      NoemaNativeSheet(title: "Scan connection QR", onDismiss: { scannerPresented = false }) {
        QRScannerSheet { value in
          scannerPresented = false
          input = value
          model.ingestPairingText(value)
        }
      }
      .presentationDetents([.medium, .large], selection: $scannerDetent)
      .presentationDragIndicator(.visible)
      .interactiveDismissDisabled()
    }
  }
}

private struct PairingConfirmation: View {
  let payload: ConnectionPayload
  let onConnect: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Confirm server")
          .font(NoemaFont.bodyEmphasized)
        Text(payload.origin.displayValue)
          .font(NoemaFont.mono)
          .foregroundStyle(NoemaColor.contentSecondary)
          .textSelection(.enabled)
      }

      Text("The server will grant complete access to this Noema client.")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)

      Button("Connect", systemImage: "checkmark.circle.fill", action: onConnect)
        .buttonStyle(NoemaActionButtonStyle(variant: .primary))
    }
    .padding(NoemaSpacing.lg)
    .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaSpacing.md, treatment: .container))
    .accessibilityElement(children: .contain)
  }
}
