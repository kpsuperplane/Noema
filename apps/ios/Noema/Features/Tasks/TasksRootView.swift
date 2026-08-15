import Foundation
import NoemaAPI
import SwiftUI

struct TasksRootView: View {
  let appModel: NoemaAppModel
  @State private var tasksModel: TasksModel?

  init(model: NoemaAppModel, tasksModel: TasksModel?) {
    appModel = model
    _tasksModel = State(initialValue: tasksModel)
  }

  var body: some View {
    Group {
      if let tasksModel {
        TasksSurface(model: tasksModel)
      } else {
        NoemaDeckState(title: "Tasks unavailable", message: "Connect this device to load your task queue.", symbol: "checklist", tone: .warning)
          .background(NoemaColor.surface)
      }
    }
    .task(id: appModel.profile?.clientId) {
      guard let client = appModel.graphQLClient?.client else { return }
      if tasksModel == nil {
        tasksModel = TasksModel(client: client, profile: appModel.profile)
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
  @State private var capturePresented = false
  @State private var projectEditor: TasksProjectSnapshot?
  @State private var createProjectPresented = false

  var body: some View {
    GeometryReader { proxy in
      if proxy.size.width < NoemaBreakpoint.regularMinimum - 216 - 8 {
        compactSurface
      } else {
        wideSurface
      }
    }
    .tint(NoemaColor.accent)
    .noemaSheet(isPresented: $capturePresented) {
      TasksCaptureSheet(model: model)
    }
    .noemaSheet(isPresented: $createProjectPresented) {
      TasksProjectSheet(model: model, project: nil)
    }
    .noemaSheet(item: $projectEditor) { project in
      TasksProjectSheet(model: model, project: project)
    }
    .overlay(alignment: .bottom) {
      if !model.isConnected, model.hasLoadedTasks {
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
    TasksListDeck(
      model: model,
      selectedTaskId: $selectedTaskId,
      capturePresented: $capturePresented,
      createProjectPresented: $createProjectPresented,
      projectEditor: $projectEditor
    )
    .noemaSheet(isPresented: taskDetailPresented) {
      if let selectedTaskId {
        TasksDetailRoute(model: model, taskId: selectedTaskId, compactPresentation: true)
          .noemaMobileDrawerPresentation()
      }
    }
  }

  private var taskDetailPresented: Binding<Bool> {
    Binding(
      get: { selectedTaskId != nil },
      set: { if !$0 { selectedTaskId = nil } }
    )
  }

  private func openRequestedTask() {
    guard let taskID = shellCoordinator.requestedTaskID else { return }
    selectedTaskId = taskID
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
          NoemaDeckState(title: "Select a task", message: "Needs You and recent tasks stay visible in the task list.", symbol: "checklist")
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
        id: "tasks.workspace.personal",
        label: model.workspace?.name ?? "Personal",
        icon: .briefcaseBusiness,
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
          id: "tasks.project.\(project.id)",
          label: project.name,
          icon: .folder,
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
          id: "tasks.project.archived.\(project.id)",
          label: project.name,
          icon: .archive,
          depth: 1,
          selected: model.selectedProjectId == project.id,
          action: {
            Task { await model.selectProject(project.id) }
          }
        )
      })
    }

    entries.append(.group("Project actions"))
    entries.append(.item(
      id: "tasks.project.new",
      label: "New project",
      icon: .folderPlus,
      action: { createProjectPresented = true }
    ))
    if let selectedProject = model.projects.first(where: { $0.id == model.selectedProjectId }) {
      entries.append(.item(
        id: "tasks.project.edit",
        label: "Edit \(selectedProject.name)",
        icon: .pencil,
        action: { projectEditor = selectedProject }
      ))
      entries.append(.item(
        id: "tasks.project.archive",
        label: selectedProject.archivedAt == nil ? "Archive project" : "Reopen project",
        icon: selectedProject.archivedAt == nil ? .archive : .undo2,
        action: { projectEditor = selectedProject }
      ))
    }

    shellCoordinator.show(NoemaSecondaryNavigation(
      title: model.workspace?.name ?? "Personal",
      icon: .briefcaseBusiness,
      entries: entries
    ), for: .tasks)
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
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element)
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
  private var scheduledTasks: [TasksTaskRow] {
    model.tasks.filter { task in
      task.schedule != nil
    }
  }
  private var recurringTasks: [TasksTaskRow] {
    var seen = Set<String>()
    return scheduledTasks
      .filter { $0.schedule?.recurrenceId != nil }
      .sorted { $0.updatedAt > $1.updatedAt }
      .filter { task in
        guard let recurrenceId = task.schedule?.recurrenceId else { return false }
        return seen.insert(recurrenceId).inserted
      }
  }
  private var oneTimeScheduledTasks: [TasksTaskRow] {
    scheduledTasks.filter {
      $0.stage.behavior == .intake
        && $0.schedule?.recurrenceId == nil
        && !needsYouTaskIds.contains($0.id)
    }
  }
  private var inboxTasks: [TasksTaskRow] {
    stageRows(for: [.intake, .unknown]).filter { $0.schedule == nil }
  }
  private var hasVisibleTasks: Bool {
    !model.needsYou.isEmpty || !model.pendingInterventions.isEmpty || !model.tasks.isEmpty || !model.history.isEmpty
  }

  var body: some View {
    VStack(spacing: 0) {
      TasksToolbar(
        model: model,
        capturePresented: $capturePresented,
        createProjectPresented: $createProjectPresented,
        projectEditor: $projectEditor,
        wide: wide
      )

      ScrollView {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          if model.isRefreshing && !model.hasLoadedTasks {
            TasksStateCard(message: "Loading tasks…", symbol: "arrow.triangle.2.circlepath")
          } else if !model.hasLoadedTasks, let error = model.tasksErrorMessage {
            TasksStateCard(
              message: "Could not load tasks",
              detail: error,
              symbol: "exclamationmark.triangle",
              actionTitle: "Retry",
              action: { Task { await model.refresh() } }
            )
          } else if !hasVisibleTasks && model.hasLoadedTasks {
            TasksStateCard(
              message: "No tasks yet",
              detail: "Capture a request to start a durable task.",
              symbol: "checklist",
              actionTitle: "Capture task",
              action: { capturePresented = true }
            )
          } else {
            if let error = model.projectsErrorMessage {
              TasksStateCard(
                message: "Project information could not refresh",
                detail: error,
                symbol: "folder.badge.questionmark",
                actionTitle: "Retry",
                action: { Task { await model.refresh() } }
              )
            }
            if let error = model.interventionsErrorMessage {
              TasksStateCard(
                message: "Could not load interventions",
                detail: error,
                symbol: "hand.raised",
                actionTitle: "Retry",
                action: { Task { await model.refresh() } }
              )
            }
            if !model.needsYou.isEmpty || !visiblePendingInterventions.isEmpty {
              TasksSectionHeader(title: "Needs you", count: model.needsYou.count + visiblePendingInterventions.count, attention: true)
              if !model.needsYou.isEmpty {
                LazyVStack(alignment: .leading, spacing: NoemaSpacing.compact) {
                  ForEach(model.needsYou) { item in
                    TasksAttentionCard(
                      model: model,
                      item: item,
                      wide: wide,
                      isSelected: selectedTaskId == item.task.id
                    ) {
                      selectedTaskId = item.task.id
                    }
                  }
                }
              }
              if !visiblePendingInterventions.isEmpty {
                TasksHumanInterventionsView(model: model, interventions: visiblePendingInterventions)
              }
            }

            taskGroup("Running", tasks: runningTasks)
            scheduledGroup
            taskGroup("Up next", tasks: upNextTasks)
            taskGroup("Inbox", tasks: inboxTasks)
            if model.hasMoreTasks {
              TasksLoadMoreButton(loading: model.isLoadingMoreTasks) {
                Task { await model.loadMoreTasks() }
              }
            }

            VStack(alignment: .leading, spacing: NoemaSpacing.md) {
              TasksSectionHeader(title: "History", count: model.history.count)
              if let error = model.historyErrorMessage {
                TasksStateCard(
                  message: "Could not load history",
                  detail: error,
                  symbol: "clock.arrow.circlepath",
                  actionTitle: "Retry",
                  action: { Task { await model.loadHistory() } }
                )
              } else if model.history.isEmpty {
                VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                  Text("No matching history")
                    .font(NoemaFont.captionEmphasized)
                    .foregroundStyle(NoemaColor.contentTertiary)
                  Text("Done and cancelled tasks remain available here.")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentTertiary)
                }
                .padding(NoemaSpacing.md)
                .frame(maxWidth: .infinity, minHeight: 72, alignment: .leading)
                .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
                .overlay {
                  NoemaSuperellipse(cornerRadius: NoemaRadius.element)
                    .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
                }
              } else {
                LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                  ForEach(model.history) { task in
                    taskLink(task)
                  }
                }
              }
              if model.hasMoreHistory {
                TasksLoadMoreButton(loading: model.isLoadingMoreHistory) {
                  Task { await model.loadMoreHistory() }
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
      .tracksNoemaSurfaceTop(for: .tasks)
      .scrollIndicators(.hidden)
      .refreshable { await model.refresh() }
    }
    .background(NoemaColor.surface)
    .overlay(alignment: .bottomTrailing) {
      if !wide {
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
        .accessibilityLabel("Capture task")
      }
    }
  }

  private var visiblePendingInterventions: [HumanIntervention] {
    guard let selectedTaskId else { return model.pendingInterventions }
    return model.pendingInterventions.filter { $0.taskID != selectedTaskId }
  }

  @ViewBuilder
  private func taskGroup(_ title: String, tasks: [TasksTaskRow]) -> some View {
    if !tasks.isEmpty {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        TasksSectionHeader(title: title, count: tasks.count)
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.compact) {
          ForEach(tasks) { task in
            taskLink(task)
          }
        }
      }
    }
  }

  @ViewBuilder
  private var scheduledGroup: some View {
    if !oneTimeScheduledTasks.isEmpty || !recurringTasks.isEmpty {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        TasksSectionHeader(title: "Scheduled", count: oneTimeScheduledTasks.count + recurringTasks.count)
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.compact) {
          ForEach(oneTimeScheduledTasks) { task in
            taskLink(
              task,
              statusOverride: "Scheduled",
              timestamp: task.schedule?.scheduledFor
            )
          }
          ForEach(recurringTasks) { task in
            TasksRecurrenceCard(
              model: model,
              task: task,
              needsAttention: needsYouTaskIds.contains(task.id),
              selected: wide && selectedTaskId == task.id,
              select: { selectedTaskId = task.id }
            )
          }
        }
      }
    }
  }

  @ViewBuilder
  private func taskLink(
    _ task: TasksTaskRow,
    statusOverride: String? = nil,
    timestamp: String? = nil
  ) -> some View {
    if wide {
      Button {
        selectedTaskId = task.id
      } label: {
        TasksTaskCard(task: task, statusOverride: statusOverride, timestamp: timestamp, selected: selectedTaskId == task.id)
      }
      .buttonStyle(.plain)
    } else {
      Button {
        selectedTaskId = task.id
      } label: {
        TasksTaskCard(task: task, statusOverride: statusOverride, timestamp: timestamp)
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

private struct TasksRecurrenceCard: View {
  @Bindable var model: TasksModel
  let task: TasksTaskRow
  let needsAttention: Bool
  let selected: Bool
  let select: () -> Void
  @State private var recurrence: TasksRecurrenceSnapshot?

  var body: some View {
    if recurrence?.lifecycle != "ENDED" {
      Button(action: select) {
        TasksTaskCard(
          task: task,
          titleOverride: recurrence?.title,
          preview: preview,
          statusOverride: recurrence?.lifecycle == "PAUSED" ? "Paused" : needsAttention ? "Needs you" : "Recurring",
          timestamp: nextRun,
          selected: selected
        )
      }
      .buttonStyle(.plain)
      .task(id: task.schedule?.recurrenceId) {
        guard let recurrenceID = task.schedule?.recurrenceId else { return }
        recurrence = await model.loadRecurrence(recurrenceId: recurrenceID)
      }
    }
  }

  private var nextRun: String? {
    recurrence?.nextRunAt ?? task.schedule?.scheduledFor
  }

  private var preview: String? {
    guard let nextRun else { return nil }
    let repeatLabel = recurrence.map { TasksScheduleFormatting.recurrenceSummary($0.cronExpression) } ?? "Repeating"
    return "\(repeatLabel) · Next \(TasksScheduleFormatting.timestampLabel(nextRun))"
  }
}

private struct TasksSectionHeader: View {
  let title: String
  let count: Int
  var attention = false

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
      Text(title)
        .font(NoemaFont.taskMeta.weight(.semibold))
        .foregroundStyle(attention ? NoemaColor.content : NoemaColor.contentTertiary)
      Spacer(minLength: NoemaSpacing.sm)
      Text("\(count)")
        .font(NoemaFont.monoTiny)
        .foregroundStyle(NoemaColor.contentTertiary)
        .accessibilityLabel("\(count) items")
    }
    .padding(.horizontal, NoemaSpacing.xs)
  }
}

private struct TasksLoadMoreButton: View {
  let loading: Bool
  let action: () -> Void

  var body: some View {
    HStack {
      Spacer(minLength: 0)
      Button(action: action) {
        if loading {
          ProgressView().controlSize(.small)
        } else {
          Text("Load more")
        }
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
      .disabled(loading)
      Spacer(minLength: 0)
    }
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
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            .controlSize(.small)
        }
      }
    }
  }
}

private struct TasksTaskCard: View {
  let task: TasksTaskRow
  var titleOverride: String?
  var preview: String?
  var statusOverride: String?
  var timestamp: String?
  var selected = false
  var attached = false

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        Text(titleOverride?.nilIfBlank ?? task.title)
          .font(NoemaFont.taskTitle)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
        Spacer(minLength: NoemaSpacing.sm)
        Text(TasksRelativeTime.label(timestamp ?? task.completedAt ?? task.updatedAt))
          .font(NoemaFont.monoTiny)
          .foregroundStyle(NoemaColor.contentTertiary)
          .lineLimit(1)
      }

      if let preview = (preview ?? task.summary).nilIfBlank {
        Text(preview)
          .font(NoemaFont.taskPreview)
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(1)
      }

      HStack(spacing: NoemaSpacing.xs) {
        TasksStatusChip(text: statusOverride ?? statusLabel, tone: statusTone, symbol: statusSymbol)
        if let project = task.projectName?.nilIfBlank {
          Text("·")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
          Text(project)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentTertiary)
            .lineLimit(1)
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.sm)
    .frame(minHeight: 60)
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element)
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

  private var statusSymbol: String {
    if statusOverride != nil || task.activeGate != nil { return "person" }
    switch task.stage.behavior {
    case .active: return task.currentRun?.kind == "REVIEWER" ? "magnifyingglass" : "arrow.trianglehead.2.clockwise.rotate.90"
    case .terminalSuccess: return "checkmark.circle"
    case .terminalCancelled: return "xmark"
    default: return "clock"
    }
  }
}

private struct TasksStatusChip: View {
  let text: String
  let tone: NoemaStatusToken.Tone
  let symbol: String

  var body: some View {
    Label(text, systemImage: symbol)
      .font(NoemaFont.taskPreview)
      .foregroundStyle(color)
      .padding(.horizontal, NoemaSpacing.compact)
      .padding(.vertical, NoemaSpacing.xxs)
      .background(NoemaColor.surface, in: NoemaSuperellipse.full)
      .overlay { NoemaSuperellipse.full.stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
  }

  private var color: Color {
    switch tone {
    case .neutral: NoemaColor.contentSecondary
    case .success: NoemaColor.pine700
    case .warning: NoemaColor.clay600
    case .error: NoemaColor.red700
    }
  }
}

private struct TasksAttentionCard: View {
  @Bindable var model: TasksModel
  let item: TasksAttentionRow
  let wide: Bool
  let isSelected: Bool
  let selectTask: () -> Void
  @State private var response = ""

  private var gate: TasksGateSnapshot? { item.gate ?? item.task.activeGate }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      if !isSelected { decision }
      TasksAttentionTaskLink(task: item.task, title: item.title, wide: wide, selectTask: selectTask)
    }
    .clipShape(NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
  }

  @ViewBuilder
  private var decision: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      if let prompt = (gate?.prompt.nilIfBlank ?? item.summary.nilIfBlank) {
        Text(prompt)
          .font(NoemaFont.taskTitle)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(3)
      }

      if let context = gate?.context.nilIfBlank {
        Text(context)
          .font(NoemaFont.caption)
          .lineSpacing(2)
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(4)
      }

      if let error = model.commandError(taskID: item.task.id) {
        Text(error)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.danger)
          .fixedSize(horizontal: false, vertical: true)
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
        .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
        .controlSize(.small)
      }
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.top, NoemaSpacing.md)
    .padding(.bottom, 14)
    .background(NoemaColor.surface)
  }

  @ViewBuilder
  private func approvalActions(gate: TasksGateSnapshot) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      TextField("Optional note", text: $response, axis: .vertical)
        .lineLimit(2...4)
        .noemaTextField()
      HStack(spacing: NoemaSpacing.sm) {
        Button("Decline", systemImage: "xmark") {
          submit(response.nilIfBlank ?? "Declined", approval: .declined)
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .danger))
        .disabled(!model.isConnected || isSubmitting || !canAnswer)
        Button("Approve", systemImage: "checkmark") {
          submit(response.nilIfBlank ?? "Approved", approval: .approved)
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        .disabled(!model.isConnected || isSubmitting || !canAnswer)
      }
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
      TextField(recoveryPlaceholder, text: $response)
        .noemaTextField()
      Button("Respond", systemImage: "arrow.up") {
        if let answer = response.nilIfBlank, canAnswer {
          submit(answer)
        } else {
          Task {
            if await model.retry(task: detailSnapshot, note: response.nilIfBlank) { response = "" }
          }
        }
      }
      .labelStyle(.iconOnly)
      .buttonStyle(NoemaActionButtonStyle(variant: .primary))
      .accessibilityLabel(recoveryActionLabel)
      .disabled(!model.isConnected || isSubmitting || (!canAnswer && !canRetry) || (canAnswer && !canRetry && response.nilIfBlank == nil))
    }
    .controlSize(.small)
  }

  @ViewBuilder
  private func clarificationActions(gate: TasksGateSnapshot) -> some View {
    VStack(alignment: .trailing, spacing: NoemaSpacing.xs) {
      ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
        Button(suggestion) { submit(suggestion) }
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.white)
          .padding(.horizontal, NoemaSpacing.md)
          .frame(minHeight: 32)
          .background(NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .buttonStyle(.plain)
          .disabled(!model.isConnected || isSubmitting || !canAnswer)
      }
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
        Button {
          submit(response.trimmingCharacters(in: .whitespacesAndNewlines))
        } label: {
          Image(systemName: "arrow.up")
            .frame(width: 32, height: 32)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.interactive(), in: Circle())
        .disabled(!model.isConnected || isSubmitting || !canAnswer || response.nilIfBlank == nil)
      }
      .padding(.leading, NoemaSpacing.md)
      .padding(.trailing, NoemaSpacing.xs)
      .padding(.vertical, NoemaSpacing.xs)
      .frame(width: 240, height: 40)
      .background(NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: 22))
    }
    .frame(maxWidth: .infinity, alignment: .trailing)
  }

  private var detailSnapshot: TasksDetailSnapshot {
    item.task.detailSnapshot(gate: gate)
  }

  private var canAnswer: Bool { hasAction("ANSWER") }
  private var canRetry: Bool { hasAction("RETRY") }
  private var isSubmitting: Bool { model.commandIsPending(taskID: item.task.id) }

  private var recoveryPlaceholder: String {
    if canAnswer && canRetry { return "Answer, or leave blank to retry" }
    if canRetry { return "Optional retry guidance" }
    return "Type your answer"
  }

  private var recoveryActionLabel: String {
    response.nilIfBlank != nil && canAnswer ? "Answer" : "Retry"
  }

  private func hasAction(_ action: String) -> Bool {
    item.validActions.contains(action) || item.validActions.contains(action.lowercased()) || detailSnapshot.validActions.contains(action)
  }

  private func submit(_ answer: String, approval: ApprovalDecision? = nil) {
    guard answer.nilIfBlank != nil else { return }
    Task {
      if await model.answer(task: detailSnapshot, answer: answer, approval: approval) {
        response = ""
      }
    }
  }
}

private struct TasksAttentionTaskLink: View {
  let task: TasksTaskRow
  let title: String
  let wide: Bool
  let selectTask: () -> Void

  var body: some View {
    Button(action: selectTask) {
      TasksTaskCard(task: task, statusOverride: "Needs you", attached: true)
    }
    .buttonStyle(.plain)
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

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
