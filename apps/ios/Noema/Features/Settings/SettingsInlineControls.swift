import NoemaAPI
import SwiftUI

struct SettingsInlineModelControls: View {
  @Environment(\.dynamicTypeSize) private var dynamicTypeSize
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  let preference: SettingsPreference?
  let options: [SettingsModelOption]
  let useCase: NoemaAPI.NoemaModelUseCase
  let enabled: Bool
  var requiresExplicitSelection = false
  let save: (SettingsModelOption, String?, String?, NoemaAPI.ModelPreferenceSelectionMode) async -> Bool

  private var option: SettingsModelOption? {
    if requiresExplicitSelection, preference == nil { return nil }
    return options.first { $0.providerAccountId == preference?.providerAccountId } ?? options.first
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

  private var usesStackedLayout: Bool {
    horizontalSizeClass == .compact || dynamicTypeSize.isAccessibilitySize
  }

  var body: some View {
    Group {
      if usesStackedLayout {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          modelMenu
          if profile?.reasoningEfforts.isEmpty == false { reasoningMenu }
        }
      } else {
        HStack(spacing: NoemaSpacing.sm) {
          modelMenu
          if profile?.reasoningEfforts.isEmpty == false { reasoningMenu }
        }
      }
    }
    .font(NoemaFont.body)
  }

  private var modelMenu: some View {
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
        Text(profile?.label ?? (isRecommended ? "Noema recommended" : (requiresExplicitSelection ? "Select a model" : "No model available")))
          .foregroundStyle(NoemaColor.content)
          .lineLimit(usesStackedLayout ? nil : 1)
          .multilineTextAlignment(.leading)
        Spacer(minLength: NoemaSpacing.xs)
        Image(systemName: "chevron.down").font(NoemaFont.metadata).foregroundStyle(NoemaColor.contentTertiary)
      }
      .padding(.horizontal, NoemaSpacing.md)
      .frame(maxWidth: .infinity, minHeight: 34)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay { RoundedRectangle(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .disabled(!enabled || options.isEmpty)
  }

  private var reasoningMenu: some View {
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
        Text(reasoning?.replacingOccurrences(of: "_", with: " ").capitalized ?? "Default")
          .lineLimit(usesStackedLayout ? nil : 1)
          .multilineTextAlignment(.leading)
        Spacer(minLength: 0)
        Image(systemName: "chevron.down").font(NoemaFont.metadata)
      }
      .foregroundStyle(NoemaColor.contentTertiary)
      .padding(.horizontal, NoemaSpacing.md)
      .frame(
        minWidth: usesStackedLayout ? 0 : 108,
        maxWidth: usesStackedLayout ? .infinity : 108,
        minHeight: 34
      )
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay { RoundedRectangle(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .disabled(!enabled || profile?.reasoningEfforts.isEmpty != false || isRecommended)
    .opacity(isRecommended ? 0.62 : 1)
  }

  private func recommendationLabel(_ recommendation: SettingsModelRecommendation, in option: SettingsModelOption) -> String {
    option.profiles.first { $0.id == recommendation.modelProfile }?.label ?? recommendation.modelProfile
  }
}

struct SettingsCompactToggleStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    HStack(spacing: 0) {
      configuration.label
      ZStack(alignment: configuration.isOn ? .trailing : .leading) {
        Capsule().fill(configuration.isOn ? NoemaColor.clay600 : NoemaColor.surfaceTertiary)
        Circle().fill(NoemaColor.surface).padding(NoemaSpacing.xxs)
      }
      .frame(width: 40, height: 24)
    }
    .contentShape(Rectangle().inset(by: -10))
    .onTapGesture { configuration.isOn.toggle() }
    .accessibilityElement(children: .combine)
    .accessibilityValue(configuration.isOn ? "On" : "Off")
    .accessibilityAddTraits(.isToggle)
    .accessibilityAction {
      configuration.isOn.toggle()
    }
  }
}
