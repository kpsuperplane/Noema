import MarkdownUI
import NoemaAPI
import SwiftUI

struct TasksDetailRoute: View {
  @Bindable var model: TasksModel
  let taskId: String
  var compactPresentation = false
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    Group {
      if compactPresentation {
        NoemaNativeSheet(title: taskTitle, onDismiss: { dismiss() }) {
          routeContent
        }
      } else {
        routeContent
      }
    }
    .task(id: taskId) {
      await model.loadDetail(taskId: taskId)
    }
    .onDisappear {
      model.clearDetail(taskId: taskId)
    }
  }

  @ViewBuilder
  private var routeContent: some View {
    VStack(spacing: 0) {
      if let detail = model.detail, detail.id == taskId {
        TasksDetailContent(model: model, detail: detail, compactPresentation: compactPresentation)
      } else if model.isLoadingDetail {
        ProgressView("Loading task…")
          .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        NoemaDeckState(
          title: "Task unavailable",
          message: model.lastError ?? "This task is no longer available in the current workspace.",
          symbol: "checklist",
          tone: .warning,
          actionTitle: model.isConnected ? "Try again" : nil,
          action: model.isConnected ? { Task { await model.loadDetail(taskId: taskId) } } : nil
        )
      }
    }
    .background(NoemaColor.surface)
  }

  private var taskTitle: String {
    if let detail = model.detail, detail.id == taskId {
      return detail.title
    }
    return (model.tasks + model.history).first(where: { $0.id == taskId })?.title ?? "Task"
  }
}

private struct TasksDetailContent: View {
  @Bindable var model: TasksModel
  let detail: TasksDetailSnapshot
  let compactPresentation: Bool
  @State private var editPresented = false
  @State private var reopenPresented = false
  @State private var queuePresented = false
  @State private var cancelPresented = false
  @State private var selectedArtifact: ArtifactSelection?
  @State private var taskInfoPresented = false
  @State private var validationPresented = false
  @State private var gateResponse = ""
  @State private var selectedTab: TaskResultTab = .result
  @State private var followsTranscriptBottom = true
  @State private var initialScrollTaskID: String?
  @State private var completedInitialHydration = false

  var body: some View {
    VStack(spacing: 0) {
      ScrollViewReader { reader in
        ScrollView {
          VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if let completed = detail.completedResult {
          Picker("Task view", selection: $selectedTab) {
            ForEach(TaskResultTab.allCases) { tab in
              Text(tab.title).tag(tab)
            }
          }
          .pickerStyle(.segmented)

          if selectedTab == .result {
            TasksSubmissionSection(submission: completed, title: "Accepted result", onArtifact: openArtifact)
          } else {
            TasksTranscriptSection(
              messages: detail.messages,
              runs: detail.runs,
              runItems: model.runItems,
              hasMore: model.hasMoreRunItems,
              isLoadingMore: model.isLoadingOlderRunItems,
              request: detail.description.nilIfBlank ?? detail.title,
              submission: completed,
              loadMore: { Task { await model.loadOlderRunItems() } },
              onArtifact: openArtifact
            )
          }
        } else {
          TasksTranscriptSection(
            messages: detail.messages,
            runs: detail.runs,
            runItems: model.runItems,
            hasMore: model.hasMoreRunItems,
            isLoadingMore: model.isLoadingOlderRunItems,
            request: detail.description.nilIfBlank ?? detail.title,
            submission: detail.latestSubmission,
            loadMore: { Task { await model.loadOlderRunItems() } },
            onArtifact: openArtifact
          )
        }
          Color.clear
            .frame(height: compactPresentation ? 27 : 1)
            .id("task-transcript-bottom")
          }
          .frame(maxWidth: 820, alignment: .leading)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.vertical, NoemaSpacing.lg)
          .frame(maxWidth: .infinity, alignment: .center)
        }
        .scrollDismissesKeyboard(.interactively)
        .onScrollGeometryChange(for: Bool.self) { geometry in
          geometry.contentOffset.y + geometry.containerSize.height >= geometry.contentSize.height - 44
        } action: { _, isAtBottom in
          followsTranscriptBottom = isAtBottom
        }
        .task(id: transcriptFollowKey) {
          await Task.yield()
          if initialScrollTaskID != detail.id {
            initialScrollTaskID = detail.id
            completedInitialHydration = false
          }
          if model.isLoadingDetail || !completedInitialHydration {
            followsTranscriptBottom = true
            reader.scrollTo("task-transcript-bottom", anchor: .bottom)
            if !model.isLoadingDetail { completedInitialHydration = true }
          } else if followsTranscriptBottom {
            reader.scrollTo("task-transcript-bottom", anchor: .bottom)
          }
        }
      }
    }
    .navigationTitle(detail.title)
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      if !compactPresentation || hasTaskActions {
        ToolbarItem(placement: .topBarTrailing) {
          taskActionsMenu
        }
      }
    }
    .sheet(isPresented: $editPresented) {
      TasksInboxEditSheet(model: model, task: detail)
    }
    .sheet(isPresented: $reopenPresented) {
      TasksReopenSheet(model: model, task: detail)
    }
    .sheet(isPresented: $queuePresented) {
      TasksQueueSheet(model: model, task: detail)
    }
    .sheet(item: $selectedArtifact) { selection in
      ArtifactVersionSheet(model: ArtifactModel(client: model.client, profile: model.profile), selection: selection)
    }
    .sheet(isPresented: $cancelPresented) {
      TasksCancelSheet(model: model, task: detail)
    }
    .sheet(isPresented: $taskInfoPresented) {
      TasksTaskInfoSheet(detail: detail)
    }
    .sheet(isPresented: $validationPresented) {
      TasksValidationSheet(criteria: detail.latestSubmission?.criteria ?? detail.criteria)
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      if showsContextDock {
        VStack(spacing: 0) {
          if !taskInterventions.isEmpty {
            TasksHumanInterventionsView(model: model, interventions: taskInterventions)
              .padding(.horizontal, NoemaSpacing.lg)
              .padding(.bottom, NoemaSpacing.md)
          }
          if let gate = detail.activeGate {
            TasksGatePanel(
              gate: gate,
              response: $gateResponse,
              isConnected: model.isConnected,
              isSubmitting: model.commandIsPending(taskID: detail.id),
              errorMessage: model.commandError(taskID: detail.id),
              canAnswer: hasAction("ANSWER"),
              canRetry: hasAction("RETRY"),
              answer: { answer, approval in
                await model.answer(task: detail, answer: answer, approval: approval)
              },
              retry: { note in
                await model.retry(task: detail, note: note)
              }
            )
            .padding(.horizontal, NoemaSpacing.lg)
          }
          TasksTaskContextDock(
            run: detail.currentRun ?? detail.runs.max(by: { runDate($0) < runDate($1) }),
            activity: latestRunActivity,
            criteria: detail.latestSubmission?.criteria ?? detail.criteria,
            canCancel: hasAction("CANCEL") && model.isConnected,
            cancel: { cancelPresented = true },
            showInfo: { taskInfoPresented = true },
            showValidation: { validationPresented = true }
          )
          .padding(.horizontal, NoemaSpacing.lg)
          .offset(y: detail.activeGate == nil ? 0 : -NoemaSpacing.xxl)
          .padding(.bottom, detail.activeGate == nil ? 0 : -NoemaSpacing.xxl)
        }
      }
    }
    .onChange(of: detail.activeGate?.id) { _, _ in
      gateResponse = ""
    }
    .onChange(of: detail.id) { _, _ in
      gateResponse = ""
      selectedTab = .result
    }
  }

  private var taskActionsMenu: some View {
    Menu {
      if hasAction("EDIT") {
        Button("Edit Inbox", systemImage: "pencil") { editPresented = true }
      }
      if hasAction("QUEUE") {
        Button("Queue", systemImage: "arrow.right.circle") { queuePresented = true }
      }
      if hasAction("CANCEL") {
        Button("Cancel task", systemImage: "xmark.circle", role: .destructive) { cancelPresented = true }
      }
      if hasAction("REOPEN") {
        Button("Reopen", systemImage: "arrow.uturn.backward") { reopenPresented = true }
      }
      if !hasAction("EDIT") && !hasAction("QUEUE") && !hasAction("CANCEL") && !hasAction("REOPEN") {
        Text("No actions available")
      }
    } label: {
      Image(systemName: "ellipsis.circle")
    }
    .disabled(!model.isConnected)
    .accessibilityLabel("Task actions")
  }

  private var hasTaskActions: Bool {
    hasAction("EDIT") || hasAction("QUEUE") || hasAction("CANCEL") || hasAction("REOPEN")
  }

  private func hasAction(_ action: String) -> Bool {
    detail.validActions.contains(action) || detail.validActions.contains(action.lowercased())
  }

  private var transcriptFollowKey: String {
    "\(detail.id):\(detail.messages.count):\(detail.runs.count):\(model.runItems.count):\(model.isLoadingDetail):\(detail.activeGate?.id ?? "none")"
  }

  private func openArtifact(_ artifact: TasksArtifactSnapshot) {
    guard let versionID = artifact.versionID.nilIfBlank else { return }
    selectedArtifact = ArtifactSelection(versionID: versionID, title: artifact.title, backTitle: "Back to task details")
  }

  private func runDate(_ run: TasksRunSnapshot) -> String {
    run.createdAt ?? run.startedAt ?? ""
  }

  private var latestRunActivity: String? {
    guard let runID = (detail.currentRun ?? detail.runs.max(by: { runDate($0) < runDate($1) }))?.id else { return nil }
    return model.runItems
      .filter { $0.runId == runID }
      .max(by: { $0.sequence < $1.sequence })?
      .content
  }

  private var showsContextDock: Bool {
    detail.completedResult == nil || selectedTab == .transcript
  }

  private var taskInterventions: [HumanIntervention] {
    model.pendingInterventions.filter { $0.taskID == detail.id }
  }
}

private struct TasksGatePanel: View {
  let gate: TasksGateSnapshot
  @Binding var response: String
  let isConnected: Bool
  let isSubmitting: Bool
  let errorMessage: String?
  let canAnswer: Bool
  let canRetry: Bool
  let answer: (String, ApprovalDecision?) async -> Bool
  let retry: (String?) async -> Bool

  var body: some View {
    ScrollView(.vertical) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        HStack(alignment: .firstTextBaseline) {
          Text(gate.prompt)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.content)
          Spacer(minLength: NoemaSpacing.sm)
        }

        if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
          Markdown(gate.context)
            .markdownTextStyle {
              FontFamily(.custom("Hanken Grotesk"))
              FontSize(14)
              ForegroundColor(NoemaColor.contentSecondary)
            }
        }

        if let errorMessage {
          Text(errorMessage)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.danger)
            .fixedSize(horizontal: false, vertical: true)
        }

        if !gate.suggestedAnswers.isEmpty && gate.kind.uppercased() == "CLARIFICATION" {
          VStack(alignment: .trailing, spacing: NoemaSpacing.xs) {
            ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
              Button {
                response = suggestion
                Task {
                  if await answer(suggestion, nil) { response = "" }
                }
              } label: {
                Text(suggestion)
                  .font(NoemaFont.body)
                  .foregroundStyle(NoemaColor.white)
                  .frame(minHeight: 32)
                  .padding(.horizontal, NoemaSpacing.md)
              }
              .buttonStyle(.plain)
              .background(NoemaColor.pine500, in: Capsule())
              .disabled(!isConnected || isSubmitting || !canAnswer)
            }
          }
          .frame(maxWidth: .infinity, alignment: .trailing)
        }

        if gate.kind.uppercased() == "RECOVERY" {
          if let reason = gate.recoveryReason, !reason.isEmpty {
            Label(reason.capitalized, systemImage: "exclamationmark.triangle")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.warning)
          }
          HStack(spacing: NoemaSpacing.xs) {
            TextField(recoveryPlaceholder, text: $response, axis: .vertical)
              .lineLimit(1...5)
              .textFieldStyle(.roundedBorder)
            Button("Respond", systemImage: "arrow.up") {
              Task {
                let succeeded: Bool
                if let answerText = response.nilIfBlank, canAnswer {
                  succeeded = await answer(answerText, nil)
                } else {
                  succeeded = await retry(response.nilIfBlank)
                }
                if succeeded { response = "" }
              }
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderedProminent)
            .accessibilityLabel(recoveryActionLabel)
          }
          .disabled(!isConnected || isSubmitting || (!canAnswer && !canRetry) || (canAnswer && !canRetry && response.nilIfBlank == nil))
        } else if gate.kind.uppercased() == "APPROVAL" {
          TextField("Explain your decision", text: $response, axis: .vertical)
            .lineLimit(2...5)
            .textFieldStyle(.roundedBorder)
          HStack(spacing: NoemaSpacing.sm) {
            Button("Decline", systemImage: "xmark") {
              guard let explanation = response.nilIfBlank else { return }
              Task { if await answer(explanation, .declined) { response = "" } }
            }
            .buttonStyle(.bordered)
            .tint(NoemaColor.danger)
            .disabled(!isConnected || isSubmitting || !canAnswer || response.nilIfBlank == nil)
            Button("Approve", systemImage: "checkmark") {
              guard let explanation = response.nilIfBlank else { return }
              Task { if await answer(explanation, .approved) { response = "" } }
            }
            .buttonStyle(.borderedProminent)
            .disabled(!isConnected || isSubmitting || !canAnswer || response.nilIfBlank == nil)
          }
        } else {
          HStack(spacing: NoemaSpacing.xs) {
            TextField(
              "",
              text: $response,
              prompt: Text("Or type another answer")
                .foregroundStyle(NoemaColor.white.opacity(0.72))
            )
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.white)
              .tint(NoemaColor.white)
              .textFieldStyle(.plain)
              .padding(.leading, NoemaSpacing.md)
            Button {
              guard let answerText = response.nilIfBlank else { return }
              Task { if await answer(answerText, nil) { response = "" } }
            } label: {
              Image(systemName: "paperplane.fill")
                .font(.system(size: 14, weight: .semibold))
                .frame(width: 34, height: 34)
                .background(NoemaColor.white.opacity(0.24), in: Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Answer")
            .disabled(!isConnected || isSubmitting || !canAnswer || response.nilIfBlank == nil)
          }
          .frame(width: 238, height: 42)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.pine500, in: Capsule())
          .frame(maxWidth: .infinity, alignment: .trailing)
          .padding(.trailing, NoemaSpacing.xs)
        }
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.top, NoemaSpacing.md)
      .padding(.bottom, 36)
    }
    .frame(maxHeight: 236)
    .background(
      NoemaColor.surface,
      in: UnevenRoundedRectangle(
        topLeadingRadius: NoemaSpacing.xxl,
        bottomLeadingRadius: 0,
        bottomTrailingRadius: 0,
        topTrailingRadius: NoemaSpacing.xxl
      )
    )
    .overlay {
      UnevenRoundedRectangle(
        topLeadingRadius: NoemaSpacing.xxl,
        bottomLeadingRadius: 0,
        bottomTrailingRadius: 0,
        topTrailingRadius: NoemaSpacing.xxl
      )
      .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
    .shadow(color: NoemaColor.ink900.opacity(0.13), radius: 14, y: -NoemaSpacing.xs)
    .accessibilityElement(children: .contain)
  }

  private var recoveryPlaceholder: String {
    if canAnswer && canRetry { return "Answer, or leave blank to retry" }
    if canRetry { return "Optional retry guidance" }
    return "Type your answer"
  }

  private var recoveryActionLabel: String {
    response.nilIfBlank != nil && canAnswer ? "Answer" : "Retry"
  }

  private var gateTitle: String {
    switch gate.kind.uppercased() {
    case "APPROVAL": "Approval needed"
    case "RECOVERY": "Recovery needed"
    default: "Clarification needed"
    }
  }
}

private struct TasksInboxEditSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  @State private var title: String
  @State private var description: String
  @State private var projectId: String?
  @FocusState private var focusedField: Field?
  @State private var isSaving = false
  @State private var discardPresented = false
  @State private var errorMessage: String?

  private enum Field: Hashable {
    case title
    case description
  }

  init(model: TasksModel, task: TasksDetailSnapshot) {
    self.model = model
    self.task = task
    _title = State(initialValue: task.title)
    _description = State(initialValue: task.description)
    _projectId = State(initialValue: task.project?.id)
  }

  var body: some View {
    NoemaNativeSheet(
      title: "Edit Inbox task",
      dismissDisabled: isSaving,
      onDismiss: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: 0) {
        Text("Changes the Inbox copy before the task is queued.")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.md)
          .padding(.bottom, 19)

        ScrollView {
          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            TasksSheetField("Title") {
              TextField("", text: $title)
                .textInputAutocapitalization(.sentences)
                .focused($focusedField, equals: .title)
                .noemaTaskSheetField(focused: focusedField == .title, height: 42)
            }
            TasksSheetField("Description (optional)") {
              TextField("", text: $description, axis: .vertical)
                .lineLimit(3...6)
                .focused($focusedField, equals: .description)
                .noemaTaskSheetField(focused: focusedField == .description, height: 76)
            }
            TasksSheetField("Project (optional)") {
              Picker(selection: $projectId) {
                Text("No project").tag(Optional<String>.none)
                ForEach(model.projects.filter { $0.archivedAt == nil || task.project?.id == $0.id }) { project in
                  Text(project.name).tag(Optional(project.id))
                }
              } label: {
                Text(projectId.flatMap { id in model.projects.first(where: { $0.id == id })?.name } ?? "No project")
                  .font(NoemaFont.body)
                  .foregroundStyle(NoemaColor.content)
                  .lineLimit(1)
              }
              .pickerStyle(.menu)
              .tint(NoemaColor.content)
              .frame(maxWidth: .infinity, minHeight: 42, maxHeight: 42, alignment: .leading)
              .padding(.horizontal, NoemaSpacing.md)
              .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
              .overlay {
                RoundedRectangle(cornerRadius: NoemaRadius.element)
                  .stroke(NoemaColor.separator, lineWidth: 1)
              }
            }

            if let errorMessage {
              Text(errorMessage)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.danger)
                .fixedSize(horizontal: false, vertical: true)
            }
          }
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
        }

        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { requestDismissal() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
            .disabled(isSaving)
          Button {
            Task {
              isSaving = true
              errorMessage = nil
              let succeeded = await model.updateInbox(task: task, title: title.trimmingCharacters(in: .whitespacesAndNewlines), description: description, projectId: projectId)
              isSaving = false
              if succeeded {
                dismiss()
              } else {
                errorMessage = model.lastError ?? "Noema could not save this task."
              }
            }
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if isSaving { ProgressView().tint(NoemaColor.white).controlSize(.small) }
              Text("Save")
            }
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.white)
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .background(NoemaColor.pine500, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .opacity(canSave ? 1 : 0.42)
          .disabled(!canSave)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.md)
        .padding(.bottom, NoemaSpacing.sm)
      }
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.height(424)], regularHeight: 560)
    .interactiveDismissDisabled(isDirty || isSaving)
    .sheet(isPresented: $discardPresented) {
      TasksDiscardSheet(title: "Discard changes?", message: "Your task edits will be lost.") {
        dismiss()
      }
    }
    .task {
      focusedField = .title
    }
  }

  private var isDirty: Bool {
    title != task.title || description != task.description || projectId != task.project?.id
  }

  private var canSave: Bool {
    !isSaving && isDirty && !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.isConnected
  }

  private func requestDismissal() {
    if isDirty {
      discardPresented = true
    } else {
      dismiss()
    }
  }
}

private struct TasksReopenSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  @State private var feedback = ""
  @State private var request = ""
  @FocusState private var focusedField: Field?
  @State private var isSubmitting = false
  @State private var discardPresented = false
  @State private var errorMessage: String?

  private enum Field: Hashable {
    case feedback
    case request
  }

  var body: some View {
    NoemaNativeSheet(
      title: "Reopen this task?",
      dismissDisabled: isSubmitting,
      onDismiss: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: 0) {
        Text("Add what should change. Historic runs and evidence stay intact; the new cycle starts in Queue.")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.md)
          .padding(.bottom, 19)

        ScrollView {
          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            TasksSheetField("Additional direction") {
              TextField("", text: $feedback, axis: .vertical)
                .lineLimit(3...6)
                .focused($focusedField, equals: .feedback)
                .noemaTaskSheetField(focused: focusedField == .feedback, height: 76)
            }
            TasksSheetField("Optional new request") {
              TextField("", text: $request, axis: .vertical)
                .lineLimit(3...5)
                .focused($focusedField, equals: .request)
                .noemaTaskSheetField(focused: focusedField == .request, height: 62)
            }

            if let errorMessage {
              Text(errorMessage)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.danger)
                .fixedSize(horizontal: false, vertical: true)
            }
          }
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
        }

        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { requestDismissal() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
            .disabled(isSubmitting)
          Button {
            Task {
              isSubmitting = true
              errorMessage = nil
              let succeeded = await model.reopen(task: task, feedback: feedback.trimmingCharacters(in: .whitespacesAndNewlines), request: request.nilIfBlank)
              isSubmitting = false
              if succeeded {
                dismiss()
              } else {
                errorMessage = model.lastError ?? "Noema could not reopen this task."
              }
            }
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if isSubmitting { ProgressView().tint(NoemaColor.white).controlSize(.small) }
              Text("Reopen")
            }
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.white)
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .background(NoemaColor.pine500, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .opacity(canSubmit ? 1 : 0.42)
          .disabled(!canSubmit)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.md)
        .padding(.bottom, NoemaSpacing.sm)
      }
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.height(362)], regularHeight: 500)
    .interactiveDismissDisabled(isDirty || isSubmitting)
    .sheet(isPresented: $discardPresented) {
      TasksDiscardSheet(title: "Discard feedback?", message: "Your reopen direction will be lost.") {
        dismiss()
      }
    }
    .task {
      focusedField = .feedback
    }
  }

  private var isDirty: Bool {
    feedback.nilIfBlank != nil || request.nilIfBlank != nil
  }

  private var canSubmit: Bool {
    !isSubmitting && feedback.nilIfBlank != nil && model.isConnected
  }

  private func requestDismissal() {
    if isDirty {
      discardPresented = true
    } else {
      dismiss()
    }
  }
}

private extension TasksStageBehavior {
  var symbol: String {
    switch self {
    case .intake: "tray"
    case .dispatch: "arrow.right.circle"
    case .active: "bolt.circle"
    case .humanGate: "hand.raised"
    case .terminalSuccess: "checkmark.circle"
    case .terminalCancelled: "xmark.circle"
    case .unknown: "circle"
    }
  }

  var color: Color {
    switch self {
    case .terminalSuccess: NoemaColor.success
    case .terminalCancelled: NoemaColor.contentTertiary
    case .humanGate: NoemaColor.warning
    default: NoemaColor.accent
    }
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
