import SwiftUI
struct ToolDetailRow: Equatable {
  let label: String
  let value: String
}
struct ToolMarkerAttachmentView: View {
  let rows: [ToolDetailRow]
  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(row.label)
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(row.value)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineSpacing(2)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
      }
    }
    .padding(.leading, NoemaSpacing.xl)
    .frame(maxWidth: 520, alignment: .leading)
    .transition(.opacity.combined(with: .move(edge: .top)))
  }
}

struct ToolTechnicalRecordView: View {
  let title: String
  let rows: [ToolDetailRow]
  let onClose: () -> Void

  var body: some View {
    NoemaNativeSheet(title: "Technical record", onDismiss: onClose) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          Text(title)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
          ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
            VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
              Text(row.label.uppercased())
                .font(NoemaFont.monoTiny)
                .tracking(0.72)
                .foregroundStyle(NoemaColor.ink400)
              Text(row.value)
                .font(NoemaFont.monoTiny)
                .foregroundStyle(NoemaColor.contentTertiary)
                .textSelection(.enabled)
            }
          }
        }
        .padding(NoemaSpacing.lg)
        .frame(maxWidth: .infinity, alignment: .leading)
      }
    }
    .presentationDetents([.large])
    .presentationDragIndicator(.visible)
  }
}
func toolDetailRows(in messages: [ChatMessage]) -> [ToolDetailRow] {
  if let completeRows = completeToolDetailRows(in: messages) {
    return completeRows
  }
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

private func completeToolDetailRows(in messages: [ChatMessage]) -> [ToolDetailRow]? {
  let actions = messages.compactMap { metadataObject(for: $0)["action"] as? [String: Any] }
  guard actions.contains(where: { stringValue($0["detail_mode"]) == "complete" }) else { return nil }
  let call = actions.first { !($0["success"] is Bool) }
  let result = actions.reversed().first { $0["success"] is Bool }
  var rows: [ToolDetailRow] = []
  if let correlation = stringValue(call?["correlation_id"])
    ?? stringValue(result?["correlation_id"])
    ?? stringValue(call?["id"])
    ?? stringValue(result?["call_id"]) {
    rows.append(ToolDetailRow(label: "Correlation", value: correlation))
  }
  if let input = call?["arguments"] ?? result?["arguments"] ?? call?["payload"], !isOmittedToolPayload(input) {
    rows.append(ToolDetailRow(label: "Input", value: technicalText(input)))
  }
  if let success = result?["success"] as? Bool {
    rows.append(ToolDetailRow(label: "Success", value: String(success)))
  }
  if let output = result?["payload"], !isOmittedToolPayload(output) {
    rows.append(ToolDetailRow(
      label: result?["success"] as? Bool == false ? "Error" : "Output",
      value: technicalText(output)
    ))
  }
  return deduplicated(rows)
}

private func isOmittedToolPayload(_ value: Any) -> Bool {
  guard let object = value as? [String: Any], object.count == 1 else { return false }
  return object["omitted"] as? Bool == true
}

private func technicalText(_ value: Any) -> String {
  var sanitized = value
  if var object = value as? [String: Any],
     var screenshot = object["screenshot"] as? [String: Any],
     screenshot["data"] is String {
    screenshot["data"] = "[image data omitted]"
    object["screenshot"] = screenshot
    sanitized = object
  }
  return prettyJSON(sanitized) ?? String(describing: sanitized)
}

func toolHumanDetailRows(in messages: [ChatMessage]) -> [ToolDetailRow] {
  let call = messages.first { activityValues($0)?.activityKind.normalizedActivityKind == "TOOL_CALL" }
    .map(metadataObject(for:))
  let result = messages.reversed().first { activityValues($0)?.activityKind.normalizedActivityKind == "TOOL_RESULT" }
    .map(metadataObject(for:))
  let callDisplay = call?["display"] as? [String: Any]
  let resultDisplay = result?["display"] as? [String: Any]
  let input = actionInput(call, result)
  let output = result.flatMap(toolActionPayload)
  var rows: [ToolDetailRow] = []
  let target = humanToolTarget(messages, input: input, callDisplay: callDisplay, resultDisplay: resultDisplay)

  if let target, toolMarkerKind(in: messages) != "web.browse" { rows.append(target) }
  let displayResult = usefulDisplayValue(resultDisplay, key: "result")
  if toolMarkerStatus(in: messages) == .error {
    rows.append(ToolDetailRow(
      label: "What happened",
      value: humanToolError(output) ?? displayResult ?? toolMarkerName(in: messages)
    ))
  } else if let displayResult, displayResult != target?.value {
    rows.append(ToolDetailRow(label: "Result", value: displayResult))
  } else if let detail = toolMarkerField("detail", in: messages), detail != target?.value {
    rows.append(ToolDetailRow(label: "Status", value: detail))
  } else if let result = humanToolResult(output), result.value != target?.value {
    rows.append(result)
  }

  if let scope = usefulDisplayValue(resultDisplay, key: "scope")
    ?? usefulDisplayValue(callDisplay, key: "scope"), scope != target?.value {
    rows.append(ToolDetailRow(label: "Scope", value: scope))
  }
  if rows.isEmpty, let purpose = usefulDisplayValue(callDisplay, key: "purpose") {
    rows.append(ToolDetailRow(label: "Purpose", value: purpose))
  }
  return deduplicated(rows)
}

private func actionInput(_ call: [String: Any]?, _ result: [String: Any]?) -> Any? {
  if let arguments = (call?["action"] as? [String: Any])?["arguments"] { return arguments }
  if let arguments = (result?["action"] as? [String: Any])?["arguments"] { return arguments }
  return call.flatMap(toolActionPayload)
}

private func humanToolTarget(
  _ messages: [ChatMessage],
  input: Any?,
  callDisplay: [String: Any]?,
  resultDisplay: [String: Any]?
) -> ToolDetailRow? {
  if let subject = toolMarkerField("subject", in: messages) {
    return ToolDetailRow(label: toolMarkerField("subjectLabel", in: messages) ?? "Item", value: subject)
  }
  let payload = input as? [String: Any]
  let title = nestedString(payload ?? [:], path: ["task", "title"])
    ?? nestedString(payload ?? [:], path: ["project", "name"])
    ?? stringValue(payload?["title"])
  if let title { return ToolDetailRow(label: "Item", value: title) }
  for (key, label) in [("query", "Search"), ("path", "File"), ("name", "Name")] {
    if let value = stringValue(payload?[key]) { return ToolDetailRow(label: label, value: value) }
  }
  let target = usefulDisplayValue(resultDisplay, key: "target")
    ?? usefulDisplayValue(callDisplay, key: "target")
    ?? stringValue(payload?["url"])
  return target.map { ToolDetailRow(label: "Item", value: $0) }
}

private func humanToolResult(_ output: Any?) -> ToolDetailRow? {
  guard let output = output as? [String: Any] else { return nil }
  for (key, noun) in [("tasks", "task"), ("projects", "project"), ("entries", "item"),
                      ("results", "result"), ("messages", "message"), ("events", "event"), ("pages", "page")] {
    if let values = output[key] as? [Any] {
      return ToolDetailRow(label: "Result", value: "\(values.count) \(noun)\(values.count == 1 ? "" : "s")")
    }
  }
  return nil
}

private func humanToolError(_ output: Any?) -> String? {
  guard let output = output as? [String: Any] else { return nil }
  return stringValue(output["error"])
    ?? stringValue(output["message"])
    ?? nestedString(output, path: ["details", "message"])
}

private func usefulDisplayValue(_ display: [String: Any]?, key: String) -> String? {
  guard let value = stringValue(display?[key]), !["Use an enabled connected tool", "Uses a connected tool", "Completed", "Done"].contains(value) else {
    return nil
  }
  return value
}

private func deduplicated(_ rows: [ToolDetailRow]) -> [ToolDetailRow] {
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
