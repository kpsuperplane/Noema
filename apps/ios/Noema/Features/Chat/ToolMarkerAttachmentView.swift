import SwiftUI
struct ToolDetailRow: Equatable {
  let label: String
  let value: String
}
struct ToolMarkerAttachmentView: View {
  let title: String
  let failed: Bool
  let rows: [ToolDetailRow]
  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(spacing: NoemaSpacing.sm + NoemaSpacing.xxs) {
        Image(systemName: "wrench")
          .font(.system(size: 14, weight: .semibold))
          .foregroundStyle(failed ? NoemaColor.danger : NoemaColor.blue700)
          .frame(width: 28, height: 28)
          .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: 8))
        Text(title)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
          .frame(maxWidth: .infinity, alignment: .leading)
      }
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Rectangle()
          .fill(NoemaColor.separatorSubtle)
          .frame(height: 1)
        ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
          VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
            Text(row.label.uppercased())
              .font(NoemaFont.monoTiny)
              .tracking(0.72)
              .foregroundStyle(NoemaColor.ink400)
            Text(row.value)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentTertiary)
              .lineSpacing(2)
              .textSelection(.enabled)
              .frame(maxWidth: .infinity, alignment: .leading)
          }
        }
      }
      .padding(.top, NoemaSpacing.xs)
    }
    .padding(NoemaSpacing.md)
    .frame(maxWidth: .infinity, alignment: .leading)
    .frame(maxWidth: 520, alignment: .leading)
    .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: 8))
    .overlay {
      RoundedRectangle(cornerRadius: 8)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
    .transition(.opacity.combined(with: .move(edge: .top)))
  }
}
func toolDetailRows(in messages: [ChatMessage]) -> [ToolDetailRow] {
  if let canonicalWebRows = canonicalWebDetailRows(in: messages) {
    return canonicalWebRows
  }
  var rows: [ToolDetailRow] = []
  for message in messages {
    let metadata = metadataObject(for: message)
    guard let values = activityValues(message) else { continue }
    let result = values.activityKind.normalizedActivityKind == "TOOL_RESULT"
    let fallback = result ? (values.status.uppercased() == "FAILED" ? "Error" : "Output") : "Input"
    if let payload = toolActionPayload(metadata) {
      rows.append(contentsOf: toolPayloadRows(payload, fallbackLabel: fallback))
    } else if let detail = toolDetail(for: message) {
      rows.append(ToolDetailRow(label: fallback, value: truncateToolDetail(detail)))
    }
  }
  var seen = Set<String>()
  return rows.filter { seen.insert("\($0.label)\n\($0.value)").inserted }
}
private func canonicalWebDetailRows(in messages: [ChatMessage]) -> [ToolDetailRow]? {
  let metadata = messages.map(metadataObject(for:))
  let actionName = metadata.reversed().compactMap { nestedString($0, path: ["action", "name"]) }.first
  let succeeded = metadata.contains { (($0["action"] as? [String: Any])?["success"] as? Bool) == true }
  guard succeeded, actionName == "web.search" || actionName == "web.fetch" else { return nil }
  for object in metadata.reversed() {
    guard let display = object["display"] as? [String: Any] else { continue }
    var rows: [ToolDetailRow] = []
    if let source = stringValue(display["fallbackFrom"]) {
      rows.append(ToolDetailRow(label: "Fallback from", value: source))
    }
    if let reason = stringValue(display["fallbackReason"]) {
      rows.append(ToolDetailRow(label: "Fallback reason", value: reason))
    }
    if !rows.isEmpty { return rows }
  }
  return []
}
private func toolActionPayload(_ metadata: [String: Any]) -> Any? {
  guard let action = metadata["action"] as? [String: Any] else { return nil }
  let payload = action["payload"] ?? action["arguments"]
  if let object = payload as? [String: Any], object.count == 1, let arguments = object["arguments"] {
    return arguments
  }
  return payload
}
private func toolPayloadRows(_ payload: Any, fallbackLabel: String) -> [ToolDetailRow] {
  if let object = payload as? [String: Any] {
    for key in ["query", "name", "url", "path", "error"] {
      if let value = scalarToolValue(object[key]) {
        let label = key == "url" ? "URL" : key.capitalized
        return [ToolDetailRow(label: label, value: truncateToolDetail(value))]
      }
    }
    let summary = object.keys.sorted().compactMap { key in
      scalarToolValue(object[key]).map { "\(key): \($0)" }
    }.prefix(3).joined(separator: ", ")
    return summary.isEmpty ? [] : [ToolDetailRow(label: fallbackLabel, value: truncateToolDetail(summary))]
  }
  guard let value = scalarToolValue(payload) else { return [] }
  return [ToolDetailRow(label: fallbackLabel, value: truncateToolDetail(value))]
}
private func scalarToolValue(_ value: Any?) -> String? {
  switch value {
  case let string as String where !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty:
    string
  case let boolean as Bool:
    String(boolean)
  case let number as NSNumber:
    number.stringValue
  default:
    nil
  }
}
private func truncateToolDetail(_ value: String) -> String {
  let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
  return trimmed.count > 280 ? String(trimmed.prefix(280)) + "..." : trimmed
}
