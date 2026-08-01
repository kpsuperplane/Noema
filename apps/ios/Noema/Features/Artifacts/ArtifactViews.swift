import Apollo
import Foundation
import QuickLook
import SwiftUI
import MarkdownUI
import NoemaAPI
import Observation

struct ArtifactSelection: Identifiable, Equatable {
  let versionID: String
  let title: String

  var id: String { versionID }
}

struct ArtifactVersionModel: Identifiable, Equatable {
  let id: String
  let index: Int
  let downloadURL: URL?
  let externalURL: URL?
  let mediaType: String?
}

struct ArtifactDetailModel: Equatable {
  let id: String
  let artifactID: String
  let title: String
  let kind: String
  let mediaType: String?
  let previewKind: String
  let markdown: String?
  let plainText: String?
  let downloadURL: URL?
  let externalURL: URL?
  let versions: [ArtifactVersionModel]
}

@MainActor
@Observable
final class ArtifactModel {
  enum State: Equatable {
    case idle
    case loading
    case loaded
    case failed(String)
  }

  let client: ApolloClient?
  let profile: NoemaProfile?
  private(set) var state: State = .idle
  private(set) var detail: ArtifactDetailModel?

  init(client: ApolloClient?, profile: NoemaProfile?) {
    self.client = client
    self.profile = profile
  }

  func load(versionID: String) async {
    guard let client else {
      state = .failed("Connect this device to preview artifacts.")
      return
    }
    state = .loading
    do {
      let response = try await client.fetch(
        query: NoemaAPI.ArtifactVersionDetailQuery(artifactVersionId: versionID),
        cachePolicy: .networkFirst
      )
      guard let value = response.data?.artifactVersionDetail else {
        throw ArtifactError.missing
      }
      detail = ArtifactDetailModel(
        id: value.artifactVersionId,
        artifactID: value.artifactId,
        title: value.title,
        kind: value.artifactKind,
        mediaType: value.mediaType,
        previewKind: value.previewKind.rawValue,
        markdown: value.markdown,
        plainText: value.plainText,
        downloadURL: resolvedURL(value.downloadUrl),
        externalURL: resolvedURL(value.externalUrl),
        versions: value.versions.map {
          ArtifactVersionModel(
            id: $0.artifactVersionId,
            index: $0.versionIndex,
            downloadURL: resolvedURL($0.downloadUrl),
            externalURL: resolvedURL($0.externalUrl),
            mediaType: $0.mediaType
          )
        }
      )
      state = .loaded
    } catch {
      state = .failed(error.localizedDescription)
    }
  }

  func download(_ url: URL?) async throws -> URL {
    guard let url else { throw ArtifactError.unavailable }
    var request = URLRequest(url: url)
    if isTrustedOrigin(url), let token = profile?.token {
      request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }
    let session = URLSession(
      configuration: .ephemeral,
      delegate: ArtifactDownloadDelegate(origin: profile?.origin, requiresSameOrigin: request.value(forHTTPHeaderField: "Authorization") != nil),
      delegateQueue: nil
    )
    defer { session.invalidateAndCancel() }
    let (temporaryURL, response) = try await session.download(for: request)
    if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
      throw ArtifactError.http(http.statusCode)
    }
    let extensionName = url.pathExtension.isEmpty ? "bin" : url.pathExtension
    let destination = FileManager.default.temporaryDirectory
      .appendingPathComponent("noema-artifact-\(UUID().uuidString).\(extensionName)")
    try? FileManager.default.removeItem(at: destination)
    try FileManager.default.moveItem(at: temporaryURL, to: destination)
    return destination
  }

  private func resolvedURL(_ value: String?) -> URL? {
    guard let value, !value.isEmpty else { return nil }
    if let absolute = URL(string: value), absolute.scheme != nil {
      guard ["http", "https"].contains(absolute.scheme?.lowercased()) else { return nil }
      return absolute
    }
    guard let origin = profile?.origin else { return nil }
    let resolved = URL(string: value, relativeTo: origin)?.absoluteURL
    guard ["http", "https"].contains(resolved?.scheme?.lowercased()) else { return nil }
    return resolved
  }

  private func isTrustedOrigin(_ url: URL) -> Bool {
    guard let origin = profile?.origin,
          let originScheme = origin.scheme?.lowercased(),
          let targetScheme = url.scheme?.lowercased(),
          originScheme == "https",
          originScheme == targetScheme,
          origin.host?.lowercased() == url.host?.lowercased()
    else { return false }
    return effectivePort(origin) == effectivePort(url)
  }

  private func effectivePort(_ url: URL) -> Int? {
    if let port = url.port { return port }
    switch url.scheme?.lowercased() {
    case "https": return 443
    case "http": return 80
    default: return nil
    }
  }
}

struct ArtifactReferenceView: View {
  let reference: ArtifactReferenceModel
  let onOpen: () -> Void
  @Environment(\.openURL) private var openURL

  var body: some View {
    Button {
      if reference.versionID != nil {
        onOpen()
      } else if let externalURL = reference.externalURL {
        openURL(externalURL)
      }
    } label: {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: iconName)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.pine700)
          .frame(width: 32, height: 32)
          .background(NoemaColor.paper100, in: RoundedRectangle(cornerRadius: NoemaRadius.element, style: .continuous))
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(reference.title)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.content)
            .multilineTextAlignment(.leading)
            .lineLimit(2)
          Text(description)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(1)
        }
        Spacer(minLength: NoemaSpacing.sm)
        Image(systemName: reference.versionID == nil ? "arrow.up.right" : "rectangle.and.text.magnifyingglass")
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      .frame(maxWidth: 520, alignment: .leading)
      .padding(NoemaSpacing.sm)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element, style: .continuous))
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.element, style: .continuous)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .buttonStyle(.plain)
    .disabled(!actionable)
    .opacity(actionable ? 1 : 0.72)
  }

  private var actionable: Bool { reference.versionID != nil || reference.externalURL != nil }

  private var description: String {
    [humanize(reference.kind), humanize(reference.mediaType)]
      .compactMap { $0 }
      .joined(separator: " · ")
  }

  private var iconName: String {
    let mediaType = reference.mediaType?.lowercased() ?? ""
    if mediaType.hasPrefix("image/") { return "photo" }
    if mediaType.hasPrefix("video/") { return "video" }
    if mediaType.hasPrefix("audio/") { return "waveform" }
    if mediaType.contains("json") { return "curlybraces" }
    if mediaType.contains("pdf") || mediaType.hasPrefix("text/") { return "doc.text" }
    if reference.storageKind == "external_url" || reference.externalURL != nil { return "link" }
    return "doc.richtext"
  }

  private func humanize(_ value: String?) -> String? {
    guard let value, !value.isEmpty else { return nil }
    if value.lowercased() == "text/markdown" { return "Markdown" }
    if value.lowercased() == "text/plain" { return "Plain text" }
    if value.lowercased() == "application/json" { return "JSON" }
    if value.lowercased() == "application/pdf" { return "PDF" }
    return value
      .replacingOccurrences(of: "_", with: " ")
      .replacingOccurrences(of: "-", with: " ")
      .split(separator: " ")
      .map { $0.prefix(1).uppercased() + $0.dropFirst().lowercased() }
      .joined(separator: " ")
  }
}

struct ArtifactVersionSheet: View {
  @Environment(\.dismiss) private var dismiss
  @State private var model: ArtifactModel
  let selection: ArtifactSelection
  @State private var previewURL: URL?
  @State private var shareURLValue: URL?
  @State private var isDownloading = false
  @State private var actionError: String?

  init(model: ArtifactModel, selection: ArtifactSelection) {
    _model = State(initialValue: model)
    self.selection = selection
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack(spacing: NoemaSpacing.sm) {
        Text(selection.title)
          .font(NoemaFont.mobileTitle)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
        Spacer(minLength: NoemaSpacing.sm)
        if isDownloading {
          ProgressView().controlSize(.small)
        } else if let detail = model.detail, detail.downloadURL != nil {
          Button { beginPreview(detail.downloadURL) } label: {
            Image(systemName: "arrow.down")
              .frame(width: 32, height: 32)
          }
          .buttonStyle(.plain)
          .accessibilityLabel("Download")
        }
        Button { dismiss() } label: {
          Image(systemName: "xmark")
            .frame(width: 32, height: 32)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Close")
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .frame(height: 53)

      if let actionError {
        NoemaInlineState(message: actionError, symbol: "exclamationmark.triangle", tone: .warning)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
      }

      Group {
        switch model.state {
        case .idle, .loading:
          ProgressView("Loading artifact…")
        case .loaded:
          if let detail = model.detail { ArtifactDetailView(detail: detail, preview: beginPreview, shareURL: shareURL) }
          else { Text("The artifact is no longer available.").foregroundStyle(NoemaColor.contentSecondary) }
        case let .failed(message):
          ContentUnavailableView {
            Label("Preview unavailable", systemImage: "doc.questionmark")
          } description: {
            Text(message)
          }
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .background(NoemaColor.surface)
      .sheet(isPresented: Binding(
        get: { previewURL != nil },
        set: { if !$0 { previewURL = nil } }
      )) {
        if let previewURL {
          ArtifactQuickLookView(url: previewURL)
        }
      }
      .sheet(isPresented: Binding(
        get: { shareURLValue != nil },
        set: { if !$0 { shareURLValue = nil } }
      )) {
        if let shareURLValue {
          ArtifactShareSheet(url: shareURLValue)
        }
      }
    }
    .background(NoemaColor.surface)
    .presentationDetents([.large])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
    .task(id: selection.versionID) { await model.load(versionID: selection.versionID) }
  }

  private func beginPreview(_ url: URL?) {
    guard !isDownloading else { return }
    actionError = nil
    isDownloading = true
    Task {
      do {
        previewURL = try await model.download(url)
      } catch {
        actionError = error.localizedDescription
      }
      isDownloading = false
    }
  }

  private func shareURL(_ url: URL?) {
    actionError = nil
    guard let url else {
      actionError = ArtifactError.unavailable.localizedDescription
      return
    }
    Task {
      do { shareURLValue = try await model.download(url) }
      catch { actionError = error.localizedDescription }
    }
  }
}

private struct ArtifactDetailView: View {
  let detail: ArtifactDetailModel
  let preview: (URL?) -> Void
  let shareURL: (URL?) -> Void

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        Text("\(detail.kind) · \(detail.previewKind.replacingOccurrences(of: "_", with: " ").capitalized)")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        if let markdown = detail.markdown {
          Markdown(markdown)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else if let plainText = detail.plainText {
          Text(plainText)
            .font(NoemaFont.mono)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
          Label("This artifact does not have an inline preview.", systemImage: "eye.slash")
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        HStack(spacing: NoemaSpacing.sm) {
          if detail.downloadURL != nil {
            Button("Preview / download", systemImage: "arrow.down.doc") { preview(detail.downloadURL) }
              .buttonStyle(.borderedProminent)
            Button("Share", systemImage: "square.and.arrow.up") { shareURL(detail.downloadURL) }
              .buttonStyle(.bordered)
          }
          if let externalURL = detail.externalURL {
            Link(destination: externalURL) {
              Label("Open source", systemImage: "arrow.up.right.square")
            }
            .buttonStyle(.bordered)
          }
        }
        if !detail.versions.isEmpty {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Versions").font(NoemaFont.bodyEmphasized)
            ForEach(detail.versions) { version in
              HStack {
                Text("Version \(version.index)").font(NoemaFont.caption)
                Spacer()
                if let url = version.externalURL {
                  Link("Open", destination: url)
                } else if version.downloadURL != nil {
                  Button("Preview") { preview(version.downloadURL) }
                }
              }
              .padding(.vertical, NoemaSpacing.xxs)
            }
          }
        }
      }
      .padding(NoemaSpacing.lg)
    }
  }
}

private struct ArtifactQuickLookView: UIViewControllerRepresentable {
  let url: URL

  func makeCoordinator() -> Coordinator { Coordinator(url: url) }

  func makeUIViewController(context: Context) -> QLPreviewController {
    let controller = QLPreviewController()
    controller.dataSource = context.coordinator
    return controller
  }

  func updateUIViewController(_ controller: QLPreviewController, context: Context) {}

  final class Coordinator: NSObject, QLPreviewControllerDataSource {
    let item: ArtifactPreviewItem

    init(url: URL) { item = ArtifactPreviewItem(url: url) }

    func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }
    func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem { item }
  }
}

private struct ArtifactShareSheet: UIViewControllerRepresentable {
  let url: URL

  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: [url], applicationActivities: nil)
  }

  func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}

private final class ArtifactPreviewItem: NSObject, QLPreviewItem {
  let previewItemURL: URL?

  init(url: URL) { previewItemURL = url }
}

private final class ArtifactDownloadDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
  let origin: URL?
  let requiresSameOrigin: Bool

  init(origin: URL?, requiresSameOrigin: Bool) {
    self.origin = origin
    self.requiresSameOrigin = requiresSameOrigin
  }

  func urlSession(
    _ session: URLSession,
    task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) {
    guard requiresSameOrigin else {
      completionHandler(request)
      return
    }
    guard let origin, let redirected = request.url, sameOrigin(origin, redirected) else {
      completionHandler(nil)
      return
    }
    var safeRequest = request
    safeRequest.setValue(task.currentRequest?.value(forHTTPHeaderField: "Authorization"), forHTTPHeaderField: "Authorization")
    completionHandler(safeRequest)
  }

  private func sameOrigin(_ lhs: URL, _ rhs: URL) -> Bool {
    guard lhs.scheme?.lowercased() == rhs.scheme?.lowercased(),
          lhs.host?.lowercased() == rhs.host?.lowercased()
    else { return false }
    return effectivePort(lhs) == effectivePort(rhs)
  }

  private func effectivePort(_ url: URL) -> Int? {
    if let port = url.port { return port }
    switch url.scheme?.lowercased() {
    case "https": return 443
    case "http": return 80
    default: return nil
    }
  }
}

private enum ArtifactError: LocalizedError {
  case missing
  case unavailable
  case http(Int)

  var errorDescription: String? {
    switch self {
    case .missing: "Noema could not find this artifact version."
    case .unavailable: "This artifact is not available for download."
    case let .http(status): "The artifact server returned HTTP \(status)."
    }
  }
}
