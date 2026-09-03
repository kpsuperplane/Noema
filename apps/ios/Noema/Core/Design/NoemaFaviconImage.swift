import Foundation
import SwiftUI
import UIKit

private actor NoemaFaviconLoader {
  static let shared = NoemaFaviconLoader()

  private let session: URLSession
  private var cachedData: [String: Data] = [:]
  private var cacheOrder: [String] = []
  private var requests: [String: Task<Data?, Never>] = [:]

  init() {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.urlCache = URLCache(
      memoryCapacity: 8 * 1024 * 1024,
      diskCapacity: 0
    )
    configuration.requestCachePolicy = .useProtocolCachePolicy
    session = URLSession(
      configuration: configuration,
      delegate: NoemaFaviconSessionDelegate(),
      delegateQueue: nil
    )
  }

  func data(hostname: String, profile: NoemaProfile) async -> Data? {
    let key = "\(profile.origin.absoluteString)#\(hostname.lowercased())"
    if let data = cachedData[key] { return data }
    if let request = requests[key] { return await request.value }

    let request = Task {
      await Self.fetch(hostname: hostname, profile: profile, session: session)
    }
    requests[key] = request
    let data = await request.value
    requests[key] = nil
    if let data {
      cachedData[key] = data
      cacheOrder.removeAll { $0 == key }
      cacheOrder.append(key)
      if cacheOrder.count > 256 {
        cachedData[cacheOrder.removeFirst()] = nil
      }
    }
    return data
  }

  private static func fetch(
    hostname: String,
    profile: NoemaProfile,
    session: URLSession
  ) async -> Data? {
    let url = profile.origin
      .appending(path: "favicons")
      .appending(path: hostname)
    var request = URLRequest(url: url)
    request.setValue("Bearer \(profile.accessToken)", forHTTPHeaderField: "Authorization")
    guard let (data, response) = try? await session.data(for: request),
          data.count <= 256 * 1024,
          let http = response as? HTTPURLResponse,
          http.statusCode == 200,
          http.mimeType == "image/png",
          trusted(response.url, matches: profile.origin),
          UIImage(data: data) != nil
    else { return nil }
    return data
  }

  private static func trusted(_ response: URL?, matches origin: URL) -> Bool {
    guard let response else { return false }
    return response.scheme?.lowercased() == origin.scheme?.lowercased()
      && response.host?.lowercased() == origin.host?.lowercased()
      && effectivePort(response) == effectivePort(origin)
  }

  private static func effectivePort(_ url: URL) -> Int? {
    if let port = url.port { return port }
    return url.scheme?.lowercased() == "https" ? 443 : 80
  }
}

private final class NoemaFaviconSessionDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
  func urlSession(
    _ session: URLSession,
    task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) {
    completionHandler(nil)
  }
}

enum NoemaFaviconSize {
  case compact
  case standard

  var dimension: CGFloat {
    switch self {
    case .compact: NoemaSpacing.md
    case .standard: NoemaSpacing.lg
    }
  }
}

struct NoemaFaviconImage: View {
  let hostname: String
  let profile: NoemaProfile?
  var size: NoemaFaviconSize = .standard
  var showsGlobeFallback = false
  @State private var data: Data?
  @State private var unavailable = false

  var body: some View {
    Group {
      if let data, let image = UIImage(data: data) {
        Image(uiImage: image)
          .resizable()
          .scaledToFit()
      } else if unavailable && showsGlobeFallback {
        Image(systemName: "globe")
      } else {
        Color.clear
      }
    }
    .frame(
      width: unavailable && !showsGlobeFallback ? 0 : size.dimension,
      height: unavailable && !showsGlobeFallback ? 0 : size.dimension
    )
    .clipShape(Circle())
    .accessibilityHidden(true)
    .task(id: requestID) {
      data = nil
      unavailable = false
      guard let profile else {
        unavailable = true
        return
      }
      data = await NoemaFaviconLoader.shared.data(hostname: hostname, profile: profile)
      unavailable = data == nil
    }
  }

  private var requestID: String {
    "\(profile?.origin.absoluteString ?? "")#\(profile?.credentialGeneration ?? 0)#\(hostname)"
  }
}
