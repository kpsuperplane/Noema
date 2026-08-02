import Apollo
import Foundation
import SwiftUI

/// Chat references stay in the conversation while opening the same large,
/// mobile-shaped detail sheet used by Work.
struct ChatTaskDetailSheet: View {
  let client: ApolloClient?
  let profile: NoemaProfile?
  let taskID: String
  @State private var tasksModel: TasksModel?

  var body: some View {
    Group {
      if let tasksModel {
        TasksDetailRoute(model: tasksModel, taskId: taskID, compactPresentation: true)
      } else if client == nil {
        ContentUnavailableView {
          Label("Task unavailable", systemImage: "checklist")
        } description: {
          Text("Connect this device to load the task details.")
        }
      } else {
        ProgressView("Loading task…")
          .frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
    .background(NoemaColor.surface)
    .noemaMobileDrawerPresentation()
    .task(id: taskID) {
      guard tasksModel == nil, let client else { return }
      tasksModel = TasksModel(client: client, profile: profile)
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

func runtimeDebugUsage(for message: ChatMessage) -> RuntimeDebugUsage? {
  guard let metadata = message.metadata,
        let data = metadata.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
        let usage = object["provider_usage"] as? [String: Any]
  else { return nil }
  return makeRuntimeDebugUsage(usage)
}

func runtimeDebugUsage(from messages: [ChatMessage]) -> RuntimeDebugUsage? {
  messages.reversed().compactMap(runtimeDebugUsage(for:)).first
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
    ?? (cachedInputTokens.map { inputTokens > 0 ? Double($0) / Double(inputTokens) : 0 })
  let totalTokens = integerValue(usage["total_tokens"]) ?? inputTokens + outputTokens
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

struct RuntimeDebugSheet: View {
  @Environment(\.dismiss) private var dismiss
  let usage: RuntimeDebugUsage

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack(spacing: NoemaSpacing.sm) {
        Text("Turn runtime")
          .font(NoemaFont.mobileTitle)
          .foregroundStyle(NoemaColor.content)
        Spacer(minLength: NoemaSpacing.sm)
        Button("Close", systemImage: "xmark") { dismiss() }
          .labelStyle(.iconOnly)
          .buttonStyle(.glass)
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.vertical, NoemaSpacing.sm)

      ScrollView {
        NoemaSectionSurface("Provider usage") {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            runtimeDebugRow("Provider", usage.provider)
            runtimeDebugRow("Model", usage.model)
            runtimeDebugRow("Phase", usage.phase)
            runtimeDebugRow("Response index", String(usage.responseIndex))
            if let outputIndex = usage.outputIndex {
              runtimeDebugRow("Output index", String(outputIndex))
            }
            runtimeDebugRow("Input tokens", formatRuntimeInteger(usage.inputTokens))
            runtimeDebugRow("Cached input tokens", usage.cachedInputTokens.map(formatRuntimeInteger) ?? "Unavailable")
            if let cacheHitRatio = usage.cacheHitRatio {
              runtimeDebugRow("Cache hit ratio", "\(Int((cacheHitRatio * 100).rounded()))%")
            }
            runtimeDebugRow("Output tokens", formatRuntimeInteger(usage.outputTokens))
            runtimeDebugRow("Total tokens", formatRuntimeInteger(usage.totalTokens))
          }
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.vertical, NoemaSpacing.md)
      }
    }
    .background(NoemaColor.surface)
    .presentationDetents([.medium, .large])
    .presentationDragIndicator(.visible)
    .presentationCornerRadius(NoemaRadius.container)
    .presentationBackground(NoemaColor.surface)
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
}

private func formatRuntimeInteger(_ value: Int) -> String {
  let formatter = NumberFormatter()
  formatter.numberStyle = .decimal
  return formatter.string(from: NSNumber(value: value)) ?? String(value)
}
