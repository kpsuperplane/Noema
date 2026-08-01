import MarkdownUI
import NoemaAPI
import SwiftUI

struct TasksDetailRoute: View {
  @Bindable var model: TasksModel
  let taskId: String
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    VStack(spacing: 0) {
      if horizontalSizeClass == .compact, model.detail?.id != taskId {
        HStack(spacing: NoemaSpacing.md) {
          Text(taskTitle).font(NoemaFont.mobileTitle).lineLimit(1)
          Spacer(minLength: NoemaSpacing.sm)
          Button("Close", systemImage: "xmark") { dismiss() }
            .labelStyle(.iconOnly)
            .buttonStyle(.glass)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.vertical, NoemaSpacing.sm)
      }
      Group {
        if let detail = model.detail, detail.id == taskId {
          TasksDetailContent(model: model, detail: detail)
        } else if model.isLoadingDetail {
          ProgressView("Loading task…")
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
          ContentUnavailableView {
            Label("Task unavailable", systemImage: "checklist")
          } description: {
            Text(model.lastError ?? "This task is no longer available in the current workspace.")
          } actions: {
            Button("Try again", systemImage: "arrow.clockwise") {
              Task { await model.loadDetail(taskId: taskId) }
            }
            .buttonStyle(.borderedProminent)
            .disabled(!model.isConnected)
          }
          .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
      }
    }
    .background(NoemaColor.surface)
    .task(id: taskId) {
      await model.loadDetail(taskId: taskId)
    }
    .onDisappear {
      model.clearDetail(taskId: taskId)
    }
  }

  private var taskTitle: String {
    (model.tasks + model.history).first(where: { $0.id == taskId })?.title ?? "Task"
  }
}

private struct TasksDetailContent: View {
  @Bindable var model: TasksModel
  let detail: TasksDetailSnapshot
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @Environment(\.dismiss) private var dismiss
  @State private var editPresented = false
  @State private var reopenPresented = false
  @State private var queuePresented = false
  @State private var cancelPresented = false
  @State private var selectedRun: TasksRunSnapshot?
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
      if horizontalSizeClass == .compact { compactHeader }
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
              onRun: { selectedRun = $0 },
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
            onRun: { selectedRun = $0 },
            loadMore: { Task { await model.loadOlderRunItems() } },
            onArtifact: openArtifact
          )
        }
          Color.clear
            .frame(height: 1)
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
      if horizontalSizeClass != .compact {
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
    .sheet(item: $selectedRun) { run in
      TasksRunDetailSheet(model: model, task: detail, run: run)
    }
    .sheet(item: $selectedArtifact) { selection in
      ArtifactVersionSheet(model: ArtifactModel(client: model.client, profile: model.profile), selection: selection)
    }
    .sheet(isPresented: $cancelPresented) {
      TasksDiscardSheet(
        title: "Cancel this task?",
        message: "Active work is fenced immediately. Historic evidence remains available.",
        confirmTitle: "Cancel task"
      ) {
        Task { await model.cancel(task: detail) }
      }
    }
    .sheet(isPresented: $taskInfoPresented) {
      TasksTaskInfoSheet(detail: detail)
    }
    .sheet(isPresented: $validationPresented) {
      TasksValidationSheet(criteria: detail.latestSubmission?.criteria ?? [])
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      if let gate = detail.activeGate {
        VStack(spacing: 0) {
          TasksGatePanel(
            gate: gate,
            response: $gateResponse,
            isConnected: model.isConnected,
            canAnswer: hasAction("ANSWER"),
            canRetry: hasAction("RETRY"),
            answer: { answer, approval in
              Task { await model.answer(task: detail, answer: answer, approval: approval) }
            },
            retry: { note in
              Task { await model.retry(task: detail, note: note) }
            }
          )
          .padding(.horizontal, NoemaSpacing.lg)
          TasksTaskContextDock(
            run: detail.currentRun ?? detail.runs.max(by: { runDate($0) < runDate($1) }),
            criteria: detail.latestSubmission?.criteria ?? [],
            canCancel: hasAction("CANCEL") && model.isConnected,
            cancel: { cancelPresented = true },
            showInfo: { taskInfoPresented = true },
            showValidation: { validationPresented = true }
          )
          .padding(.horizontal, NoemaSpacing.lg)
          .offset(y: -NoemaSpacing.xxl)
          .padding(.bottom, -NoemaSpacing.xxl)
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

  private var compactHeader: some View {
    VStack(spacing: NoemaSpacing.xs) {
      Capsule()
        .fill(NoemaColor.ink900.opacity(0.24))
        .frame(width: 32, height: 4)
      HStack(spacing: NoemaSpacing.md) {
        Text(detail.title)
          .font(NoemaFont.mobileTitle)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
        Spacer(minLength: NoemaSpacing.sm)
        Button("Close", systemImage: "xmark") { dismiss() }
          .labelStyle(.iconOnly)
          .buttonStyle(.glass)
          .accessibilityLabel("Close task detail")
      }
    }
    .padding(.horizontal, NoemaSpacing.lg)
    .padding(.top, NoemaSpacing.xs)
    .padding(.bottom, NoemaSpacing.sm)
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
    .buttonStyle(.glass)
    .disabled(!model.isConnected)
    .accessibilityLabel("Task actions")
  }

  private func hasAction(_ action: String) -> Bool {
    detail.validActions.contains(action) || detail.validActions.contains(action.lowercased())
  }

  private var transcriptFollowKey: String {
    "\(detail.id):\(detail.messages.count):\(detail.runs.count):\(model.runItems.count):\(model.isLoadingDetail):\(detail.activeGate?.id ?? "none")"
  }

  private func openArtifact(_ artifact: TasksArtifactSnapshot) {
    guard let versionID = artifact.versionID.nilIfBlank else { return }
    selectedArtifact = ArtifactSelection(versionID: versionID, title: artifact.title)
  }

  private func runDate(_ run: TasksRunSnapshot) -> String {
    run.createdAt ?? run.startedAt ?? ""
  }
}

private struct TasksGatePanel: View {
  let gate: TasksGateSnapshot
  @Binding var response: String
  let isConnected: Bool
  let canAnswer: Bool
  let canRetry: Bool
  let answer: (String, ApprovalDecision?) -> Void
  let retry: (String?) -> Void

  var body: some View {
    ScrollView(.vertical) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        HStack(alignment: .firstTextBaseline) {
          Text(gate.prompt)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.content)
          Spacer(minLength: NoemaSpacing.sm)
          Text(gate.state.capitalized)
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.contentSecondary)
        }

        if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
          Markdown(gate.context)
            .markdownTextStyle {
              FontFamily(.custom("Hanken Grotesk"))
              FontSize(14)
              ForegroundColor(NoemaColor.contentSecondary)
            }
        }

        if !gate.suggestedAnswers.isEmpty && gate.kind.uppercased() == "CLARIFICATION" {
          VStack(alignment: .trailing, spacing: NoemaSpacing.xs) {
            ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
              Button {
                response = suggestion
                answer(suggestion, nil)
              } label: {
                Text(suggestion)
                  .font(NoemaFont.body)
                  .foregroundStyle(NoemaColor.white)
                  .frame(minHeight: 32)
                  .padding(.horizontal, NoemaSpacing.md)
              }
              .buttonStyle(.plain)
              .background(NoemaColor.pine500, in: Capsule())
              .disabled(!isConnected || !canAnswer)
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
          TextField("Optional retry note", text: $response, axis: .vertical)
            .lineLimit(2...5)
            .textFieldStyle(.roundedBorder)
          Button("Retry", systemImage: "arrow.clockwise") {
            retry(response.nilIfBlank)
          }
          .buttonStyle(.borderedProminent)
          .disabled(!isConnected || !canRetry)
        } else if gate.kind.uppercased() == "APPROVAL" {
          TextField("Optional note", text: $response, axis: .vertical)
            .lineLimit(2...5)
            .textFieldStyle(.roundedBorder)
          HStack(spacing: NoemaSpacing.sm) {
            Button("Decline", systemImage: "xmark") {
              answer(response.nilIfBlank ?? "Declined", .declined)
            }
            .buttonStyle(.bordered)
            .tint(NoemaColor.danger)
            .disabled(!isConnected || !canAnswer)
            Button("Approve", systemImage: "checkmark") {
              answer(response.nilIfBlank ?? "Approved", .approved)
            }
            .buttonStyle(.borderedProminent)
            .disabled(!isConnected || !canAnswer)
          }
        } else {
          HStack(spacing: NoemaSpacing.xs) {
            TextField("Or type another answer", text: $response)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.white)
              .textFieldStyle(.plain)
              .padding(.leading, NoemaSpacing.md)
            Button {
              answer(response.trimmingCharacters(in: .whitespacesAndNewlines), nil)
            } label: {
              Image(systemName: "paperplane.fill")
                .font(.system(size: 14, weight: .semibold))
                .frame(width: 34, height: 34)
                .background(NoemaColor.white.opacity(0.24), in: Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Answer")
            .disabled(!isConnected || !canAnswer || response.nilIfBlank == nil)
          }
          .frame(height: 42)
          .padding(.trailing, NoemaSpacing.xs)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.pine500, in: Capsule())
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
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(
        title: "Edit Inbox task",
        subtitle: "Changes the Inbox copy before the task is queued.",
        onClose: requestDismissal,
        isDisabled: isSaving
      )

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
            await model.updateInbox(task: task, title: title.trimmingCharacters(in: .whitespacesAndNewlines), description: description, projectId: projectId)
            isSaving = false
            dismiss()
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
    .presentationDetents([.height(424)])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
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

  private enum Field: Hashable {
    case feedback
    case request
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(
        title: "Reopen this task?",
        subtitle: "Add what should change. Historic runs and evidence stay intact; the new cycle starts in Queue.",
        onClose: requestDismissal,
        isDisabled: isSubmitting
      )

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
            await model.reopen(task: task, feedback: feedback.trimmingCharacters(in: .whitespacesAndNewlines), request: request.nilIfBlank)
            isSubmitting = false
            dismiss()
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
    .presentationDetents([.height(362)])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
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

fileprivate extension View {
  func noemaTaskSheetField(focused: Bool, height: CGFloat) -> some View {
    font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .textFieldStyle(.plain)
      .padding(.horizontal, NoemaSpacing.md)
      .frame(height: height, alignment: .topLeading)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.element)
          .stroke(focused ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focused ? 2 : 1)
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
