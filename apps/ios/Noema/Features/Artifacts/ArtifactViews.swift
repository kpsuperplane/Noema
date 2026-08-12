import Apollo
import Foundation
import QuickLook
import SwiftUI
import MarkdownUI
import NoemaAPI
import Observation
import UniformTypeIdentifiers

struct ArtifactSelection: Identifiable, Equatable {
  let versionID: String
  let title: String
  var backTitle: String? = nil

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

enum ArtifactLinkResolver {
  static func detailVersionID(storageKind: String, versionID: String?) -> String? {
    guard storageKind.lowercased() == "local_file",
          let value = versionID?.trimmingCharacters(in: .whitespacesAndNewlines),
          !value.isEmpty
    else { return nil }
    return value
  }

  static func externalURL(_ value: String?) -> URL? {
    guard let value,
          let url = URL(string: value),
          ["http", "https"].contains(url.scheme?.lowercased()),
          url.host != nil
    else { return nil }
    return url
  }

  static func downloadURL(_ value: String?, origin: URL?) -> URL? {
    guard let value, !value.isEmpty,
          let components = URLComponents(string: value),
          components.scheme == nil,
          components.host == nil,
          value.hasPrefix("/"),
          trustedDownloadPath(components.path),
          let origin,
          origin.scheme?.lowercased() == "https"
    else { return nil }
    return URL(string: value, relativeTo: origin)?.absoluteURL
  }

  static func isTrustedDownloadURL(_ url: URL, origin: URL?) -> Bool {
    guard let origin else { return false }
    return sameOrigin(origin, url) && trustedDownloadPath(url.path)
  }

  static func sameOrigin(_ lhs: URL, _ rhs: URL) -> Bool {
    guard lhs.scheme?.lowercased() == "https",
          lhs.scheme?.lowercased() == rhs.scheme?.lowercased(),
          lhs.host?.lowercased() == rhs.host?.lowercased()
    else { return false }
    return effectivePort(lhs) == effectivePort(rhs)
  }

  private static func trustedDownloadPath(_ path: String) -> Bool {
    let segments = path.split(separator: "/", omittingEmptySubsequences: false)
    if segments.count == 4 {
      return segments[0].isEmpty
        && segments[1] == "artifacts"
        && !segments[2].isEmpty
        && !segments[2].contains(":")
        && segments[3] == "download"
    }
    return segments.count == 5
      && segments[0].isEmpty
      && segments[1] == "artifacts"
      && segments[2] == "versions"
      && !segments[3].isEmpty
      && !segments[3].contains(":")
      && segments[4] == "download"
  }

  private static func effectivePort(_ url: URL) -> Int? {
    if let port = url.port { return port }
    return url.scheme?.lowercased() == "https" ? 443 : nil
  }
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
      let response = try await client.fetchNetworkFirst(
        query: NoemaAPI.ArtifactVersionDetailQuery(artifactVersionId: versionID)
      )
      if let message = response.errors?.first?.message {
        throw ArtifactError.server(message)
      }
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
        downloadURL: ArtifactLinkResolver.downloadURL(value.downloadUrl, origin: profile?.origin),
        externalURL: ArtifactLinkResolver.externalURL(value.externalUrl),
        versions: value.versions.map {
          ArtifactVersionModel(
            id: $0.artifactVersionId,
            index: $0.versionIndex,
            downloadURL: ArtifactLinkResolver.downloadURL($0.downloadUrl, origin: profile?.origin),
            externalURL: ArtifactLinkResolver.externalURL($0.externalUrl),
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
    guard let url,
          ArtifactLinkResolver.isTrustedDownloadURL(url, origin: profile?.origin),
          let token = profile?.token
    else {
      throw ArtifactError.untrustedDownload
    }
    var request = URLRequest(url: url)
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    let session = URLSession(
      configuration: .ephemeral,
      delegate: ArtifactDownloadDelegate(origin: profile?.origin),
      delegateQueue: nil
    )
    defer { session.invalidateAndCancel() }
    let (temporaryURL, response) = try await session.download(for: request)
    if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
      throw ArtifactError.http(http.statusCode)
    }
    guard let responseURL = response.url, isTrustedOrigin(responseURL) else {
      throw ArtifactError.untrustedDownload
    }
    let extensionName = downloadExtension(response: response)
    let destination = FileManager.default.temporaryDirectory
      .appendingPathComponent("noema-artifact-\(UUID().uuidString).\(extensionName)")
    try? FileManager.default.removeItem(at: destination)
    try FileManager.default.moveItem(at: temporaryURL, to: destination)
    return destination
  }

  private func downloadExtension(response: URLResponse) -> String {
    if let filename = response.suggestedFilename {
      let pathExtension = URL(fileURLWithPath: filename).pathExtension
      if !pathExtension.isEmpty { return pathExtension }
    }
    if let mimeType = response.mimeType,
       let value = UTType(mimeType: mimeType)?.preferredFilenameExtension {
      return value
    }
    return "bin"
  }

  private func isTrustedOrigin(_ url: URL) -> Bool {
    guard let origin = profile?.origin else { return false }
    return ArtifactLinkResolver.sameOrigin(origin, url)
  }
}

struct ArtifactReferenceView: View {
  let reference: ArtifactReferenceModel
  let onOpen: () -> Void
  @Environment(\.openURL) private var openURL

  var body: some View {
    Button {
      if opensDetail {
        onOpen()
      } else if let externalURL = reference.externalURL {
        openURL(externalURL)
      }
    } label: {
      ViewThatFits(in: .horizontal) {
        referenceCard.frame(minWidth: 220, maxWidth: 520, minHeight: 57, alignment: .leading)
        referenceCard.frame(maxWidth: .infinity, minHeight: 57, alignment: .leading)
      }
    }
    .buttonStyle(.plain)
    .disabled(!actionable)
    .opacity(actionable ? 1 : 0.72)
  }

  private var opensDetail: Bool {
    ArtifactLinkResolver.detailVersionID(storageKind: reference.storageKind, versionID: reference.versionID) != nil
  }

  private var actionable: Bool { opensDetail || reference.externalURL != nil }

  private var referenceCard: some View {
    ZStack(alignment: .bottomTrailing) {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: iconName)
          .font(.system(size: 18))
          .foregroundStyle(NoemaColor.pine700)
          .frame(width: 32, height: 32)
          .background(NoemaColor.paper100, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
        VStack(alignment: .leading, spacing: 0) {
          Text(reference.title)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
            .multilineTextAlignment(.leading)
            .lineLimit(2)
            .frame(minHeight: 20, alignment: .leading)
          Text(description)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(1)
            .frame(height: 20, alignment: .leading)
        }
      }
      .padding(.leading, NoemaSpacing.sm)
      .padding(.trailing, 61)

      Image(systemName: opensDetail ? "rectangle.and.text.magnifyingglass" : "arrow.up.right")
        .font(.system(size: 14, weight: .medium))
        .foregroundStyle(NoemaColor.contentTertiary)
        .frame(width: 28, height: 28)
        .padding(NoemaSpacing.sm)
    }
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
  }

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
  @State private var selectedVersionID: String

  init(model: ArtifactModel, selection: ArtifactSelection) {
    _model = State(initialValue: model)
    _selectedVersionID = State(initialValue: selection.versionID)
    self.selection = selection
  }

  var body: some View {
    NoemaNativeSheet(
      title: model.detail?.title ?? selection.title,
      dismissControl: selection.backTitle == nil ? .close : .text("Back"),
      onDismiss: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: 0) {
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
            if let detail = model.detail {
              ArtifactDetailView(
                detail: detail,
                selectedVersionID: selectedVersionID,
                selectVersion: { versionID in
                  guard versionID != model.detail?.id else { return }
                  selectedVersionID = versionID
                  Task { await model.load(versionID: versionID) }
                },
                preview: beginPreview,
                shareURL: shareURL
              )
            }
            else { Text("Artifact unavailable").foregroundStyle(NoemaColor.contentSecondary) }
          case let .failed(message):
            NoemaDeckState(title: "Preview unavailable", message: message, symbol: "doc.questionmark", tone: .warning)
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
      .toolbar {
        ToolbarItem(placement: .confirmationAction) {
          if isDownloading {
            ProgressView().controlSize(.small)
          } else if let detail = model.detail, detail.downloadURL != nil {
            Button("Download", systemImage: "arrow.down") { beginPreview(detail.downloadURL) }
              .labelStyle(.iconOnly)
          }
        }
      }
    }
    .noemaMobileDrawerPresentation()
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
  let selectedVersionID: String
  let selectVersion: (String) -> Void
  let preview: (URL?) -> Void
  let shareURL: (URL?) -> Void

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        HStack(spacing: NoemaSpacing.sm) {
          Text([detail.kind, detail.mediaType].compactMap { $0?.isEmpty == false ? $0 : nil }.joined(separator: " · "))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(2)
          Spacer(minLength: NoemaSpacing.sm)
          if !detail.versions.isEmpty {
            Picker("Artifact version", selection: Binding(
              get: { selectedVersionID },
              set: { selectVersion($0) }
            )) {
              ForEach(detail.versions) { version in
                Text("Version \(version.index)").tag(version.id)
              }
            }
            .pickerStyle(.menu)
            .disabled(detail.versions.count <= 1)
            .frame(width: 128)
          }
        }
        if detail.previewKind.uppercased() == "MARKDOWN", let markdown = detail.markdown {
          Markdown(markdown)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else if detail.previewKind.uppercased() == "PLAIN_TEXT", let plainText = detail.plainText {
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
              .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            Button("Share", systemImage: "square.and.arrow.up") { shareURL(detail.downloadURL) }
              .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
          }
          if let externalURL = detail.externalURL {
            Link(destination: externalURL) {
              Label("Open source", systemImage: "arrow.up.right.square")
            }
            .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
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

  init(origin: URL?) {
    self.origin = origin
  }

  func urlSession(
    _ session: URLSession,
    task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) {
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
  case untrustedDownload
  case http(Int)
  case server(String)

  var errorDescription: String? {
    switch self {
    case .missing: "Noema could not find this artifact version."
    case .unavailable: "This artifact is not available for download."
    case .untrustedDownload: "Noema rejected an untrusted artifact download link."
    case let .http(status): "The artifact server returned HTTP \(status)."
    case let .server(message): "Error loading artifact: \(message)"
    }
  }
}
