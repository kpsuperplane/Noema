import Foundation
import SwiftUI

struct ActionRequestReviewContent: View {
  let action: GovernedActionModel

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(action.question)
        .font(NoemaFont.taskTitle)
        .foregroundStyle(NoemaColor.content)
      Text(action.consequence)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
        .fixedSize(horizontal: false, vertical: true)
      DisclosureGroup("Review details") {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          if let serviceName = nonblank(action.serviceName) {
            detail("Service", serviceName)
          }
          if let account = nonblank(action.connectionLabel) {
            detail("Account", account)
          }
          if let sharedContent = nonblank(action.sharedContent) {
            detail("Shared content", sharedContent)
          }
          detail("Effect", action.effect)
          if let summary = nonblank(action.summary) {
            detail("Request", summary)
          }
          Text(action.capabilityName)
            .font(NoemaFont.monoTiny)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(action.arguments)
            .font(NoemaFont.monoTiny)
            .foregroundStyle(NoemaColor.content)
            .textSelection(.enabled)
            .padding(NoemaSpacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(NoemaColor.paper100, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        }
        .padding(.top, NoemaSpacing.xs)
      }
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  private func detail(_ label: String, _ value: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
      Text(label)
        .font(NoemaFont.metadata.weight(.semibold))
        .foregroundStyle(NoemaColor.contentTertiary)
      Text(value)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  private func nonblank(_ value: String?) -> String? {
    let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return trimmed.isEmpty ? nil : trimmed
  }
}
