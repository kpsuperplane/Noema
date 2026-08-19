import SwiftUI

struct TasksCaptureSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  @State private var title = ""
  @State private var taskDocument = ""
  @State private var projectId: String?
  @State private var scheduling = false
  @State private var scheduleDraft = TasksScheduleDraft.initial()
  @State private var executorAgentId = "agent:task-executor"
  @State private var cwdOverride = ""
  @FocusState private var focusedField: Field?
  @State private var isSaving = false
  @State private var discardPresented = false
  @State private var errorMessage: String?

  private enum Field: Hashable {
    case title
    case taskDocument
  }

  init(model: TasksModel) {
    self.model = model
    _projectId = State(initialValue: model.selectedProjectId)
  }

  var body: some View {
    NoemaNativeSheet(
      title: "New task",
      dismissDisabled: isSaving,
      onDismiss: requestDismissal
    ) {
      ScrollView {
        VStack(alignment: .leading, spacing: 0) {
          Text(scheduling ? "Runs automatically at the time you choose." : "Saved to Inbox until you queue it.")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, NoemaSpacing.lg)
            .padding(.top, NoemaSpacing.md)
            .padding(.bottom, 19)

          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            TasksSheetField("Title") {
              TextField("", text: $title)
                .textInputAutocapitalization(.sentences)
                .focused($focusedField, equals: .title)
                .noemaTaskSheetField(focused: focusedField == .title, height: 46)
            }
            TasksSheetField("Task document") {
              TasksMarkdownSourceEditor(text: $taskDocument)
                .focused($focusedField, equals: .taskDocument)
            }
            TasksSheetField("Project (optional)") {
              Picker(selection: $projectId) {
                Text("No project").tag(Optional<String>.none)
                ForEach(model.projects.filter { $0.archivedAt == nil }) { project in
                  Text(project.name).tag(Optional(project.id))
                }
              } label: {
                Text("Project (optional)")
              }
              .pickerStyle(.menu)
              .tint(NoemaColor.content)
              .frame(maxWidth: .infinity, minHeight: 38, maxHeight: 38, alignment: .leading)
              .padding(.horizontal, NoemaSpacing.md)
              .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
              .overlay {
                NoemaSuperellipse(cornerRadius: NoemaRadius.element)
                  .stroke(NoemaColor.separator, lineWidth: 1)
              }
              .padding(.top, NoemaSpacing.xs)
            }
            .padding(.top, NoemaSpacing.compact)
            Toggle("Schedule for later", isOn: $scheduling)
              .font(NoemaFont.body)
              .tint(NoemaColor.pine500)
            if scheduling {
              TasksScheduleFields(model: model, draft: $scheduleDraft, recurringOnly: false)
            }
            TasksExecutorFields(model: model, executorAgentId: $executorAgentId, cwdOverride: $cwdOverride)
          }
          .padding(.horizontal, NoemaSpacing.lg)

          if let errorMessage {
            NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .warning)
              .padding(.horizontal, NoemaSpacing.lg)
              .padding(.top, NoemaSpacing.md)
          }

          HStack(spacing: NoemaSpacing.sm) {
            Spacer(minLength: 0)
            Button("Cancel") { requestDismissal() }
              .buttonStyle(.plain)
              .font(NoemaFont.body)
              .disabled(isSaving)
            Button {
              Task {
                isSaving = true
                errorMessage = nil
                let succeeded = await model.capture(
                  title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                  taskDocument: taskDocument,
                  projectId: projectId,
                  schedule: scheduling ? scheduleDraft.input() : nil,
                  executorAgentId: executorAgentId,
                  cwdOverride: cwdOverride.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : cwdOverride.trimmingCharacters(in: .whitespacesAndNewlines)
                )
                isSaving = false
                if succeeded {
                  dismiss()
                } else {
                  errorMessage = model.lastError ?? "The task could not be captured. Try again."
                }
              }
            } label: {
              Text(scheduling ? "Schedule task" : "Add to Inbox")
                .frame(minHeight: 32)
                .padding(.horizontal, NoemaSpacing.md)
              .overlay {
                if isSaving { ProgressView().controlSize(.small) }
              }
            }
            .buttonStyle(.plain)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.white)
            .background(NoemaColor.clay600, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
            .opacity(canSave ? 1 : 0.42)
            .disabled(!canSave)
          }
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
        }
      }
      .scrollBounceBehavior(.basedOnSize)
      .frame(maxHeight: .infinity, alignment: .top)
      .background(NoemaColor.surface)
    }
    .interactiveDismissDisabled(isDirty || isSaving)
    .noemaSheet(isPresented: $discardPresented) {
      TasksDiscardSheet(
        title: "Discard capture?",
        message: "Your new task draft will be lost.",
        confirmTitle: "Discard capture"
      ) {
        dismiss()
      }
    }
    .task {
      focusedField = .title
    }
  }

  private var canSave: Bool {
    !isSaving && !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.isConnected && (!scheduling || scheduleDraft.input() != nil)
  }

  private func requestDismissal() {
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private var isDirty: Bool {
      !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      !taskDocument.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      projectId != nil || scheduling || !cwdOverride.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || executorAgentId != "agent:task-executor"
  }
}

struct TasksMarkdownSourceEditor: View {
  @Binding var text: String

  var body: some View {
    TextEditor(text: $text)
      .font(.system(.body, design: .monospaced))
      .foregroundStyle(NoemaColor.content)
      .scrollContentBackground(.hidden)
      .padding(NoemaSpacing.sm)
      .frame(minHeight: 280, alignment: .topLeading)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
      .accessibilityLabel("Markdown source")
  }
}

extension View {
  func noemaTaskSheetField(focused: Bool, height: CGFloat) -> some View {
    font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .textFieldStyle(.plain)
      .padding(.horizontal, NoemaSpacing.md)
      .frame(height: height, alignment: .topLeading)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.element)
          .stroke(focused ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focused ? 2 : 1)
      }
  }

  func noemaTaskSheetPresentation(
    _ compactDetents: Set<PresentationDetent>,
    regularHeight: CGFloat,
    compactDragIndicator: Visibility = .visible
  ) -> some View {
    modifier(TasksSheetPresentationModifier(
      compactDetents: compactDetents,
      regularHeight: regularHeight,
      compactDragIndicator: compactDragIndicator
    ))
  }
}

private struct TasksSheetPresentationModifier: ViewModifier {
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  let compactDetents: Set<PresentationDetent>
  let regularHeight: CGFloat
  let compactDragIndicator: Visibility

  func body(content: Content) -> some View {
    if horizontalSizeClass == .compact {
      content
        .presentationDetents(compactDetents)
        .presentationSizing(.page)
        .presentationDragIndicator(compactDragIndicator)
        .presentationContentInteraction(.scrolls)
    } else {
      content
        .frame(minWidth: 460, idealWidth: 520, maxWidth: 580, minHeight: 300, idealHeight: regularHeight, maxHeight: 680)
        .presentationSizing(.form)
    }
  }
}

struct TasksSheetField<Content: View>: View {
  let label: String
  @ViewBuilder let content: () -> Content

  init(_ label: String, @ViewBuilder content: @escaping () -> Content) {
    self.label = label
    self.content = content
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.taskTitle)
        .foregroundStyle(NoemaColor.content)
      content()
    }
  }
}

struct TasksDiscardSheet: View {
  @Environment(\.dismiss) private var dismiss
  let title: String
  let message: String
  let confirmTitle: String
  let onDiscard: () -> Void

  init(title: String, message: String, confirmTitle: String = "Discard", onDiscard: @escaping () -> Void) {
    self.title = title
    self.message = message
    self.confirmTitle = confirmTitle
    self.onDiscard = onDiscard
  }

  var body: some View {
    NoemaNativeSheet(title: title, onDismiss: { dismiss() }) {
      VStack(alignment: .leading, spacing: 0) {
        Text(message)
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, NoemaSpacing.lg)
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Keep editing") { dismiss() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
          Button(confirmTitle, role: .destructive) {
            onDiscard()
            dismiss()
          }
          .buttonStyle(.plain)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.danger)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.xl)
        .padding(.bottom, NoemaSpacing.sm)
      }
      .frame(maxHeight: .infinity, alignment: .top)
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.height(194)], regularHeight: 300)
  }
}
