import NoemaAPI
import SwiftUI

struct SettingsInlineModelControls: View {
  let preference: SettingsPreference?
  let options: [SettingsModelOption]
  let useCase: NoemaAPI.NoemaModelUseCase
  let enabled: Bool
  var requiresExplicitSelection = false
  let save: (SettingsModelOption, String?, String?, NoemaAPI.ModelPreferenceSelectionMode) async -> Bool

  private var option: SettingsModelOption? {
    options.first { $0.providerAccountId == preference?.providerAccountId } ?? options.first
  }

  private var recommendation: SettingsModelRecommendation? {
    option?.recommendations.first { $0.useCase == useCase }
  }

  private var isRecommended: Bool {
    if preference?.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue { return true }
    guard !requiresExplicitSelection, preference == nil, let recommendation else { return false }
    return recommendation.disabledReason == nil
  }

  private var profile: SettingsModelProfile? {
    guard let option else { return nil }
    if isRecommended, let recommendation {
      return option.profiles.first { $0.id == recommendation.modelProfile }
    }
    return option.profiles.first { $0.id == preference?.modelProfile } ?? option.profiles.first
  }

  private var reasoning: String? {
    if isRecommended { return recommendation?.reasoningEffort }
    return preference?.reasoningEffort ?? profile?.defaultReasoningEffort ?? profile?.reasoningEfforts.first
  }

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Menu {
        ForEach(options) { option in
          if let recommendation = option.recommendations.first(where: { $0.useCase == useCase }) {
            Button("Noema recommended · \(recommendationLabel(recommendation, in: option))") {
              Task { _ = await save(option, nil, nil, .noemaRecommended) }
            }
            .disabled(option.disabledReason != nil || recommendation.disabledReason != nil)
          }
          ForEach(option.profiles.filter { $0.disabledReason == nil }) { profile in
            Button(profile.label) {
              Task { _ = await save(option, profile.id, profile.defaultReasoningEffort ?? profile.reasoningEfforts.first, .explicitProfile) }
            }
            .disabled(option.disabledReason != nil)
          }
        }
      } label: {
        HStack(spacing: NoemaSpacing.sm) {
          Image(systemName: "sparkles").foregroundStyle(NoemaColor.clay600)
          Text(isRecommended ? "Noema recommended" : (profile?.label ?? "No model available"))
            .foregroundStyle(NoemaColor.content)
            .lineLimit(1)
          Spacer(minLength: NoemaSpacing.xs)
          Image(systemName: "chevron.down").font(NoemaFont.metadata).foregroundStyle(NoemaColor.contentTertiary)
        }
        .padding(.horizontal, NoemaSpacing.md)
        .frame(maxWidth: .infinity, minHeight: 34)
        .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        .overlay { RoundedRectangle(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
      }
      .buttonStyle(.plain)
      .disabled(!enabled || (profile == nil && recommendation == nil))

      Menu {
        if let option, let profile {
          ForEach(profile.reasoningEfforts, id: \.self) { effort in
            Button(effort.replacingOccurrences(of: "_", with: " ").capitalized) {
              Task { _ = await save(option, profile.id, effort, .explicitProfile) }
            }
          }
        }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          Text(reasoning?.replacingOccurrences(of: "_", with: " ").capitalized ?? "Default").lineLimit(1)
          Spacer(minLength: 0)
          Image(systemName: "chevron.down").font(NoemaFont.metadata)
        }
        .foregroundStyle(NoemaColor.contentTertiary)
        .padding(.horizontal, NoemaSpacing.md)
        .frame(width: 108)
        .frame(minHeight: 34)
        .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        .overlay { RoundedRectangle(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
      }
      .buttonStyle(.plain)
      .disabled(!enabled || profile?.reasoningEfforts.isEmpty != false || isRecommended)
      .opacity(isRecommended ? 0.62 : 1)
    }
    .font(NoemaFont.body)
  }

  private func recommendationLabel(_ recommendation: SettingsModelRecommendation, in option: SettingsModelOption) -> String {
    option.profiles.first { $0.id == recommendation.modelProfile }?.label ?? recommendation.modelProfile
  }
}

struct SettingsCompactToggleStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    ZStack(alignment: configuration.isOn ? .trailing : .leading) {
      Capsule().fill(configuration.isOn ? NoemaColor.clay600 : NoemaColor.surfaceTertiary)
      Circle().fill(NoemaColor.surface).padding(NoemaSpacing.xxs)
    }
    .frame(width: 40, height: 24)
    .contentShape(.interaction, Rectangle().inset(by: -10))
    .onTapGesture { configuration.isOn.toggle() }
  }
}
