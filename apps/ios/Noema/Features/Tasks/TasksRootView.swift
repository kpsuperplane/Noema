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
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var selectedTaskId: String?
  @State private var selectedProjectId: String?
  @State private var capturePresented = false
  @State private var projectEditor: TasksProjectSnapshot?
  @State private var createProjectPresented = false

  var body: some View {
    GeometryReader { proxy in
      switch NoemaBreakpoint.resolve(width: proxy.size.width) {
      case .compact:
        compactSurface
      case .regular:
        regularSurface
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
  }

  private var compactSurface: some View {
    NavigationStack {
      TasksListDeck(model: model, selectedTaskId: $selectedTaskId, capturePresented: $capturePresented)
        .navigationDestination(for: String.self) { taskId in
          TasksDetailRoute(model: model, taskId: taskId)
        }
        .navigationTitle("Tasks")
        .navigationBarTitleDisplayMode(.inline)
    }
  }

  private var regularSurface: some View {
    NavigationSplitView {
      TasksSidebar(model: model, selectedProjectId: $selectedProjectId, createProjectPresented: $createProjectPresented, projectEditor: $projectEditor)
    } detail: {
      NavigationStack {
        TasksListDeck(model: model, selectedTaskId: $selectedTaskId, capturePresented: $capturePresented)
          .navigationDestination(for: String.self) { taskId in
            TasksDetailRoute(model: model, taskId: taskId)
          }
          .navigationTitle("Tasks")
          .navigationBarTitleDisplayMode(.inline)
      }
    }
    .navigationSplitViewStyle(.balanced)
  }

  private var wideSurface: some View {
    NavigationSplitView {
      TasksSidebar(model: model, selectedProjectId: $selectedProjectId, createProjectPresented: $createProjectPresented, projectEditor: $projectEditor)
    } content: {
      TasksListDeck(model: model, selectedTaskId: $selectedTaskId, capturePresented: $capturePresented, wide: true)
    } detail: {
      if let selectedTaskId {
        TasksDetailRoute(model: model, taskId: selectedTaskId)
      } else {
        ContentUnavailableView {
          Label("Select a task", systemImage: "checklist")
        } description: {
          Text("Needs You and recent Work stay visible in the task list.")
        }
      }
    }
    .navigationSplitViewStyle(.balanced)
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
        .disabled(false)
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.sm)
    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: NoemaSpacing.md))
    .overlay {
      RoundedRectangle(cornerRadius: NoemaSpacing.md)
        .stroke(NoemaColor.separator.opacity(0.4), lineWidth: 0.5)
    }
    .accessibilityElement(children: .combine)
  }
}

struct TasksListDeck: View {
  @Bindable var model: TasksModel
  @Binding var selectedTaskId: String?
  @Binding var capturePresented: Bool
  var wide = false

  var body: some View {
    let needsYouTaskIds = Set(model.needsYou.map(\.task.id))

    List(selection: wide ? $selectedTaskId : nil) {
      if !model.needsYou.isEmpty {
        Section {
          ForEach(model.needsYou) { item in
            taskLink(item.task, attention: item.title)
          }
        } header: {
          Label("Needs You", systemImage: "hand.raised.fill")
            .foregroundStyle(NoemaColor.warning)
        }
      }

      ForEach(TasksStageBehavior.allCases, id: \.self) { behavior in
        let rows = model.tasks.filter {
          $0.stage.behavior == behavior && !needsYouTaskIds.contains($0.id)
        }
        if !rows.isEmpty {
          Section(behavior.title) {
            ForEach(rows) { task in
              taskLink(task)
            }
          }
        }
      }

      if model.tasks.isEmpty && model.needsYou.isEmpty {
        ContentUnavailableView {
          Label("No tasks yet", systemImage: "checklist")
        } description: {
          Text("Capture a request to start a durable Work item.")
        } actions: {
          Button("Capture task", systemImage: "plus") { capturePresented = true }
            .buttonStyle(.borderedProminent)
        }
        .listRowBackground(NoemaColor.surface)
      }
    }
    .listStyle(.plain)
    .scrollContentBackground(.hidden)
    .background(NoemaColor.surface)
    .refreshable { await model.refresh() }
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) {
        Button("Capture", systemImage: "plus") { capturePresented = true }
          .labelStyle(.iconOnly)
          .buttonStyle(.glassProminent)
          .disabled(!model.isConnected)
          .accessibilityLabel("Capture task")
      }
    }
  }

  @ViewBuilder
  private func taskLink(_ task: TasksTaskRow, attention: String? = nil) -> some View {
    if wide {
      Button {
        selectedTaskId = task.id
      } label: {
        TasksTaskRowView(task: task, attention: attention)
      }
      .buttonStyle(.plain)
      .tag(task.id)
    } else {
      NavigationLink(value: task.id) {
        TasksTaskRowView(task: task, attention: attention)
      }
    }
  }
}

struct TasksTaskRowView: View {
  let task: TasksTaskRow
  let attention: String?

  var body: some View {
    HStack(alignment: .top, spacing: NoemaSpacing.sm) {
      Image(systemName: task.activeGate != nil || attention != nil ? "exclamationmark.circle.fill" : symbol)
        .foregroundStyle(task.activeGate != nil || attention != nil ? NoemaColor.warning : color)
        .frame(width: 22)
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text(task.title)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(2)
        Text(attention ?? task.summary)
          .font(NoemaFont.caption)
          .foregroundStyle(attention == nil ? NoemaColor.contentSecondary : NoemaColor.warning)
          .lineLimit(2)
        HStack(spacing: NoemaSpacing.sm) {
          Text(task.stage.name)
          if let project = task.projectName {
            Label(project, systemImage: "folder")
          }
          if let run = task.currentRun {
            Label(run.activity, systemImage: "play.circle")
          }
        }
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentTertiary)
        .lineLimit(1)
      }
      Spacer(minLength: NoemaSpacing.xs)
    }
    .padding(.vertical, NoemaSpacing.xs)
    .contentShape(Rectangle())
    .accessibilityElement(children: .combine)
    .accessibilityLabel("\(task.title), \(task.stage.name)")
  }

  private var symbol: String {
    switch task.stage.behavior {
    case .intake: "tray"
    case .dispatch: "arrow.right.circle"
    case .active: "bolt.circle"
    case .humanGate: "hand.raised"
    case .terminalSuccess: "checkmark.circle"
    case .terminalCancelled: "xmark.circle"
    case .unknown: "circle"
    }
  }

  private var color: Color {
    switch task.stage.behavior {
    case .terminalSuccess: NoemaColor.success
    case .terminalCancelled: NoemaColor.contentTertiary
    case .humanGate: NoemaColor.warning
    default: NoemaColor.accent
    }
  }
}
