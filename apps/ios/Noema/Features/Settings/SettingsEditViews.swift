import NoemaAPI
import SwiftUI

enum SettingsPreferenceKind: Hashable {
  case agent(String)
  case taskPool(SettingsTaskModelPool)
  case memory
  case webFetch
  case privacy
  case usage
  case defaultModel
}

struct SettingsPreferenceTarget: Identifiable {
  let id: String
  let title: String
  let kind: SettingsPreferenceKind
  let preference: SettingsPreference?
  let options: [SettingsModelOption]
  var requiresExplicitSelection: Bool = false
}

private struct SettingsPreferenceDraft {
  var providerAccountID: String
  var selectionMode: String
  var modelProfile: String?
  var reasoningEffort: String?
}

struct SettingsPreferenceEditor: View {
  let target: SettingsPreferenceTarget
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var draft: SettingsPreferenceDraft
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Field?

  private enum Field: Hashable {
    case provider
  }

  private var initialSelectionMode: String {
    target.preference?.selectionMode
      ?? (target.requiresExplicitSelection ? "" : NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue)
  }

  private var isDirty: Bool {
    draft.providerAccountID != (target.preference?.providerAccountId ?? target.options.first?.providerAccountId ?? "")
      || draft.selectionMode != initialSelectionMode
      || draft.modelProfile != target.preference?.modelProfile
      || draft.reasoningEffort != target.preference?.reasoningEffort
  }

  init(target: SettingsPreferenceTarget, settings: SettingsModel) {
    self.target = target
    self.settings = settings
    let preference = target.preference
    _draft = State(initialValue: SettingsPreferenceDraft(
      providerAccountID: preference?.providerAccountId ?? target.options.first?.providerAccountId ?? "",
      selectionMode: preference?.selectionMode ?? (target.requiresExplicitSelection ? "" : NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue),
      modelProfile: preference?.modelProfile,
      reasoningEffort: preference?.reasoningEffort
    ))
  }

  var body: some View {
    SettingsBottomSheet(
      title: target.title,
      subtitle: "Choose the provider route and model profile for this setting.",
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if target.options.isEmpty {
          NoemaInlineState(message: "No model providers are available.", symbol: "server.rack", tone: .warning)
        } else {
          SettingsSheetField("Provider") {
            Picker("Provider", selection: $draft.providerAccountID) {
              ForEach(target.options) { option in
                Text("\(option.providerDisplayName) · \(option.providerKind)")
                  .tag(option.providerAccountId)
              }
            }
            .pickerStyle(.menu)
            .tint(NoemaColor.content)
            .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
            .padding(.horizontal, NoemaSpacing.md)
            .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
            .overlay {
              NoemaSuperellipse(cornerRadius: NoemaRadius.element)
                .stroke(focusedField == .provider ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focusedField == .provider ? 2 : 1)
            }
            .focused($focusedField, equals: .provider)
          }
          SettingsSheetField("Selection") {
            Picker("Selection", selection: $draft.selectionMode) {
              if target.requiresExplicitSelection && draft.selectionMode.isEmpty {
                Text("Choose a reviewer").tag("")
              }
              if !target.requiresExplicitSelection {
                Text("Noema recommended").tag(NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue)
              }
              Text("Explicit profile").tag(NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue)
            }
            .pickerStyle(.menu)
            .tint(NoemaColor.content)
            .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
            .padding(.horizontal, NoemaSpacing.md)
            .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
            .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
          }
          if draft.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue,
             let option = target.options.first(where: { $0.providerAccountId == draft.providerAccountID }),
             !option.profiles.isEmpty {
            SettingsSheetField("Profile") {
              Picker("Profile", selection: Binding(
                get: { draft.modelProfile ?? option.profiles.first?.id ?? "" },
                set: { draft.modelProfile = $0 }
              )) {
                ForEach(option.profiles) { profile in
                  Text(profile.label).tag(profile.id)
                }
              }
              .pickerStyle(.menu)
              .tint(NoemaColor.content)
              .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
              .padding(.horizontal, NoemaSpacing.md)
              .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
              .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
            }
            let profile = option.profiles.first(where: { $0.id == draft.modelProfile })
            if let profile, !profile.reasoningEfforts.isEmpty {
              SettingsSheetField("Reasoning") {
                Picker("Reasoning", selection: Binding(
                  get: { draft.reasoningEffort ?? profile.defaultReasoningEffort ?? profile.reasoningEfforts[0] },
                  set: { draft.reasoningEffort = $0 }
                )) {
                  ForEach(profile.reasoningEfforts, id: \.self) { effort in
                    Text(effort.replacingOccurrences(of: "_", with: " ").capitalized).tag(effort)
                  }
                }
                .pickerStyle(.menu)
                .tint(NoemaColor.content)
                .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
                .padding(.horizontal, NoemaSpacing.md)
                .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
                .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
              }
            }
          }
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.danger)
        }
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { requestDismissal() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .disabled(isSaving)
          Button { save() } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if isSaving { ProgressView().controlSize(.small) }
              Text("Save")
            }
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.clay600, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .opacity(isSaving || draft.providerAccountID.isEmpty || draft.selectionMode.isEmpty || !settings.canMutate ? 0.42 : 1)
          .disabled(isSaving || draft.providerAccountID.isEmpty || draft.selectionMode.isEmpty || !settings.canMutate)
        }
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .noemaSheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard model changes?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .task { focusedField = target.options.isEmpty ? nil : .provider }
    .onChange(of: draft.providerAccountID) { _, _ in
      draft.modelProfile = nil
      draft.reasoningEffort = nil
    }
    .onChange(of: draft.selectionMode) { _, mode in
      guard mode == NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue else { return }
      draft.modelProfile = nil
      draft.reasoningEffort = nil
    }
    .onChange(of: draft.modelProfile) { _, _ in
      draft.reasoningEffort = nil
    }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    isSaving = true
    Task {
      let explicit = draft.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue
      let option = target.options.first { $0.providerAccountId == draft.providerAccountID }
      let modelProfile = explicit ? (draft.modelProfile ?? option?.profiles.first?.id) : nil
      let profile = option?.profiles.first { $0.id == modelProfile }
      let reasoningEffort = explicit
        ? (draft.reasoningEffort ?? profile?.defaultReasoningEffort ?? profile?.reasoningEfforts.first)
        : nil
      let success: Bool
      switch target.kind {
      case .agent(let agentID):
        success = await settings.saveAgentModelPreference(agentID: agentID, providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      case .taskPool(let pool):
        guard let option else { isSaving = false; return }
        success = await settings.updateTaskModelPool(
          pool,
          preference: SettingsPreference(
            providerKind: option.providerKind,
            providerAccountId: draft.providerAccountID,
            modelProfile: modelProfile,
            reasoningEffort: reasoningEffort,
            selectionMode: draft.selectionMode
          )
        )
      case .memory:
        success = await settings.saveMemoryModelPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      case .webFetch:
        success = await settings.saveWebFetchSummarizerPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      case .privacy:
        success = await settings.saveActionReviewerPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      case .usage:
        success = await settings.saveToolProgressAuditPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      case .defaultModel:
        guard let option else { isSaving = false; return }
        success = await settings.saveDefaultModelPreference(providerKind: option.providerKind, providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: modelProfile, reasoningEffort: reasoningEffort)
      }
      if success { dismiss() } else { isSaving = false }
    }
  }
}
