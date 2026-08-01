import MarkdownUI
import NoemaAPI
import SwiftUI

struct TasksDetailRoute: View {
  @Bindable var model: TasksModel
  let taskId: String

  var body: some View {
    Group {
      if let detail = model.detail, detail.id == taskId {
        TasksDetailContent(model: model, detail: detail)
      } else if model.isRefreshing {
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
    .background(NoemaColor.surface)
    .task(id: taskId) {
      await model.loadDetail(taskId: taskId)
    }
    .onDisappear {
      guard model.detail?.id == taskId else { return }
      model.clearDetail()
    }
  }
}

private struct TasksDetailContent: View {
  @Bindable var model: TasksModel
  let detail: TasksDetailSnapshot
  @State private var editPresented = false
  @State private var reopenPresented = false
  @State private var cancelPresented = false
  @State private var selectedRun: TasksRunSnapshot?
  @State private var gateResponse = ""
  @State private var selectedTab: TaskResultTab = .result

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        header

        if let gate = detail.activeGate {
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
        }

        if !detail.description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
          TasksMarkdownSection(title: "Description", text: detail.description)
        }

        if let contract = detail.currentContract, !contract.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
          TasksMarkdownSection(title: "Current request", text: contract)
        }

        if let completed = detail.completedResult {
          Picker("Task view", selection: $selectedTab) {
            ForEach(TaskResultTab.allCases) { tab in
              Text(tab.title).tag(tab)
            }
          }
          .pickerStyle(.segmented)

          if selectedTab == .result {
            TasksSubmissionSection(submission: completed, title: "Accepted result")
          } else {
            TasksTranscriptSection(messages: detail.messages)
          }
        } else {
          if let submission = detail.latestSubmission {
            TasksSubmissionSection(submission: submission, title: "Latest submission")
          }
          TasksTranscriptSection(messages: detail.messages)
        }

        if let review = detail.latestReview {
          TasksReviewSection(review: review)
        }

        TasksRunsSection(runs: detail.runs, currentRun: detail.currentRun) { run in
          selectedRun = run
        }
      }
      .frame(maxWidth: 820, alignment: .leading)
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.vertical, NoemaSpacing.lg)
      .frame(maxWidth: .infinity, alignment: .center)
    }
    .scrollDismissesKeyboard(.interactively)
    .navigationTitle(detail.title)
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) {
        Menu {
          if hasAction("EDIT") {
            Button("Edit Inbox", systemImage: "pencil") { editPresented = true }
          }
          if hasAction("QUEUE") {
            Button("Queue", systemImage: "arrow.right.circle") {
              Task { await model.queue(task: detail) }
            }
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
    }
    .sheet(isPresented: $editPresented) {
      TasksInboxEditSheet(model: model, task: detail)
    }
    .sheet(isPresented: $reopenPresented) {
      TasksReopenSheet(model: model, task: detail)
    }
    .sheet(item: $selectedRun) { run in
      TasksRunDetailSheet(model: model, task: detail, run: run)
    }
    .confirmationDialog("Cancel this task?", isPresented: $cancelPresented, titleVisibility: .visible) {
      Button("Cancel task", role: .destructive) {
        Task { await model.cancel(task: detail) }
      }
    } message: {
      Text("The current run will stop and the task will move to Cancelled.")
    }
    .onChange(of: detail.activeGate?.id) { _, _ in
      gateResponse = ""
    }
    .onChange(of: detail.id) { _, _ in
      gateResponse = ""
      selectedTab = .result
    }
  }

  private var header: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        Label(detail.stage.name, systemImage: detail.stage.behavior.symbol)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(detail.stage.behavior.color)
        Spacer(minLength: NoemaSpacing.sm)
        Text("Revision \(detail.revision) · Generation \(detail.generation)")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      Text(detail.title)
        .font(NoemaFont.pageTitle)
        .foregroundStyle(NoemaColor.content)
        .textSelection(.enabled)
      HStack(spacing: NoemaSpacing.sm) {
        if let project = detail.project {
          Label(project.name, systemImage: "folder")
        } else {
          Label("Personal", systemImage: "person")
        }
        if let completedAt = detail.completedAt {
          Label(completedAt, systemImage: "checkmark.circle")
        }
      }
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
      if let run = detail.currentRun {
        TasksCurrentRunNotice(run: run)
      }
    }
  }

  private func hasAction(_ action: String) -> Bool {
    detail.validActions.contains(action) || detail.validActions.contains(action.lowercased())
  }
}

private enum TaskResultTab: String, CaseIterable, Identifiable {
  case result
  case transcript

  var id: String { rawValue }
  var title: String { rawValue.capitalized }
}

private struct TasksCurrentRunNotice: View {
  let run: TasksRunSnapshot

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Image(systemName: run.status.uppercased().contains("FAIL") ? "exclamationmark.triangle" : "play.circle.fill")
        .foregroundStyle(run.status.uppercased().contains("FAIL") ? NoemaColor.warning : NoemaColor.accent)
      VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text(run.activity.isEmpty ? run.kind.capitalized : run.activity)
          .font(NoemaFont.bodyEmphasized)
        Text("Run \(run.attempt + 1) · \(run.status.capitalized)")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      Spacer(minLength: NoemaSpacing.sm)
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))
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
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      HStack(alignment: .firstTextBaseline) {
        Label(gateTitle, systemImage: "hand.raised.fill")
          .font(NoemaFont.title)
          .foregroundStyle(NoemaColor.warning)
        Spacer(minLength: NoemaSpacing.sm)
        Text(gate.state.capitalized)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.contentSecondary)
      }

      Text(gate.prompt)
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.content)
        .textSelection(.enabled)

      if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        Markdown(gate.context)
          .markdownTextStyle { ForegroundColor(NoemaColor.contentSecondary) }
      }

      if !gate.suggestedAnswers.isEmpty && gate.kind.uppercased() == "CLARIFICATION" {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Suggested answers")
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.contentSecondary)
          ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
            Button {
              response = suggestion
              answer(suggestion, nil)
            } label: {
              Text(suggestion)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.bordered)
            .disabled(!isConnected || !canAnswer)
          }
        }
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
        TextField("Your answer", text: $response, axis: .vertical)
          .lineLimit(3...8)
          .textFieldStyle(.roundedBorder)
        Button("Answer", systemImage: "arrow.up.circle") {
          answer(response.trimmingCharacters(in: .whitespacesAndNewlines), nil)
        }
        .buttonStyle(.borderedProminent)
        .disabled(!isConnected || !canAnswer || response.nilIfBlank == nil)
      }
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.md))
    .overlay {
      RoundedRectangle(cornerRadius: NoemaSpacing.md)
        .stroke(NoemaColor.warning.opacity(0.5), lineWidth: 1)
    }
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

private struct TasksMarkdownSection: View {
  let title: String
  let text: String

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(title)
        .font(NoemaFont.title)
      Markdown(text)
        .markdownTextStyle { ForegroundColor(NoemaColor.content) }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
  }
}

private struct TasksSubmissionSection: View {
  let submission: TasksSubmissionSnapshot
  let title: String
  @State private var criteriaExpanded = false

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline) {
        Text(title)
          .font(NoemaFont.title)
        Spacer(minLength: NoemaSpacing.sm)
        Text(submission.createdAt)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      if !submission.summary.isEmpty {
        Text(submission.summary)
          .font(NoemaFont.bodyEmphasized)
      }
      Markdown(submission.result)
        .markdownTextStyle { ForegroundColor(NoemaColor.content) }
      if !submission.criteria.isEmpty {
        DisclosureGroup("Criteria evidence", isExpanded: $criteriaExpanded) {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            ForEach(submission.criteria) { criterion in
              VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                Text(criterion.description.isEmpty ? "Criterion \(criterion.ordinal + 1)" : criterion.description)
                  .font(NoemaFont.captionEmphasized)
                if let evidence = criterion.evidence, !evidence.isEmpty {
                  Text(evidence)
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
              }
            }
          }
          .padding(.top, NoemaSpacing.xs)
        }
        .font(NoemaFont.captionEmphasized)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}

private struct TasksTranscriptSection: View {
  let messages: [TasksMessageSnapshot]
  @State private var expanded = false

  var body: some View {
    DisclosureGroup("Transcript", isExpanded: $expanded) {
      if messages.isEmpty {
        Text("No transcript entries yet.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
          .padding(.top, NoemaSpacing.xs)
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          ForEach(messages) { message in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack {
                Text(message.author)
                  .font(NoemaFont.captionEmphasized)
                Spacer(minLength: NoemaSpacing.sm)
                Text(message.createdAt)
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentTertiary)
              }
              Markdown(message.body)
                .markdownTextStyle { ForegroundColor(NoemaColor.content) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
          }
        }
        .padding(.top, NoemaSpacing.xs)
      }
    }
    .font(NoemaFont.title)
  }
}

private struct TasksReviewSection: View {
  let review: TasksReviewSnapshot

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline) {
        Label("Review", systemImage: "checkmark.seal")
          .font(NoemaFont.title)
        Spacer(minLength: NoemaSpacing.sm)
        Text(review.verdict.capitalized)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(review.verdict.uppercased().contains("PASS") ? NoemaColor.success : NoemaColor.warning)
      }
      Markdown(review.feedback)
        .markdownTextStyle { ForegroundColor(NoemaColor.content) }
      Text(review.createdAt)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentTertiary)
    }
  }
}

private struct TasksRunsSection: View {
  let runs: [TasksRunSnapshot]
  let currentRun: TasksRunSnapshot?
  let selectRun: (TasksRunSnapshot) -> Void
  @State private var expanded = false

  var body: some View {
    DisclosureGroup(isExpanded: $expanded) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        if runs.isEmpty {
          Text("No runs recorded yet.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          ForEach(runs) { run in
            Button { selectRun(run) } label: {
              HStack(alignment: .top, spacing: NoemaSpacing.sm) {
                Image(systemName: run.status.uppercased().contains("FAIL") ? "xmark.circle" : "play.circle")
                  .foregroundStyle(run.status.uppercased().contains("FAIL") ? NoemaColor.danger : NoemaColor.accent)
                VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                  Text(run.activity.isEmpty ? run.kind.capitalized : run.activity)
                    .font(NoemaFont.captionEmphasized)
                  Text("Attempt \(run.attempt + 1) · \(run.status.capitalized)")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                  if let error = run.error, !error.isEmpty {
                    Text(error)
                      .font(NoemaFont.caption)
                      .foregroundStyle(NoemaColor.warning)
                  }
                }
              }
            }
            .buttonStyle(.plain)
          }
        }
      }
      .padding(.top, NoemaSpacing.xs)
    } label: {
      HStack {
        Label("Runs", systemImage: "clock.arrow.circlepath")
          .font(NoemaFont.title)
        Spacer(minLength: NoemaSpacing.sm)
        if let currentRun {
          Text(currentRun.status.capitalized)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          Text("\(runs.count)")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
      }
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
