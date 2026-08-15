import Apollo
import ApolloAPI
import Foundation
import NoemaAPI
import SwiftUI

/// Chat references stay in the conversation while opening the same large,
/// mobile-shaped detail sheet used by Work.
struct ChatTaskDetailSheet: View {
  @Environment(\.dismiss) private var dismiss
  let client: ApolloClient?
  let profile: NoemaProfile?
  let connectionStatus: NoemaConnectionStatus?
  let taskID: String
  @State private var tasksModel: TasksModel?

  var body: some View {
    Group {
      if let tasksModel {
        TasksDetailRoute(model: tasksModel, taskId: taskID, compactPresentation: true)
      } else {
        NoemaNativeSheet(title: "Task", onDismiss: { dismiss() }) {
          if client == nil {
            NoemaDeckState(title: "Task unavailable", message: "Connect this device to load the task details.", symbol: "checklist", tone: .warning)
          } else {
            ProgressView("Loading task…")
              .frame(maxWidth: .infinity, maxHeight: .infinity)
          }
        }
      }
    }
    .background(NoemaColor.surface)
    .noemaMobileDrawerPresentation()
    .task(id: taskID) {
      guard tasksModel == nil, let client else { return }
      guard let connectionStatus else { return }
      tasksModel = TasksModel(client: client, profile: profile, connectionStatus: connectionStatus)
    }
  }
}

struct RuntimeDebugUsage: Identifiable, Equatable {
  let id: String
  let provider: String
  let model: String
  let phase: String
  let responseIndex: Int
  let outputIndex: Int?
  let inputTokens: Int
  let cachedInputTokens: Int?
  let cacheHitRatio: Double?
  let outputTokens: Int
  let totalTokens: Int
}

enum RuntimeDebugScopeKind: String, Equatable {
  case conversationTurn = "CONVERSATION_TURN"
  case taskRun = "TASK_RUN"

  var title: String { self == .taskRun ? "Agent run runtime" : "Turn runtime" }
}

struct RuntimeDebugScope: Equatable {
  let kind: RuntimeDebugScopeKind
  let scopeID: String

  var id: String { "\(kind.rawValue):\(scopeID)" }
}

enum RuntimeDebugFocus: Equatable {
  case provider(phase: String?, responseIndex: Int?, roundIndex: Int?)
  case tool(correlationID: String?)
}

struct RuntimeDebugTarget: Identifiable, Equatable {
  let scope: RuntimeDebugScope?
  let legacyUsage: RuntimeDebugUsage?
  let focus: RuntimeDebugFocus?

  var id: String { scope?.id ?? legacyUsage?.id ?? "runtime-debug" }
}

enum RuntimeDebugStatus: String, Equatable {
  case running = "RUNNING"
  case completed = "COMPLETED"
  case failed = "FAILED"
  case cancelled = "CANCELLED"
  case interrupted = "INTERRUPTED"

  var isTerminal: Bool { self != .running }
}

enum RuntimeDebugSpanCategory: String, CaseIterable, Equatable {
  case provider = "PROVIDER"
  case tool = "TOOL"
  case runtime = "RUNTIME"
  case persistence = "PERSISTENCE"

  var tint: Color {
    switch self {
    case .provider: NoemaColor.blue700
    case .tool: NoemaColor.pine700
    case .runtime: NoemaColor.clay600
    case .persistence: NoemaColor.contentSecondary
    }
  }
}

struct RuntimeDebugSpanSnapshot: Identifiable, Equatable {
  let id: String
  let category: RuntimeDebugSpanCategory
  let name: String
  let status: RuntimeDebugStatus
  let startedAt: String
  let endedAt: String?
  let startOffsetMilliseconds: Int
  let durationMilliseconds: Int
  let provider: String?
  let model: String?
  let phase: String?
  let responseIndex: Int?
  let roundIndex: Int?
  let toolName: String?
  let correlationID: String?
  let inputTokens: Int?
  let cachedInputTokens: Int?
  let outputTokens: Int?
  let totalTokens: Int?
}

struct RuntimeDebugProfileSnapshot: Equatable {
  let kind: RuntimeDebugScopeKind
  let scopeID: String
  let status: RuntimeDebugStatus
  let startedAt: String
  let endedAt: String?
  let elapsedMilliseconds: Int
  let accountedMilliseconds: Int
  let uninstrumentedMilliseconds: Int
  let spans: [RuntimeDebugSpanSnapshot]
}

func runtimeDebugScope(turnID: String?, metadata: String?) -> RuntimeDebugScope? {
  guard let object = runtimeDebugMetadataObject(metadata) else {
    return turnID.map { RuntimeDebugScope(kind: .conversationTurn, scopeID: $0) }
  }
  for key in ["debug_scope", "debugScope", "runtime_debug_scope", "runtimeDebugScope"] {
    if let scopeObject = object[key] as? [String: Any], let scope = runtimeDebugScope(from: scopeObject) {
      return scope
    }
  }
  for key in ["task_run_id", "taskRunId", "agent_run_id", "agentRunId", "run_id", "runId"] {
    if let scopeID = nonEmptyString(object[key]) {
      return RuntimeDebugScope(kind: .taskRun, scopeID: scopeID)
    }
  }
  return turnID.map { RuntimeDebugScope(kind: .conversationTurn, scopeID: $0) }
}

private func runtimeDebugScope(from object: [String: Any]) -> RuntimeDebugScope? {
  guard let scopeID = nonEmptyString(object["scope_id"] ?? object["scopeId"] ?? object["id"]) else { return nil }
  let rawKind = nonEmptyString(object["kind"])?.uppercased() ?? "CONVERSATION_TURN"
  guard let kind = RuntimeDebugScopeKind(rawValue: rawKind) else { return nil }
  return RuntimeDebugScope(kind: kind, scopeID: scopeID)
}

private func runtimeDebugMetadataObject(_ metadata: String?) -> [String: Any]? {
  guard let metadata,
        let data = metadata.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
  return object
}

func runtimeDebugUsage(for message: ChatMessage) -> RuntimeDebugUsage? {
  guard let usage = runtimeDebugMetadataObject(message.metadata)?["provider_usage"] as? [String: Any] else { return nil }
  return makeRuntimeDebugUsage(usage)
}

func runtimeDebugUsage(from messages: [ChatMessage]) -> RuntimeDebugUsage? {
  messages.reversed().compactMap(runtimeDebugUsage(for:)).first
}

func runtimeDebugTarget(for message: ChatMessage) -> RuntimeDebugTarget? {
  let usage = runtimeDebugUsage(for: message)
  let scope = message.debugScope ?? runtimeDebugScope(turnID: message.turnID, metadata: message.metadata)
  guard scope != nil || usage != nil else { return nil }
  let focus = RuntimeDebugFocus.provider(phase: usage?.phase, responseIndex: usage?.responseIndex, roundIndex: nil)
  return RuntimeDebugTarget(scope: scope, legacyUsage: usage, focus: focus)
}

func runtimeDebugTarget(from messages: [ChatMessage]) -> RuntimeDebugTarget? {
  let scope = messages.reversed().compactMap(\.debugScope).first
    ?? messages.reversed().compactMap { runtimeDebugScope(turnID: $0.turnID, metadata: $0.metadata) }.first
  let usage = runtimeDebugUsage(from: messages)
  guard scope != nil || usage != nil else { return nil }
  let correlationID = messages.reversed().compactMap { runtimeDebugToolCorrelationID(for: $0) }.first
  let focus: RuntimeDebugFocus? = correlationID.map { .tool(correlationID: $0) }
    ?? .provider(phase: usage?.phase, responseIndex: usage?.responseIndex, roundIndex: nil)
  return RuntimeDebugTarget(scope: scope, legacyUsage: usage, focus: focus)
}

private func runtimeDebugToolCorrelationID(for message: ChatMessage) -> String? {
  guard let metadata = runtimeDebugMetadataObject(message.metadata) else { return nil }
  for path in [["action", "correlation_id"], ["action", "correlationId"], ["action", "id"], ["action", "call_id"], ["action", "callId"], ["correlation_id"], ["correlationId"], ["call_id"], ["callId"]] {
    var value: Any = metadata
    for key in path {
      guard let object = value as? [String: Any], let next = object[key] else {
        value = NSNull()
        break
      }
      value = next
    }
    if let correlationID = nonEmptyString(value) { return correlationID }
  }
  return nil
}

private func makeRuntimeDebugUsage(_ usage: [String: Any]) -> RuntimeDebugUsage? {
  guard let provider = nonEmptyString(usage["provider"]),
        let model = nonEmptyString(usage["model"]),
        let phase = nonEmptyString(usage["phase"]),
        let responseIndex = integerValue(usage["response_index"]),
        let inputTokens = integerValue(usage["input_tokens"]),
        let outputTokens = integerValue(usage["output_tokens"]) else { return nil }
  let outputIndex = integerValue(usage["output_index"])
  let cachedInputTokens = integerValue(usage["cached_input_tokens"])
  let cacheHitRatio = decimalValue(usage["cache_hit_ratio"])
  guard let totalTokens = integerValue(usage["total_tokens"]) else { return nil }
  let id = [provider, model, phase, String(responseIndex), String(outputIndex ?? -1)].joined(separator: ":")
  return RuntimeDebugUsage(
    id: id,
    provider: provider,
    model: model,
    phase: phase,
    responseIndex: responseIndex,
    outputIndex: outputIndex,
    inputTokens: inputTokens,
    cachedInputTokens: cachedInputTokens,
    cacheHitRatio: cacheHitRatio,
    outputTokens: outputTokens,
    totalTokens: totalTokens
  )
}

private func nonEmptyString(_ value: Any?) -> String? {
  guard let value = value as? String else { return nil }
  let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
  return trimmed.isEmpty ? nil : trimmed
}

private func integerValue(_ value: Any?) -> Int? {
  guard let value = value as? NSNumber else { return nil }
  return value.intValue
}

private func decimalValue(_ value: Any?) -> Double? {
  guard let value = value as? NSNumber else { return nil }
  return value.doubleValue.isFinite ? value.doubleValue : nil
}

private enum RuntimeDebugLoadState: Equatable {
  case idle
  case loading
  case loaded(RuntimeDebugProfileSnapshot)
  case unavailable
  case failed(String)
}

struct RuntimeDebugSheet: View {
  @Environment(\.dismiss) private var dismiss
  let client: ApolloClient?
  let target: RuntimeDebugTarget
  @State private var loadState: RuntimeDebugLoadState = .idle
  @State private var selectedSpanID: String?

  var body: some View {
    NoemaNativeSheet(title: target.scope?.kind.title ?? "Turn runtime", onDismiss: { dismiss() }) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          runtimeContent
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.vertical, NoemaSpacing.md)
      }
    }
    .presentationDetents([.medium, .large])
    .presentationDragIndicator(.visible)
    .task(id: target.id) { await loadProfile() }
  }

  @ViewBuilder
  private var runtimeContent: some View {
    if target.scope == nil {
      NoemaInlineState(message: "Runtime profiling wasn’t captured for this historical message.", symbol: "chart.xyaxis.line")
      if let usage = target.legacyUsage { RuntimeDebugUsageDetails(usage: usage) }
    } else if client == nil {
      NoemaInlineState(message: "Connect this device to load the runtime profile.", symbol: "bolt.horizontal.circle")
      if let usage = target.legacyUsage { RuntimeDebugUsageDetails(usage: usage) }
    } else {
      switch loadState {
      case .idle, .loading:
        ProgressView("Loading runtime profile…")
          .frame(maxWidth: .infinity, alignment: .leading)
      case let .failed(message):
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          NoemaInlineState(message: message, symbol: "exclamationmark.triangle")
          Button("Try again") { Task { await loadProfile() } }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        }
      case .unavailable:
        NoemaInlineState(message: "Runtime profiling wasn’t captured for this turn or run.", symbol: "chart.xyaxis.line")
        if let usage = target.legacyUsage { RuntimeDebugUsageDetails(usage: usage) }
      case let .loaded(profile):
        RuntimeDebugProfileContent(profile: profile, target: target, selectedSpanID: $selectedSpanID)
      }
    }
  }

  private func loadProfile() async {
    guard let client, let scope = target.scope else {
      loadState = .unavailable
      return
    }
    loadState = .loading
    do {
      var profile = try await fetchProfile(client: client, scope: scope)
      guard !Task.isCancelled else { return }
      guard let first = profile else {
        loadState = .unavailable
        return
      }
      loadState = .loaded(first)
      guard !first.status.isTerminal else { return }
      for _ in 0..<120 {
        try await Task.sleep(for: .seconds(1))
        guard !Task.isCancelled else { return }
        profile = try await fetchProfile(client: client, scope: scope)
        guard !Task.isCancelled else { return }
        guard let next = profile else {
          loadState = .unavailable
          return
        }
        loadState = .loaded(next)
        if next.status.isTerminal { return }
      }
    } catch is CancellationError {
      return
    } catch {
      guard !Task.isCancelled else { return }
      loadState = .failed(error.localizedDescription)
    }
  }

  private func fetchProfile(client: ApolloClient, scope: RuntimeDebugScope) async throws -> RuntimeDebugProfileSnapshot? {
    let kind = NoemaAPI.RuntimeDebugScopeKind(rawValue: scope.kind.rawValue) ?? .conversationTurn
    let input = NoemaAPI.RuntimeDebugProfileInput(kind: GraphQLEnum(kind), scopeId: scope.scopeID)
    let response = try await client.fetch(query: NoemaAPI.RuntimeDebugProfileQuery(input: input), cachePolicy: .networkOnly)
    if let message = response.errors?.first?.message { throw RuntimeDebugProfileError.server(message) }
    return response.data?.runtimeDebugProfile.map(runtimeDebugProfileSnapshot)
  }
}

private enum RuntimeDebugProfileError: LocalizedError {
  case server(String)

  var errorDescription: String? {
    switch self {
    case let .server(message): "Runtime profile could not be loaded: \(message)"
    }
  }
}

private func runtimeDebugProfileSnapshot(_ profile: NoemaAPI.RuntimeDebugProfileQuery.Data.RuntimeDebugProfile) -> RuntimeDebugProfileSnapshot {
  RuntimeDebugProfileSnapshot(
    kind: RuntimeDebugScopeKind(rawValue: profile.kind.rawValue) ?? .conversationTurn,
    scopeID: profile.scopeId,
    status: RuntimeDebugStatus(rawValue: profile.status.rawValue) ?? .running,
    startedAt: profile.startedAt,
    endedAt: profile.endedAt,
    elapsedMilliseconds: profile.elapsedMilliseconds,
    accountedMilliseconds: profile.accountedMilliseconds,
    uninstrumentedMilliseconds: profile.uninstrumentedMilliseconds,
    spans: profile.spans.map { span in
      RuntimeDebugSpanSnapshot(
        id: span.id,
        category: RuntimeDebugSpanCategory(rawValue: span.category.rawValue) ?? .runtime,
        name: span.name,
        status: RuntimeDebugStatus(rawValue: span.status.rawValue) ?? .running,
        startedAt: span.startedAt,
        endedAt: span.endedAt,
        startOffsetMilliseconds: span.startOffsetMilliseconds,
        durationMilliseconds: span.durationMilliseconds,
        provider: span.provider,
        model: span.model,
        phase: span.phase,
        responseIndex: span.responseIndex,
        roundIndex: span.roundIndex,
        toolName: span.toolName,
        correlationID: span.correlationId,
        inputTokens: span.inputTokens,
        cachedInputTokens: span.cachedInputTokens,
        outputTokens: span.outputTokens,
        totalTokens: span.totalTokens
      )
    }
  )
}

private struct RuntimeDebugProfileContent: View {
  let profile: RuntimeDebugProfileSnapshot
  let target: RuntimeDebugTarget
  @Binding var selectedSpanID: String?

  private var selectedSpan: RuntimeDebugSpanSnapshot? {
    profile.spans.first(where: { $0.id == selectedSpanID })
      ?? runtimeDebugFocusedSpan(profile.spans, focus: target.focus)
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      NoemaSectionSurface("Runtime summary") {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          HStack(alignment: .firstTextBaseline) {
            Text(formatRuntimeDuration(profile.elapsedMilliseconds))
              .font(NoemaFont.mobileTitle)
              .foregroundStyle(NoemaColor.content)
            Spacer(minLength: NoemaSpacing.sm)
            Text(humanizeRuntimeStatus(profile.status))
              .font(NoemaFont.caption)
              .foregroundStyle(profile.status == .failed ? NoemaColor.danger : NoemaColor.contentSecondary)
          }
          ForEach(RuntimeDebugSpanCategory.allCases, id: \.self) { category in
            let duration = runtimeCoveredDuration(profile.spans.filter { $0.category == category })
            if duration > 0 {
              HStack(spacing: NoemaSpacing.sm) {
                Circle().fill(category.tint).frame(width: 7, height: 7)
                Text(category.rawValue.capitalized)
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                Spacer(minLength: NoemaSpacing.sm)
                Text(formatRuntimeDuration(duration))
                  .font(NoemaFont.mono)
                  .foregroundStyle(NoemaColor.content)
              }
            }
          }
          if profile.uninstrumentedMilliseconds > 0 {
            runtimeDebugRow("Uninstrumented", formatRuntimeDuration(profile.uninstrumentedMilliseconds))
          }
        }
      }

      if profile.spans.isEmpty {
        NoemaInlineState(message: "Runtime profiling wasn’t captured for this turn or run.", symbol: "chart.xyaxis.line")
      } else {
        NoemaSectionSurface("Span timeline") {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            ForEach(profile.spans) { span in
              Button {
                selectedSpanID = span.id
              } label: {
                RuntimeDebugTimelineRow(span: span, elapsedMilliseconds: profile.elapsedMilliseconds, selected: selectedSpan?.id == span.id)
              }
              .buttonStyle(.plain)
            }
          }
        }
      }

      if let selectedSpan {
        RuntimeDebugSpanDetails(span: selectedSpan)
      } else if let usage = target.legacyUsage {
        RuntimeDebugUsageDetails(usage: usage)
      }
    }
  }
}

private struct RuntimeDebugTimelineRow: View {
  let span: RuntimeDebugSpanSnapshot
  let elapsedMilliseconds: Int
  let selected: Bool

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      HStack(spacing: NoemaSpacing.xs) {
        Circle().fill(span.category.tint).frame(width: 7, height: 7)
        Text(span.name)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
        Spacer(minLength: NoemaSpacing.xs)
        Text(formatRuntimeDuration(span.durationMilliseconds))
          .font(NoemaFont.monoTiny)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      GeometryReader { proxy in
        let total = max(elapsedMilliseconds, 1)
        let start = min(max(Double(span.startOffsetMilliseconds) / Double(total), 0), 1)
        let width = min(max(Double(max(span.durationMilliseconds, 1)) / Double(total), 0.015), 1 - start)
        ZStack(alignment: .leading) {
          NoemaSuperellipse.full.fill(NoemaColor.separatorSubtle)
          NoemaSuperellipse.full
            .fill(span.category.tint)
            .frame(width: max(4, proxy.size.width * width))
            .offset(x: proxy.size.width * start)
        }
      }
      .frame(height: 7)
    }
    .padding(.vertical, NoemaSpacing.xs)
    .padding(.horizontal, NoemaSpacing.xs)
    .background(selected ? NoemaColor.pine50 : NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.inner))
  }
}

private struct RuntimeDebugSpanDetails: View {
  let span: RuntimeDebugSpanSnapshot

  var body: some View {
    NoemaSectionSurface("Selected span") {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        runtimeDebugRow("Span", span.name)
        runtimeDebugRow("Start", formatRuntimeDuration(span.startOffsetMilliseconds))
        runtimeDebugRow("Timeline duration", formatRuntimeDuration(span.durationMilliseconds))
        runtimeDebugRow("Status", humanizeRuntimeStatus(span.status))
        runtimeDebugOptionalRow("Provider", span.provider)
        runtimeDebugOptionalRow("Model", span.model)
        runtimeDebugOptionalRow("Phase", span.phase)
        runtimeDebugOptionalRow("Tool", span.toolName)
        runtimeDebugOptionalRow("Response", span.responseIndex.map(String.init))
        runtimeDebugOptionalRow("Round", span.roundIndex.map(String.init))
        runtimeDebugOptionalRow("Input tokens", span.inputTokens.map(formatRuntimeInteger))
        runtimeDebugOptionalRow("Cached input", span.cachedInputTokens.map(formatRuntimeInteger))
        runtimeDebugOptionalRow("Output tokens", span.outputTokens.map(formatRuntimeInteger))
        runtimeDebugOptionalRow("Total tokens", span.totalTokens.map(formatRuntimeInteger))
      }
    }
  }
}

private struct RuntimeDebugUsageDetails: View {
  let usage: RuntimeDebugUsage

  var body: some View {
    NoemaSectionSurface("Provider usage") {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        runtimeDebugRow("Provider", usage.provider)
        runtimeDebugRow("Model", usage.model)
        runtimeDebugRow("Phase", usage.phase)
        runtimeDebugRow("Response index", String(usage.responseIndex))
        runtimeDebugOptionalRow("Output index", usage.outputIndex.map(String.init))
        runtimeDebugRow("Input tokens", formatRuntimeInteger(usage.inputTokens))
        runtimeDebugOptionalRow("Cached input", usage.cachedInputTokens.map(formatRuntimeInteger))
        if let cacheHitRatio = usage.cacheHitRatio {
          runtimeDebugRow("Cache hit ratio", "\(Int((cacheHitRatio * 100).rounded()))%")
        }
        runtimeDebugRow("Output tokens", formatRuntimeInteger(usage.outputTokens))
        runtimeDebugRow("Total tokens", formatRuntimeInteger(usage.totalTokens))
      }
    }
  }
}

private func runtimeDebugFocusedSpan(_ spans: [RuntimeDebugSpanSnapshot], focus: RuntimeDebugFocus?) -> RuntimeDebugSpanSnapshot? {
  guard let focus else { return nil }
  switch focus {
  case let .tool(correlationID):
    guard let correlationID else { return nil }
    return spans.first { $0.correlationID == correlationID }
  case let .provider(phase, responseIndex, roundIndex):
    return spans.first {
      $0.category == .provider
        && (phase == nil || $0.phase == phase)
        && (responseIndex == nil || $0.responseIndex == responseIndex)
        && (roundIndex == nil || $0.roundIndex == roundIndex)
    }
  }
}

private func runtimeCoveredDuration(_ spans: [RuntimeDebugSpanSnapshot]) -> Int {
  let intervals = spans
    .map { (start: max($0.startOffsetMilliseconds, 0), end: max($0.startOffsetMilliseconds, 0) + max($0.durationMilliseconds, 0)) }
    .sorted { $0.start < $1.start }
  var total = 0
  var current: (start: Int, end: Int)?
  for interval in intervals {
    guard let active = current else {
      current = interval
      continue
    }
    if interval.start <= active.end {
      current = (active.start, max(active.end, interval.end))
    } else {
      total += max(active.end - active.start, 0)
      current = interval
    }
  }
  if let active = current { total += max(active.end - active.start, 0) }
  return total
}

@ViewBuilder
private func runtimeDebugOptionalRow(_ label: String, _ value: String?) -> some View {
  if let value { runtimeDebugRow(label, value) }
}

private func runtimeDebugRow(_ label: String, _ value: String) -> some View {
  HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.md) {
    Text(label)
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
    Spacer(minLength: NoemaSpacing.sm)
    Text(value)
      .font(NoemaFont.mono)
      .foregroundStyle(NoemaColor.content)
      .multilineTextAlignment(.trailing)
      .textSelection(.enabled)
  }
}

private func humanizeRuntimeStatus(_ status: RuntimeDebugStatus) -> String {
  status.rawValue.lowercased().replacingOccurrences(of: "_", with: " ").capitalized
}

private func formatRuntimeDuration(_ milliseconds: Int) -> String {
  if milliseconds < 1_000 { return "\(milliseconds) ms" }
  let seconds = Double(milliseconds) / 1_000
  if seconds < 60 { return String(format: "%.2f s", seconds) }
  return String(format: "%.1f min", seconds / 60)
}

private func formatRuntimeInteger(_ value: Int) -> String {
  let formatter = NumberFormatter()
  formatter.numberStyle = .decimal
  return formatter.string(from: NSNumber(value: value)) ?? String(value)
}
