import SwiftUI
import UIKit

struct PairingView: View {
  @Bindable var model: NoemaAppModel
  @State private var input = ""
  @State private var displayName = UIDevice.current.name
  @State private var scannerPresented = false
  @State private var scannerDetent: PresentationDetent = .large
  @FocusState private var focusedField: Field?

  fileprivate enum Field: Hashable {
    case pairingLink
    case displayName
  }

  var body: some View {
    NavigationStack {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.xl) {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Label("Pair this device", systemImage: "link.badge.plus")
              .font(NoemaFont.title)
              .foregroundStyle(NoemaColor.content)
            Text("Connect to your Noema server with a short-lived pairing link.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          }

          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            Text("Pairing link")
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.contentSecondary)
            TextField("noema://pair…", text: $input, axis: .vertical)
              .textInputAutocapitalization(.never)
              .autocorrectionDisabled()
              .font(NoemaFont.mono)
              .focused($focusedField, equals: .pairingLink)
              .padding(NoemaSpacing.md)
              .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))
              .overlay {
                RoundedRectangle(cornerRadius: NoemaSpacing.sm)
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
              .buttonStyle(.bordered)

              Button("Scan", systemImage: "qrcode.viewfinder") {
                scannerPresented = true
              }
              .buttonStyle(.borderedProminent)
              .disabled(!QRScannerSheet.isAvailable)
            }
            .labelStyle(.titleAndIcon)
            .font(NoemaFont.captionEmphasized)
          }

          if let payload = model.pairingPayload {
            PairingConfirmation(payload: payload, displayName: $displayName, focusedField: $focusedField) {
              model.completePairing(displayName: displayName)
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
    .sheet(isPresented: $scannerPresented) {
      NoemaNativeSheet(title: "Scan pairing QR", onDismiss: { scannerPresented = false }) {
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
  let payload: PairingPayload
  @Binding var displayName: String
  var focusedField: FocusState<PairingView.Field?>.Binding
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

      TextField("Device name", text: $displayName)
        .textInputAutocapitalization(.words)
        .focused(focusedField, equals: .displayName)
        .padding(NoemaSpacing.md)
        .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))

      Button("Connect", systemImage: "checkmark.circle.fill", action: onConnect)
        .buttonStyle(.borderedProminent)
        .disabled(!NoemaDisplayName.isValid(displayName))
    }
    .padding(NoemaSpacing.lg)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.md))
    .accessibilityElement(children: .contain)
  }
}
