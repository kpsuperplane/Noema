import Foundation
import Network
import OSLog

/// A bounded, device-local trace for short-lived connectivity investigations.
///
/// Entries contain lifecycle and transport metadata only. They never contain
/// credentials, notification tokens, GraphQL variables, or product content.
final class NoemaDiagnosticTrace: @unchecked Sendable {
  static let shared = NoemaDiagnosticTrace()

  private struct Entry: Encodable {
    let timestamp: String
    let uptimeMilliseconds: UInt64
    let session: String
    let category: String
    let event: String
    let fields: [String: String]
  }

  private let logger = Logger(subsystem: "dev.noema.app.ios", category: "Connectivity")
  private let ioQueue = DispatchQueue(label: "dev.noema.app.ios.diagnostics")
  private let monitorQueue = DispatchQueue(label: "dev.noema.app.ios.network-path")
  private let monitor = NWPathMonitor()
  private let session = UUID().uuidString
  private let processStart = ProcessInfo.processInfo.systemUptime
  private let encoder: JSONEncoder
  private let formatter: ISO8601DateFormatter
  private let maximumFileBytes = 512 * 1024

  private init() {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    self.encoder = encoder
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    self.formatter = formatter
    monitor.pathUpdateHandler = { [weak self] path in
      self?.record(
        category: "network",
        event: "path_changed",
        fields: [
          "status": Self.pathStatus(path.status),
          "interface": Self.interfaceName(path),
          "expensive": String(path.isExpensive),
          "constrained": String(path.isConstrained),
          "dns": String(path.supportsDNS),
          "ipv4": String(path.supportsIPv4),
          "ipv6": String(path.supportsIPv6)
        ]
      )
    }
    monitor.start(queue: monitorQueue)
    record(category: "app", event: "trace_started")
  }

  func record(category: String, event: String, fields: [String: String] = [:]) {
    let details = fields
      .sorted { $0.key < $1.key }
      .map { "\($0.key)=\($0.value)" }
      .joined(separator: " ")
    logger.notice(
      "\(category, privacy: .public).\(event, privacy: .public) \(details, privacy: .public)"
    )
    let timestamp = Date()
    let uptime = ProcessInfo.processInfo.systemUptime
    ioQueue.async { [self] in
      let entry = Entry(
        timestamp: formatter.string(from: timestamp),
        uptimeMilliseconds: UInt64(max(0, uptime - processStart) * 1_000),
        session: session,
        category: category,
        event: event,
        fields: fields
      )
      append(entry)
    }
  }

  func record(category: String, event: String, error: Error, fields: [String: String] = [:]) {
    let error = error as NSError
    var values = fields
    values["errorDomain"] = error.domain
    values["errorCode"] = String(error.code)
    record(category: category, event: event, fields: values)
  }

  private func append(_ entry: Entry) {
    do {
      let urls = try traceURLs()
      try rotateIfNeeded(current: urls.current, previous: urls.previous)
      var data = try encoder.encode(entry)
      data.append(0x0A)
      if !FileManager.default.fileExists(atPath: urls.current.path) {
        try data.write(to: urls.current, options: .atomic)
        try protect(urls.current)
        return
      }
      let handle = try FileHandle(forWritingTo: urls.current)
      defer { try? handle.close() }
      try handle.seekToEnd()
      try handle.write(contentsOf: data)
    } catch {
      logger.error("diagnostic trace write failed: \((error as NSError).code, privacy: .public)")
    }
  }

  private func traceURLs() throws -> (current: URL, previous: URL) {
    let fileManager = FileManager.default
    guard let applicationSupport = fileManager.urls(
      for: .applicationSupportDirectory,
      in: .userDomainMask
    ).first else { throw CocoaError(.fileNoSuchFile) }
    var directory = applicationSupport
      .appending(path: "Noema", directoryHint: .isDirectory)
      .appending(path: "Diagnostics", directoryHint: .isDirectory)
    try fileManager.createDirectory(
      at: directory,
      withIntermediateDirectories: true,
      attributes: [.protectionKey: FileProtectionType.complete]
    )
    try fileManager.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: directory.path)
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try directory.setResourceValues(values)
    return (
      directory.appending(path: "connectivity.jsonl"),
      directory.appending(path: "connectivity.previous.jsonl")
    )
  }

  private func rotateIfNeeded(current: URL, previous: URL) throws {
    let fileManager = FileManager.default
    guard let attributes = try? fileManager.attributesOfItem(atPath: current.path),
          let size = attributes[.size] as? NSNumber,
          size.intValue >= maximumFileBytes else { return }
    try? fileManager.removeItem(at: previous)
    try fileManager.moveItem(at: current, to: previous)
    try protect(previous)
  }

  private func protect(_ url: URL) throws {
    try FileManager.default.setAttributes(
      [.protectionKey: FileProtectionType.complete],
      ofItemAtPath: url.path
    )
  }

  private static func pathStatus(_ status: NWPath.Status) -> String {
    switch status {
    case .satisfied: "satisfied"
    case .unsatisfied: "unsatisfied"
    case .requiresConnection: "requires_connection"
    @unknown default: "unknown"
    }
  }

  private static func interfaceName(_ path: NWPath) -> String {
    if path.usesInterfaceType(.wifi) { return "wifi" }
    if path.usesInterfaceType(.cellular) { return "cellular" }
    if path.usesInterfaceType(.wiredEthernet) { return "wired" }
    if path.usesInterfaceType(.loopback) { return "loopback" }
    if path.usesInterfaceType(.other) { return "other" }
    return "none"
  }
}
