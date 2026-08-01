import Foundation
import NoemaAPI
import SwiftUI

struct TasksRootView: View {
  let appModel: NoemaAppModel
  @State private var tasksModel: TasksModel?

  init(model: NoemaAppModel) {
    appModel = model
  }

  var body: some View {
    Group {
      if let tasksModel {
        TasksSurface(model: tasksModel)
      } else {
        ContentUnavailableView {
          Label("Tasks unavailable", systemImage: "checklist")
        } description: {
          Text("Connect this device to load your Work queue.")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(NoemaColor.surface)
      }
    }
    .task(id: appModel.profile?.clientId) {
      guard let client = appModel.graphQLClient?.client else { return }
      if tasksModel == nil {
        tasksModel = TasksModel(client: client)
      }
      await tasksModel?.start()
    }
    .onChange(of: appModel.recoveryGeneration) { _, _ in
      Task { await tasksModel?.recoverConnection() }
    }
  }
}

private struct TasksSurface: View {
  @Bindable var model: TasksModel
  @Environment(NoemaShellCoordinator.self) private var shellCoordinator
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var selectedTaskId: String?
  @State private var taskPath: [String] = []
  @State private var capturePresented = false
  @State private var projectEditor: TasksProjectSnapshot?
  @State private var createProjectPresented = false

  var body: some View {
    GeometryReader { proxy in
      switch NoemaBreakpoint.resolve(width: proxy.size.width) {
      case .compact, .regular:
        compactSurface
      case .wide:
        wideSurface
      }
    }
    .tint(NoemaColor.accent)
    .sheet(isPresented: $capturePresented) {
      TasksCaptureSheet(model: model)
    }
    .sheet(isPresented: $createProjectPresented) {
      TasksProjectSheet(model: model, project: nil)
    }
    .sheet(item: $projectEditor) { project in
      TasksProjectSheet(model: model, project: project)
    }
    .overlay(alignment: .bottom) {
      if !model.isConnected {
        TasksConnectionBanner(error: model.lastError) {
          Task { await model.recoverConnection() }
        }
        .padding(.horizontal, NoemaSpacing.md)
        .padding(.bottom, NoemaSpacing.sm)
        .transition(.move(edge: .bottom).combined(with: .opacity))
      }
    }
    .animation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion), value: model.isConnected)
    .onAppear { registerSecondaryNavigation() }
    .onAppear { openRequestedTask() }
    .onChange(of: shellCoordinator.requestedTaskID) { _, _ in openRequestedTask() }
    .onChange(of: model.projects) { _, _ in registerSecondaryNavigation() }
    .onChange(of: model.selectedProjectId) { _, _ in
      selectedTaskId = nil
      registerSecondaryNavigation()
    }
    .onChange(of: model.workspace?.id) { _, _ in registerSecondaryNavigation() }
  }

  private var compactSurface: some View {
    NavigationStack(path: $taskPath) {
      TasksListDeck(
        model: model,
        selectedTaskId: $selectedTaskId,
        capturePresented: $capturePresented,
        createProjectPresented: $createProjectPresented,
        projectEditor: $projectEditor
      )
      .navigationDestination(for: String.self) { taskId in
        TasksDetailRoute(model: model, taskId: taskId)
      }
    }
  }

  private func openRequestedTask() {
    guard let taskID = shellCoordinator.requestedTaskID else { return }
    selectedTaskId = taskID
    taskPath = [taskID]
    shellCoordinator.requestedTaskID = nil
  }

  private var wideSurface: some View {
    HStack(spacing: 0) {
      TasksListDeck(
        model: model,
        selectedTaskId: $selectedTaskId,
        capturePresented: $capturePresented,
        createProjectPresented: $createProjectPresented,
        projectEditor: $projectEditor,
        wide: true
      )
      .frame(width: 320)

      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(width: 1)

      Group {
        if let selectedTaskId {
          NavigationStack {
            TasksDetailRoute(model: model, taskId: selectedTaskId)
          }
        } else {
          ContentUnavailableView {
            Label("Select a task", systemImage: "checklist")
          } description: {
            Text("Needs You and recent Work stay visible in the task list.")
          }
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
    .background(NoemaColor.surface)
  }

  private func registerSecondaryNavigation() {
    let activeProjects = model.projects.filter { $0.archivedAt == nil }
    let archivedProjects = model.projects.filter { $0.archivedAt != nil }
    var entries: [NoemaSidebarEntry] = [
      .item(
        id: "work.workspace.personal",
        label: model.workspace?.name ?? "Personal",
        symbol: "briefcase",
        selected: model.selectedProjectId == nil,
        action: {
          Task { await model.selectProject(nil) }
        }
      )
    ]

    if !activeProjects.isEmpty {
      entries.append(.group("Projects"))
      entries.append(contentsOf: activeProjects.map { project in
        .item(
          id: "work.project.\(project.id)",
          label: project.name,
          symbol: "folder",
          depth: 1,
          selected: model.selectedProjectId == project.id,
          action: {
            Task { await model.selectProject(project.id) }
          }
        )
      })
    }

    if !archivedProjects.isEmpty {
      entries.append(.group("Archived"))
      entries.append(contentsOf: archivedProjects.map { project in
        .item(
          id: "work.project.archived.\(project.id)",
          label: project.name,
          symbol: "archivebox",
          depth: 1,
          selected: model.selectedProjectId == project.id,
          action: {
            Task { await model.selectProject(project.id) }
          }
        )
      })
    }

    shellCoordinator.show(NoemaSecondaryNavigation(
      title: model.workspace?.name ?? "Personal",
      symbol: "briefcase",
      entries: entries
    ))
  }
}

private struct TasksConnectionBanner: View {
  let error: String?
  let reconnect: () -> Void

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Image(systemName: "wifi.slash")
        .foregroundStyle(NoemaColor.warning)
      VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text("Showing cached tasks")
          .font(NoemaFont.captionEmphasized)
        if let error, !error.isEmpty {
          Text(error)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(1)
        }
      }
      Spacer(minLength: NoemaSpacing.sm)
      Button("Reconnect", action: reconnect)
        .font(NoemaFont.captionEmphasized)
        .buttonStyle(.glass)
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.sm)
    .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
    .overlay {
      RoundedRectangle(cornerRadius: NoemaRadius.element)
        .stroke(NoemaColor.separator.opacity(0.4), lineWidth: 0.5)
    }
    .accessibilityElement(children: .combine)
  }
}

struct TasksListDeck: View {
  @Bindable var model: TasksModel
  @Binding var selectedTaskId: String?
  @Binding var capturePresented: Bool
  @Binding var createProjectPresented: Bool
  @Binding var projectEditor: TasksProjectSnapshot?
  var wide = false

  private var needsYouTaskIds: Set<String> { Set(model.needsYou.map(\.task.id)) }
  private var runningTasks: [TasksTaskRow] {
    stageRows(for: [.active, .humanGate])
  }
  private var upNextTasks: [TasksTaskRow] {
    stageRows(for: [.dispatch])
  }
  private var inboxTasks: [TasksTaskRow] {
    stageRows(for: [.intake, .unknown])
  }
  private var hasVisibleTasks: Bool {
    !model.needsYou.isEmpty || !runningTasks.isEmpty || !upNextTasks.isEmpty || !inboxTasks.isEmpty || !model.history.isEmpty
  }

  var body: some View {
    VStack(spacing: 0) {
      TasksWorkToolbar(
        model: model,
        capturePresented: $capturePresented,
        createProjectPresented: $createProjectPresented,
        projectEditor: $projectEditor,
        wide: wide
      )

      ScrollView {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          if model.isRefreshing && !hasVisibleTasks {
            TasksStateCard(message: "Loading tasks…", symbol: "arrow.triangle.2.circlepath")
          } else if !hasVisibleTasks && !model.isRefreshing {
            TasksStateCard(
              message: "No tasks yet",
              detail: "Capture a request to start a durable Work item.",
              symbol: "checklist",
              actionTitle: "Capture task",
              action: { capturePresented = true }
            )
          } else {
            if !model.needsYou.isEmpty {
              TasksSectionHeader(title: "Needs You", count: model.needsYou.count, attention: true)
              LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                ForEach(model.needsYou) { item in
                  TasksAttentionCard(model: model, item: item, wide: wide) {
                    selectedTaskId = item.task.id
                  }
                }
              }
            }

            taskGroup("Running", tasks: runningTasks)
            taskGroup("Up next", tasks: upNextTasks)
            taskGroup("Inbox", tasks: inboxTasks)

            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              TasksSectionHeader(title: "History", count: model.history.count)
              if model.history.isEmpty {
                Text("Done and cancelled tasks remain available here.")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentTertiary)
                  .padding(.horizontal, NoemaSpacing.sm)
              } else {
                LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                  ForEach(model.history) { task in
                    taskLink(task)
                  }
                }
              }
            }
          }
        }
        .frame(maxWidth: 820, alignment: .leading)
        .padding(.horizontal, NoemaSpacing.md)
        .padding(.top, NoemaSpacing.sm)
        .padding(.bottom, NoemaSpacing.xxl + 48)
        .frame(maxWidth: .infinity, alignment: .center)
      }
      .scrollIndicators(.hidden)
      .refreshable { await model.refresh() }
    }
    .background(NoemaColor.surface)
    .overlay(alignment: .bottomTrailing) {
      Button {
        capturePresented = true
      } label: {
        Image(systemName: "plus")
          .font(.system(size: 20, weight: .semibold))
          .foregroundStyle(NoemaColor.white)
          .frame(width: 48, height: 48)
          .background(NoemaColor.clay600, in: Circle())
          .shadow(color: NoemaColor.ink900.opacity(0.16), radius: 8, y: 4)
      }
      .buttonStyle(.plain)
      .disabled(!model.isConnected)
      .opacity(model.isConnected ? 1 : 0.55)
      .padding(.trailing, NoemaSpacing.lg)
      .padding(.bottom, NoemaSpacing.lg)
      .accessibilityLabel("Capture task")
    }
  }

  @ViewBuilder
  private func taskGroup(_ title: String, tasks: [TasksTaskRow]) -> some View {
    if !tasks.isEmpty {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        TasksSectionHeader(title: title, count: tasks.count)
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          ForEach(tasks) { task in
            taskLink(task)
          }
        }
      }
    }
  }

  @ViewBuilder
  private func taskLink(_ task: TasksTaskRow) -> some View {
    if wide {
      Button {
        selectedTaskId = task.id
      } label: {
        TasksTaskCard(task: task, selected: selectedTaskId == task.id)
      }
      .buttonStyle(.plain)
    } else {
      NavigationLink(value: task.id) {
        TasksTaskCard(task: task)
      }
      .buttonStyle(.plain)
    }
  }

  private func stageRows(for behaviors: Set<TasksStageBehavior>) -> [TasksTaskRow] {
    model.tasks.filter {
      behaviors.contains($0.stage.behavior) && !needsYouTaskIds.contains($0.id)
    }
  }
}

private struct TasksSectionHeader: View {
  let title: String
  let count: Int
  var attention = false

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
      Text(title)
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(attention ? NoemaColor.clay600 : NoemaColor.contentTertiary)
      Spacer(minLength: NoemaSpacing.sm)
      Text("\(count)")
        .font(NoemaFont.monoTiny)
        .foregroundStyle(NoemaColor.contentTertiary)
        .accessibilityLabel("\(count) items")
    }
    .padding(.horizontal, NoemaSpacing.xs)
  }
}

private struct TasksStateCard: View {
  let message: String
  var detail: String?
  let symbol: String
  var actionTitle: String?
  var action: (() -> Void)?

  var body: some View {
    NoemaCard {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: symbol)
          .foregroundStyle(NoemaColor.contentTertiary)
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(message)
            .font(NoemaFont.bodyEmphasized)
          if let detail {
            Text(detail)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        Spacer(minLength: NoemaSpacing.sm)
        if let actionTitle, let action {
          Button(actionTitle, action: action)
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        }
      }
    }
  }
}

private struct TasksTaskCard: View {
  let task: TasksTaskRow
  var preview: String?
  var statusOverride: String?
  var selected = false
  var attached = false

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        Text(task.title)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(2)
        Spacer(minLength: NoemaSpacing.sm)
        Text(TasksRelativeTime.label(task.completedAt ?? task.updatedAt))
          .font(NoemaFont.monoTiny)
          .foregroundStyle(NoemaColor.contentTertiary)
          .lineLimit(1)
      }

      if let preview = (preview ?? task.summary).nilIfBlank {
        Text(preview)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(2)
      }

      HStack(spacing: NoemaSpacing.xs) {
        NoemaStatusToken(text: statusOverride ?? statusLabel, tone: statusTone)
        if let project = task.projectName?.nilIfBlank {
          Text("·")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
          Text(project)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
            .lineLimit(1)
        }
        if let run = task.currentRun, let activity = run.activity.nilIfBlank {
          Text("·")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
          Text(activity)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
            .lineLimit(1)
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: attached ? NoemaRadius.inner : NoemaRadius.element))
    .overlay {
      RoundedRectangle(cornerRadius: attached ? NoemaRadius.inner : NoemaRadius.element)
        .stroke(selected ? NoemaColor.pine500.opacity(0.55) : NoemaColor.separatorSubtle, lineWidth: selected ? 1.5 : 1)
    }
    .shadow(color: NoemaColor.ink900.opacity(selected ? 0.09 : 0.04), radius: selected ? 6 : 2, y: selected ? 2 : 1)
    .contentShape(Rectangle())
    .accessibilityElement(children: .combine)
    .accessibilityLabel("\(task.title), \(statusOverride ?? statusLabel)")
  }

  private var statusLabel: String {
    if let activity = task.currentRun?.activity.nilIfBlank {
      return activity
    }
    return task.stage.name
  }

  private var statusTone: NoemaStatusToken.Tone {
    if statusOverride != nil { return .warning }
    if task.activeGate != nil { return .warning }
    switch task.stage.behavior {
    case .terminalSuccess: return .success
    case .terminalCancelled: return .error
    default: return .neutral
    }
  }
}

private struct TasksAttentionCard: View {
  @Bindable var model: TasksModel
  let item: TasksAttentionRow
  let wide: Bool
  let selectTask: () -> Void
  @State private var response = ""

  private var gate: TasksGateSnapshot? { item.gate ?? item.task.activeGate }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      decision
      TasksAttentionTaskLink(task: item.task, title: item.title, wide: wide, selectTask: selectTask)
    }
    .clipShape(RoundedRectangle(cornerRadius: NoemaRadius.element))
    .overlay {
      RoundedRectangle(cornerRadius: NoemaRadius.element)
        .stroke(NoemaColor.clay600.opacity(0.38), lineWidth: 1)
    }
  }

  @ViewBuilder
  private var decision: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        Image(systemName: "hand.raised.fill")
          .foregroundStyle(NoemaColor.clay600)
        Text(item.title.nilIfBlank ?? "Needs your attention")
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
        Spacer(minLength: NoemaSpacing.sm)
        Text(item.kind.capitalized)
          .font(NoemaFont.metadata)
          .foregroundStyle(NoemaColor.clay600)
      }
      if let summary = item.summary.nilIfBlank {
        Text(summary)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(3)
      }

      if let currentGate = gate, let prompt = currentGate.prompt.nilIfBlank, prompt != item.summary {
        Text(prompt)
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(4)
      }

      if let gate {
        switch gate.kind.uppercased() {
        case "APPROVAL": approvalActions(gate: gate)
        case "RECOVERY": recoveryActions(gate: gate)
        default: clarificationActions(gate: gate)
        }
      } else if item.review != nil {
        Button("Open review", systemImage: "arrow.up.right") {
          selectTask()
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
      }
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surface)
  }

  @ViewBuilder
  private func approvalActions(gate: TasksGateSnapshot) -> some View {
    HStack(spacing: NoemaSpacing.sm) {
      Button("Decline", systemImage: "xmark") {
        submit("Declined", approval: .declined)
      }
      .buttonStyle(.bordered)
      .tint(NoemaColor.danger)
      .disabled(!model.isConnected || !canAnswer)
      Button("Approve", systemImage: "checkmark") {
        submit("Approved", approval: .approved)
      }
      .buttonStyle(.borderedProminent)
      .disabled(!model.isConnected || !canAnswer)
    }
    .controlSize(.small)
  }

  @ViewBuilder
  private func recoveryActions(gate: TasksGateSnapshot) -> some View {
    if let reason = gate.recoveryReason?.nilIfBlank {
      Text(reason.capitalized)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.warning)
        .lineLimit(2)
    }
    HStack(spacing: NoemaSpacing.sm) {
      TextField("Optional retry note", text: $response)
        .textFieldStyle(.roundedBorder)
      Button("Retry", systemImage: "arrow.clockwise") {
        Task { await model.retry(task: detailSnapshot, note: response.nilIfBlank) }
        response = ""
      }
      .buttonStyle(.borderedProminent)
      .disabled(!model.isConnected || !hasAction("RETRY"))
    }
    .controlSize(.small)
  }

  @ViewBuilder
  private func clarificationActions(gate: TasksGateSnapshot) -> some View {
    HStack(spacing: NoemaSpacing.sm) {
      TextField("Your answer", text: $response)
        .textFieldStyle(.roundedBorder)
      Button("Answer", systemImage: "arrow.up.circle") {
        submit(response.trimmingCharacters(in: .whitespacesAndNewlines))
      }
      .buttonStyle(.borderedProminent)
      .disabled(!model.isConnected || !canAnswer || response.nilIfBlank == nil)
    }
    .controlSize(.small)
  }

  private var detailSnapshot: TasksDetailSnapshot {
    item.task.detailSnapshot(gate: gate)
  }

  private var canAnswer: Bool { hasAction("ANSWER") }

  private func hasAction(_ action: String) -> Bool {
    item.validActions.contains(action) || item.validActions.contains(action.lowercased()) || detailSnapshot.validActions.contains(action)
  }

  private func submit(_ answer: String, approval: ApprovalDecision? = nil) {
    guard answer.nilIfBlank != nil else { return }
    Task {
      await model.answer(task: detailSnapshot, answer: answer, approval: approval)
    }
    response = ""
  }
}

private struct TasksAttentionTaskLink: View {
  let task: TasksTaskRow
  let title: String
  let wide: Bool
  let selectTask: () -> Void

  var body: some View {
    Group {
      if wide {
        Button(action: selectTask) {
          TasksTaskCard(task: task, preview: title, statusOverride: "Needs you", attached: true)
        }
        .buttonStyle(.plain)
      } else {
        NavigationLink(value: task.id) {
          TasksTaskCard(task: task, preview: title, statusOverride: "Needs you", attached: true)
        }
        .buttonStyle(.plain)
      }
    }
    .accessibilityLabel("Open \(task.title)")
  }
}

private enum TasksRelativeTime {
  static func label(_ raw: String) -> String {
    let fractionalFormatter = ISO8601DateFormatter()
    fractionalFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let plainFormatter = ISO8601DateFormatter()
    plainFormatter.formatOptions = [.withInternetDateTime]
    guard let date = fractionalFormatter.date(from: raw) ?? plainFormatter.date(from: raw) else {
      return raw
    }
    let elapsed = max(0, Date().timeIntervalSince(date))
    switch elapsed {
    case ..<60: return "now"
    case ..<3_600: return "\(Int(elapsed / 60))m"
    case ..<86_400: return "\(Int(elapsed / 3_600))h"
    case ..<604_800: return "\(Int(elapsed / 86_400))d"
    default:
      let formatter = DateFormatter()
      formatter.dateFormat = "MMM d"
      return formatter.string(from: date)
    }
  }
}

private extension TasksTaskRow {
  func detailSnapshot(gate: TasksGateSnapshot?) -> TasksDetailSnapshot {
    TasksDetailSnapshot(
      id: id,
      title: title,
      description: summary,
      project: nil,
      stage: stage,
      revision: revision,
      generation: generation,
      updatedAt: updatedAt,
      completedAt: completedAt,
      currentContract: nil,
      currentRun: currentRun,
      activeGate: gate,
      latestSubmission: nil,
      completedResult: nil,
      latestReview: latestReview,
      messages: [],
      runs: [],
      validActions: validActions
    )
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
