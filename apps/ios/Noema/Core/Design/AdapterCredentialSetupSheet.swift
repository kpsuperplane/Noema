import SwiftUI
import UniformTypeIdentifiers

struct AdapterCredentialSetupSheet: View {
  let serviceName: String
  var title: String? = nil
  let setup: AdapterCredentialSetupModel
  let scopes: [String]
  var introduction = "Create the exact reviewed credential below. Noema stores only the declared private fields."
  var submitTitle = "Add connection"
  var dismissAfterSubmit = true
  let onClose: () -> Void
  let onSubmit: (AdapterCredentialSubmission) async throws -> Void

  @State private var values: [String: String] = [:]
  @State private var document: Data?
  @State private var documentName: String?
  @State private var fileImporterPresented = false
  @State private var isSubmitting = false
  @State private var errorMessage: String?
  @FocusState private var focusedField: String?

  private var isDocument: Bool { setup.inputKind == "document" }
  private var isComplete: Bool {
    if isDocument { return document != nil }
    return setup.fields.allSatisfy { values[$0.id]?.isEmpty == false }
  }

  var body: some View {
    NoemaNativeSheet(
      title: title ?? "Connect \(serviceName)",
      dismissControl: .text("Cancel"),
      dismissDisabled: isSubmitting,
      onDismiss: requestDismissal
    ) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Text(setup.credentialType)
              .font(NoemaFont.sectionTitle)
              .foregroundStyle(NoemaColor.content)
            Text(introduction)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          }

          if !setup.instructions.isEmpty {
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              ForEach(Array(setup.instructions.enumerated()), id: \.offset) { index, instruction in
                HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
                  Text("\(index + 1).")
                    .font(NoemaFont.captionEmphasized)
                    .foregroundStyle(NoemaColor.contentTertiary)
                  Text(instruction)
                    .font(NoemaFont.body)
                    .foregroundStyle(NoemaColor.content)
                }
              }
            }
          }

          if let setupURL = setup.setupURL {
            Link(destination: setupURL) {
              Label("Open official credential setup", systemImage: "arrow.up.right")
            }
            .font(NoemaFont.bodyEmphasized)
          }

          if let redirectURI = setup.redirectURI {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text("Authorized redirect URI").font(NoemaFont.captionEmphasized)
              Text("Copy this exact value into the provider client.")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              Text(redirectURI)
                .font(NoemaFont.monoTiny)
                .textSelection(.enabled)
                .padding(NoemaSpacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaRadius.inner))
            }
          }

          if isDocument {
            Button {
              fileImporterPresented = true
            } label: {
              HStack(spacing: NoemaSpacing.sm) {
                Image(systemName: document == nil ? "doc.badge.plus" : "checkmark.circle.fill")
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  Text(documentName ?? "Choose \(setup.credentialType) document")
                    .font(NoemaFont.bodyEmphasized)
                  Text("Processed once and not retained · 128 KB maximum")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                Spacer(minLength: 0)
              }
              .padding(NoemaSpacing.md)
              .frame(maxWidth: .infinity, alignment: .leading)
              .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
              .overlay {
                NoemaSuperellipse(cornerRadius: NoemaRadius.element)
                  .stroke(NoemaColor.separator, lineWidth: 1)
              }
            }
            .buttonStyle(.plain)
            .disabled(isSubmitting)
          } else {
            VStack(alignment: .leading, spacing: NoemaSpacing.md) {
              ForEach(setup.fields) { field in
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  Text(field.label).font(NoemaFont.captionEmphasized)
                  SecureField(field.label, text: valueBinding(for: field.id))
                    .textContentType(.password)
                    .focused($focusedField, equals: field.id)
                    .settingsSheetControl()
                    .disabled(isSubmitting)
                }
              }
            }
          }

          DisclosureGroup("Technical disclosure") {
            VStack(alignment: .leading, spacing: NoemaSpacing.md) {
              technicalSection(title: "Scopes", value: scopes.isEmpty ? "None" : scopes.joined(separator: "\n"))
              if let transform = setup.normalizationTransform {
                transformSection(title: "Credential normalization", transform: transform)
              }
              if let transform = setup.requestAuthTransform {
                transformSection(title: "Request authentication", transform: transform)
              }
            }
            .padding(.top, NoemaSpacing.sm)
          }
          .font(NoemaFont.captionEmphasized)

          if let errorMessage {
            NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .error)
          }

          HStack(spacing: NoemaSpacing.sm) {
            Spacer(minLength: 0)
            Button("Cancel", action: requestDismissal)
              .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
              .disabled(isSubmitting)
            Button {
              submit()
            } label: {
              if isSubmitting { ProgressView().controlSize(.small) }
              else { Text(submitTitle) }
            }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            .disabled(isSubmitting || !isComplete)
          }
        }
        .padding(NoemaSpacing.lg)
      }
    }
    .presentationDetents([.large])
    .presentationDragIndicator(.visible)
    .interactiveDismissDisabled(isSubmitting)
    .fileImporter(
      isPresented: $fileImporterPresented,
      allowedContentTypes: allowedContentTypes,
      allowsMultipleSelection: false,
      onCompletion: importDocument
    )
    .onAppear {
      if !isDocument { focusedField = setup.fields.first?.id }
    }
    .onDisappear(perform: clearSensitiveState)
  }

  private var allowedContentTypes: [UTType] {
    guard let mediaType = setup.documentMediaType,
          let type = UTType(mimeType: mediaType) else { return [.data] }
    return [type]
  }

  private func valueBinding(for fieldID: String) -> Binding<String> {
    Binding(get: { values[fieldID, default: ""] }, set: { values[fieldID] = $0 })
  }

  private func requestDismissal() {
    guard !isSubmitting else { return }
    clearSensitiveState()
    onClose()
  }

  private func clearSensitiveState() {
    values.removeAll(keepingCapacity: false)
    document = nil
    documentName = nil
    focusedField = nil
  }

  private func importDocument(_ result: Result<[URL], Error>) {
    guard case let .success(urls) = result, let url = urls.first else {
      if case let .failure(error) = result { errorMessage = error.localizedDescription }
      return
    }
    let accessed = url.startAccessingSecurityScopedResource()
    defer { if accessed { url.stopAccessingSecurityScopedResource() } }
    do {
      let data = try Data(contentsOf: url, options: .mappedIfSafe)
      guard !data.isEmpty, data.count <= 128 * 1024 else {
        throw AdapterCredentialError.invalidDocument
      }
      document = data
      documentName = url.lastPathComponent
      errorMessage = nil
    } catch {
      document = nil
      documentName = nil
      errorMessage = error.localizedDescription
    }
  }

  private func submit() {
    guard isComplete, !isSubmitting else { return }
    let submission = AdapterCredentialSubmission(
      fieldValues: isDocument
        ? []
        : setup.fields.map { AdapterCredentialValue(fieldID: $0.id, value: values[$0.id, default: ""]) },
      document: document
    )
    isSubmitting = true
    errorMessage = nil
    Task {
      do {
        try await onSubmit(submission)
        clearSensitiveState()
        if dismissAfterSubmit { onClose() }
      } catch {
        errorMessage = error.localizedDescription
      }
      isSubmitting = false
    }
  }

  @ViewBuilder
  private func technicalSection(title: String, value: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(title).font(NoemaFont.captionEmphasized)
      Text(value).font(NoemaFont.monoTiny).textSelection(.enabled)
    }
  }

  private func transformSection(title: String, transform: AdapterCredentialTransformModel) -> some View {
    technicalSection(
      title: "\(title) · \(transform.language) · \(transform.sourceDigest)",
      value: transform.source
    )
  }
}
