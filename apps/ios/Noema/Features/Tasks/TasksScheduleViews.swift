import Foundation
import NoemaAPI
import SwiftUI

enum TasksScheduleAction: String, Identifiable {
  case schedule
  case reschedule
  case unschedule

  var id: String { rawValue }
  var title: String {
    switch self {
    case .schedule: "Schedule task"
    case .reschedule: "Reschedule task"
    case .unschedule: "Unschedule task"
    }
  }
}

enum TasksRepeatChoice: String, CaseIterable, Identifiable {
  case never
  case daily
  case weekdays
  case selected
  case weekly
  case monthly
  case custom

  var id: String { rawValue }
  var title: String {
    switch self {
    case .never: "Never"
    case .daily: "Daily"
    case .weekdays: "Weekdays"
    case .selected: "Selected weekdays"
    case .weekly: "Weekly"
    case .monthly: "Monthly"
    case .custom: "Custom cron"
    }
  }
}

struct TasksScheduleDraft: Hashable, Sendable {
  var localStart: Date
  var timeZone: String
  var repeatChoice: TasksRepeatChoice
  var selectedWeekdays: Set<Int>
  var cronExpression: String
  var missedRunPolicy: String
  var overlapPolicy: String
  var preview: [String] = []

  static func initial(schedule: TasksScheduleSnapshot? = nil, recurrence: TasksRecurrenceSnapshot? = nil) -> TasksScheduleDraft {
    let timeZone = recurrence?.timeZone ?? schedule?.timeZone ?? TimeZone.current.identifier
    let instant = recurrence.flatMap { TasksScheduleDraft.date(from: $0.startsAt) }
      ?? schedule.flatMap { TasksScheduleDraft.date(from: $0.scheduledFor) }
      ?? Date(timeIntervalSinceNow: 3_600)
    let repeatChoice: TasksRepeatChoice = schedule?.isRecurring == true ? .custom : .never
    let local = TasksScheduleDraft.localDate(from: instant, in: timeZone)
    let calendar = Calendar(identifier: .gregorian)
    return TasksScheduleDraft(
      localStart: local,
      timeZone: timeZone,
      repeatChoice: recurrence == nil ? repeatChoice : .custom,
      selectedWeekdays: [calendar.component(.weekday, from: local) - 1],
      cronExpression: recurrence?.cronExpression ?? "",
      missedRunPolicy: recurrence?.missedRunPolicy ?? schedule?.missedRunPolicy ?? "RUN_ONCE",
      overlapPolicy: recurrence?.overlapPolicy ?? "SKIP"
    )
  }

  var startsAt: Date? { Self.utcDate(from: localStart, in: timeZone) }

  var cron: String? {
    return switch repeatChoice {
    case .never: nil
    case .custom: cronExpression.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
    default: Self.cron(for: repeatChoice, localStart: localStart, weekdays: selectedWeekdays)
    }
  }

  var validationMessage: String? {
    guard startsAt != nil else { return "Check the start time, timezone, and repeat settings." }
    if repeatChoice != .never && cron == nil { return "Check the start time, timezone, and repeat settings." }
    return nil
  }

  func input() -> NewTaskScheduleInput? {
    guard let startsAt else { return nil }
    let cron = cron
    if repeatChoice != .never && cron == nil { return nil }
    if cron != nil && preview.isEmpty { return nil }
    let recurrence: GraphQLNullable<NewTaskRecurrenceInput>
    let isRecurring: Bool
    if let cron {
      isRecurring = true
      recurrence = .some(NewTaskRecurrenceInput(
        startsAt: startsAt.iso8601String,
        cronExpression: cron,
        overlapPolicy: .some(GraphQLEnum(rawValue: overlapPolicy))
      ))
    } else {
      isRecurring = false
      recurrence = .none
    }
    return NewTaskScheduleInput(
      scheduledFor: (isRecurring ? preview.first.flatMap(Self.date(from:)) ?? startsAt : startsAt).iso8601String,
      timeZone: timeZone,
      missedRunPolicy: .some(GraphQLEnum(rawValue: missedRunPolicy)),
      recurrence: recurrence
    )
  }

  var signature: String {
    "\(localStart.timeIntervalSince1970)|\(timeZone)|\(repeatChoice.rawValue)|\(selectedWeekdays.sorted())|\(cronExpression)|\(missedRunPolicy)|\(overlapPolicy)"
  }

  private static func cron(for repeatChoice: TasksRepeatChoice, localStart: Date, weekdays: Set<Int>) -> String? {
    let calendar = Calendar(identifier: .gregorian)
    let hour = calendar.component(.hour, from: localStart)
    let minute = calendar.component(.minute, from: localStart)
    let day = calendar.component(.day, from: localStart)
    guard (0...23).contains(hour), (0...59).contains(minute) else { return nil }
    return switch repeatChoice {
    case .daily: "\(minute) \(hour) * * *"
    case .weekdays: "\(minute) \(hour) * * 1-5"
    case .selected:
      weekdays.isEmpty ? nil : "\(minute) \(hour) * * \(weekdays.sorted().map(String.init).joined(separator: ","))"
    case .weekly:
      "\(minute) \(hour) * * \(max(0, calendar.component(.weekday, from: localStart) - 1))"
    case .monthly: "\(minute) \(hour) \(day) * *"
    case .never, .custom: nil
    }
  }

  private static func localDate(from instant: Date, in identifier: String) -> Date {
    let source = Calendar(identifier: .gregorian)
    let components = source.dateComponents(in: TimeZone(identifier: identifier) ?? .current, from: instant)
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = .current
    return calendar.date(from: DateComponents(year: components.year, month: components.month, day: components.day, hour: components.hour, minute: components.minute)) ?? instant
  }

  private static func utcDate(from local: Date, in identifier: String) -> Date? {
    var localCalendar = Calendar(identifier: .gregorian)
    localCalendar.timeZone = .current
    let components = localCalendar.dateComponents([.year, .month, .day, .hour, .minute], from: local)
    guard let timeZone = TimeZone(identifier: identifier) else { return nil }
    var target = Calendar(identifier: .gregorian)
    target.timeZone = timeZone
    return target.date(from: components)
  }

  private static func date(from value: String) -> Date? {
    TasksISO8601.date(from: value)
  }
}

private extension Date {
  var iso8601String: String { TasksISO8601.string(from: self) }
}

private enum TasksISO8601 {
  static func date(from value: String) -> Date? {
    let fractional = ISO8601DateFormatter()
    fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return fractional.date(from: value) ?? ISO8601DateFormatter().date(from: value)
  }

  static func string(from date: Date) -> String {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter.string(from: date)
  }
}

enum TasksScheduleFormatting {
  static func timestampLabel(_ raw: String) -> String {
    guard let date = TasksISO8601.date(from: raw) else { return raw }
    return date.formatted(date: .abbreviated, time: .shortened)
  }

  static func recurrenceSummary(_ cron: String) -> String {
    let fields = cron.split(whereSeparator: \.isWhitespace).map(String.init)
    guard fields.count == 5,
          let minute = Int(fields[0]), (0...59).contains(minute),
          let hour = Int(fields[1]), (0...23).contains(hour),
          fields[3] == "*" else { return "Custom recurring schedule" }
    let formatter = DateFormatter()
    formatter.locale = .current
    formatter.timeZone = TimeZone(secondsFromGMT: 0)
    formatter.dateFormat = "h:mm a"
    let time = formatter.string(from: Date(timeIntervalSince1970: TimeInterval(hour * 3_600 + minute * 60)))
    let day = fields[2]
    let weekday = fields[4]
    if day != "*", weekday == "*", let dayNumber = Int(day), (1...31).contains(dayNumber) {
      return "Monthly on day \(dayNumber) at \(time)"
    }
    guard day == "*" else { return "Custom recurring schedule" }
    if weekday == "*" { return "Every day at \(time)" }
    if weekday == "1-5" { return "Weekdays at \(time)" }
    let names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    let values = weekday.split(separator: ",").compactMap { Int($0) }
    guard values.count == weekday.split(separator: ",").count,
          values.allSatisfy(names.indices.contains) else { return "Custom recurring schedule" }
    return "\(values.map { names[$0] }.joined(separator: ", ")) at \(time)"
  }
}

struct TasksScheduleFields: View {
  @Bindable var model: TasksModel
  @Binding var draft: TasksScheduleDraft
  let recurringOnly: Bool

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      TasksSheetField("Starts") {
        DatePicker("", selection: $draft.localStart, displayedComponents: [.date, .hourAndMinute])
          .labelsHidden()
          .datePickerStyle(.compact)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, NoemaSpacing.md)
          .frame(minHeight: 42)
          .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
      }
      TasksSheetField("Timezone") {
        TextField("America/Los_Angeles", text: $draft.timeZone)
          .textInputAutocapitalization(.never)
          .autocorrectionDisabled()
          .noemaTaskSheetField(focused: false, height: 42)
      }
      Text("IANA timezone used for wall-clock recurrence.")
        .font(NoemaFont.metadata)
        .foregroundStyle(NoemaColor.contentTertiary)
      TasksSheetField("Repeat") {
        Picker("Repeat", selection: $draft.repeatChoice) {
          ForEach(TasksRepeatChoice.allCases.filter { !recurringOnly || $0 != .never }) { choice in
            Text(choice.title).tag(choice)
          }
        }
        .pickerStyle(.menu)
        .tint(NoemaColor.content)
        .frame(maxWidth: .infinity, minHeight: 42, alignment: .leading)
        .padding(.horizontal, NoemaSpacing.md)
        .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
        .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
      }
      if draft.repeatChoice == .selected { weekdayPicker }
      if draft.repeatChoice == .custom {
        TasksSheetField("Cron expression") {
          TextField("minute hour day month weekday", text: $draft.cronExpression)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .noemaTaskSheetField(focused: false, height: 42)
        }
        Text("Five fields: minute hour day month weekday.")
          .font(NoemaFont.metadata)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      DisclosureGroup("Advanced") {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          TasksSheetField("If a run is missed") {
            Picker("Missed run policy", selection: $draft.missedRunPolicy) {
              Text("Run once").tag("RUN_ONCE")
              Text("Skip").tag("SKIP")
            }
            .pickerStyle(.segmented)
          }
          if draft.repeatChoice != .never {
            TasksSheetField("If another run is active") {
              Picker("Overlap policy", selection: $draft.overlapPolicy) {
                Text("Skip").tag("SKIP")
                Text("Queue one").tag("QUEUE_ONE")
                Text("Allow overlap").tag("ALLOW")
              }
              .pickerStyle(.menu)
              .tint(NoemaColor.content)
            }
          }
        }
        .padding(.top, NoemaSpacing.sm)
      }
      if let error = draft.validationMessage {
        NoemaInlineState(message: error, symbol: "exclamationmark.triangle", tone: .warning)
      } else if !draft.preview.isEmpty {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Next runs")
            .font(NoemaFont.captionEmphasized)
          ForEach(draft.preview, id: \.self) { value in
            Text(Self.dateLabel(value, timeZone: draft.timeZone))
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
      }
    }
    .task(id: draft.signature) {
      guard draft.validationMessage == nil, let startsAt = draft.startsAt else {
        draft.preview = []
        return
      }
      draft.preview = await model.schedulePreview(startsAt: startsAt.iso8601String, timeZone: draft.timeZone, cronExpression: draft.cron)
    }
  }

  private var weekdayPicker: some View {
    TasksSheetField("Days") {
      HStack(spacing: NoemaSpacing.xxs) {
        ForEach(0..<7, id: \.self) { day in
          let selected = draft.selectedWeekdays.contains(day)
          Button(Self.weekdayLabel(day)) {
            if selected { draft.selectedWeekdays.remove(day) } else { draft.selectedWeekdays.insert(day) }
          }
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(selected ? NoemaColor.white : NoemaColor.contentSecondary)
          .frame(maxWidth: .infinity, minHeight: 30)
          .background(selected ? NoemaColor.pine500 : NoemaColor.paper100, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .buttonStyle(.plain)
        }
      }
    }
  }

  private static func weekdayLabel(_ day: Int) -> String {
    ["S", "M", "T", "W", "T", "F", "S"][safe: day] ?? "?"
  }

  private static func dateLabel(_ raw: String, timeZone: String) -> String {
    guard let date = TasksISO8601.date(from: raw) else { return raw }
    let formatter = DateFormatter()
    formatter.locale = .current
    formatter.timeZone = TimeZone(identifier: timeZone) ?? .current
    formatter.dateFormat = "EEE, MMM d, h:mm a z"
    return formatter.string(from: date)
  }
}

struct TasksExecutorFields: View {
  @Bindable var model: TasksModel
  @Binding var executorAgentId: String
  @Binding var cwdOverride: String

  var body: some View {
    DisclosureGroup("Advanced") {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        TasksSheetField("Executor") {
          Picker("Executor", selection: $executorAgentId) {
            Text("Built-in executor").tag("agent:task-executor")
            ForEach(model.acpAgents.filter { $0.enabled || $0.id == executorAgentId }) { agent in
              Text("\(agent.displayName) (ACP)").tag(agent.id)
            }
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
        }
        TasksSheetField("Task directory base (optional)") {
          TextField("/absolute/path", text: $cwdOverride)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .noemaTaskSheetField(focused: false, height: 42)
        }
        Text(summary)
          .font(NoemaFont.metadata)
          .foregroundStyle(NoemaColor.contentTertiary)
          .fixedSize(horizontal: false, vertical: true)
      }
      .padding(.top, NoemaSpacing.sm)
    }
  }

  let effectiveCwd: String?

  init(model: TasksModel, executorAgentId: Binding<String>, cwdOverride: Binding<String>, effectiveCwd: String? = nil, projectFolder: String? = nil) {
    self.model = model
    _executorAgentId = executorAgentId
    _cwdOverride = cwdOverride
    self.effectiveCwd = effectiveCwd
    self.projectFolder = projectFolder
  }

  private let projectFolder: String?

  private var summary: String {
    if let override = cwdOverride.nilIfBlank { return "Task directory · under \(override)" }
    if let projectFolder { return "Task directory · under \(projectFolder)" }
    if let effectiveCwd { return "Task directory · \(effectiveCwd)" }
    return "Task directory · Noema Tasks folder (created when queued)"
  }
}

struct TasksScheduleSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  let action: TasksScheduleAction
  @State private var draft: TasksScheduleDraft
  @State private var isSubmitting = false
  @State private var errorMessage: String?

  init(model: TasksModel, task: TasksDetailSnapshot, action: TasksScheduleAction) {
    self.model = model
    self.task = task
    self.action = action
    _draft = State(initialValue: TasksScheduleDraft.initial(schedule: task.schedule))
  }

  var body: some View {
    NoemaNativeSheet(title: action.title, dismissDisabled: isSubmitting, onDismiss: { dismiss() }) {
      ScrollView {
        VStack(alignment: .leading, spacing: 0) {
          Text(action == .unschedule ? "Return this task to Inbox." : "The task will enter Queue when it is due.")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, NoemaSpacing.lg)
            .padding(.top, NoemaSpacing.md)
            .padding(.bottom, 19)
          if action == .unschedule {
            Text("This removes only future timing. The task content and history stay intact.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
              .padding(.horizontal, NoemaSpacing.lg)
          } else {
            TasksScheduleFields(model: model, draft: $draft, recurringOnly: false)
              .padding(.horizontal, NoemaSpacing.lg)
          }
          if let errorMessage {
            NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .warning)
              .padding(.horizontal, NoemaSpacing.lg)
              .padding(.top, NoemaSpacing.md)
          }
          actions
        }
      }
      .scrollBounceBehavior(.basedOnSize)
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation(action == .unschedule ? [.height(260)] : [.medium, .large], regularHeight: action == .unschedule ? 360 : 680)
  }

  private var actions: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Spacer(minLength: 0)
      Button("Cancel") { dismiss() }
        .buttonStyle(.plain)
        .font(NoemaFont.body)
        .disabled(isSubmitting)
      Button {
        Task {
          isSubmitting = true
          errorMessage = nil
          let succeeded: Bool
          if action == .unschedule {
            succeeded = await model.unschedule(task: task)
          } else if let input = draft.input() {
            succeeded = action == .schedule ? await model.schedule(task: task, input: input) : await model.reschedule(task: task, input: input)
          } else {
            succeeded = false
          }
          isSubmitting = false
          if succeeded { dismiss() } else { errorMessage = model.lastError ?? "Noema could not update this schedule." }
        }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          if isSubmitting { ProgressView().tint(NoemaColor.white).controlSize(.small) }
          Text(action == .unschedule ? "Unschedule" : "Save")
        }
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.white)
        .frame(minHeight: 32)
        .padding(.horizontal, NoemaSpacing.md)
      }
      .buttonStyle(.plain)
      .background(action == .unschedule ? NoemaColor.danger : NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .opacity(canSubmit ? 1 : 0.42)
      .disabled(!canSubmit)
    }
    .padding(.horizontal, NoemaSpacing.lg)
    .padding(.top, NoemaSpacing.lg)
    .padding(.bottom, NoemaSpacing.sm)
  }

  private var canSubmit: Bool {
    guard !isSubmitting, model.isConnected else { return false }
    return action == .unschedule || draft.input() != nil
  }
}

struct TasksRecurrenceSummaryView: View {
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  @Environment(NoemaShellCoordinator.self) private var coordinator
  @State private var recurrence: TasksRecurrenceSnapshot?
  @State private var recurrenceLoadFailed = false
  @State private var scheduleAction: TasksScheduleAction?
  @State private var editing = false
  @State private var confirmingEnd = false
  @State private var isSubmitting = false
  @State private var errorMessage: String?

  var body: some View {
    Group {
      if let schedule = task.schedule {
        if schedule.recurrenceId == nil {
          oneTimeBody(schedule: schedule)
        } else if let recurrence {
          recurrenceBody(schedule: schedule, recurrence: recurrence)
        } else {
          Text(recurrenceLoadFailed
            ? "Schedule details could not be loaded."
            : "Repeating from \(label(schedule.scheduledFor, timeZone: schedule.timeZone))")
            .font(NoemaFont.caption)
            .foregroundStyle(recurrenceLoadFailed ? NoemaColor.danger : NoemaColor.contentSecondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, NoemaSpacing.md)
            .padding(.vertical, NoemaSpacing.sm)
            .overlay(alignment: .top) { Rectangle().fill(NoemaColor.separatorSubtle).frame(height: 1) }
        }
      }
    }
    .task(id: task.schedule?.recurrenceId) {
      recurrenceLoadFailed = false
      guard let id = task.schedule?.recurrenceId else { recurrence = nil; return }
      recurrence = await model.loadRecurrence(recurrenceId: id)
      recurrenceLoadFailed = recurrence == nil
    }
    .noemaSheet(item: $scheduleAction) { action in
      TasksScheduleSheet(model: model, task: task, action: action)
    }
    .noemaSheet(isPresented: $editing) {
      if let recurrence { TasksRecurrenceEditSheet(model: model, recurrence: recurrence) { refresh() } }
    }
    .confirmationDialog("End recurring schedule?", isPresented: $confirmingEnd, titleVisibility: .visible) {
      Button("End schedule", role: .destructive) { run(.end) }
      Button("Cancel", role: .cancel) {}
    } message: {
      Text("No future tasks will be created. This task and earlier runs stay unchanged.")
    }
  }

  @ViewBuilder
  private func recurrenceBody(schedule: TasksScheduleSnapshot, recurrence: TasksRecurrenceSnapshot) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(TasksScheduleFormatting.recurrenceSummary(recurrence.cronExpression))
            .font(NoemaFont.captionEmphasized)
          Text(occurrenceLabel(schedule: schedule, recurrence: recurrence))
            .font(NoemaFont.metadata)
            .foregroundStyle(NoemaColor.contentSecondary)
          if recurrence.lifecycle == "ACTIVE", let next = recurrence.nextRunAt {
            Text("Following run · \(label(next, timeZone: recurrence.timeZone))")
              .font(NoemaFont.metadata)
              .foregroundStyle(NoemaColor.contentTertiary)
          }
        }
        Spacer(minLength: NoemaSpacing.sm)
        Menu {
          if task.validActions.contains("RUN_NOW"), recurrence.lifecycle != "ENDED" { Button("Run now", systemImage: "play") { run(.runNow) } }
          if recurrence.lifecycle != "ENDED" {
            Button("Edit schedule", systemImage: "pencil") { editing = true }
            Button(recurrence.lifecycle == "ACTIVE" ? "Pause future runs" : "Resume future runs", systemImage: recurrence.lifecycle == "ACTIVE" ? "pause" : "play") { run(recurrence.lifecycle == "ACTIVE" ? .pause : .resume) }
            Button("Skip next run", systemImage: "forward.end") { run(.skip) }
            Button("End recurring schedule", systemImage: "stop.circle", role: .destructive) { confirmingEnd = true }
          }
        } label: {
          Image(systemName: "ellipsis.circle")
            .frame(width: 30, height: 30)
        }
        .buttonStyle(.plain)
        .disabled(isSubmitting || !model.isConnected)
      }
      if !recurrence.occurrences.isEmpty {
        DisclosureGroup("Schedule history (\(recurrence.occurrences.count))") {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            ForEach(recurrence.occurrences) { occurrence in
              if let taskID = occurrence.taskId {
                Button { coordinator.openTask(taskID) } label: {
                  occurrenceRow(occurrence, recurrence: recurrence)
                }
                .buttonStyle(.plain)
                .accessibilityHint("Opens this scheduled task")
              } else {
                occurrenceRow(occurrence, recurrence: recurrence)
              }
            }
          }
          .padding(.top, NoemaSpacing.xs)
        }
      }
      if let errorMessage { Text(errorMessage).font(NoemaFont.metadata).foregroundStyle(NoemaColor.danger) }
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.sm)
    .overlay(alignment: .top) { Rectangle().fill(NoemaColor.separatorSubtle).frame(height: 1) }
  }

  private func oneTimeBody(schedule: TasksScheduleSnapshot) -> some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text("Scheduled")
          .font(NoemaFont.captionEmphasized)
        Text(label(schedule.scheduledFor, timeZone: schedule.timeZone))
          .font(NoemaFont.metadata)
          .foregroundStyle(NoemaColor.contentSecondary)
        Text(schedule.timeZone)
          .font(NoemaFont.metadata)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      Spacer(minLength: NoemaSpacing.sm)
      Menu {
        if task.validActions.contains("RUN_NOW") { Button("Run now", systemImage: "play") { runOneTimeNow() } }
        if task.validActions.contains("RESCHEDULE") { Button("Reschedule", systemImage: "calendar.badge.clock") { scheduleAction = .reschedule } }
        if task.validActions.contains("UNSCHEDULE") { Button("Unschedule", systemImage: "calendar.badge.minus", role: .destructive) { scheduleAction = .unschedule } }
      } label: {
        Image(systemName: "ellipsis.circle")
          .frame(width: 30, height: 30)
      }
      .buttonStyle(.plain)
      .disabled(isSubmitting || !model.isConnected)
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.sm)
    .overlay(alignment: .top) { Rectangle().fill(NoemaColor.separatorSubtle).frame(height: 1) }
  }

  private func runOneTimeNow() {
    Task {
      isSubmitting = true
      errorMessage = nil
      if !(await model.runScheduledNow(task: task)) {
        errorMessage = model.lastError ?? "Noema could not start this task."
      }
      isSubmitting = false
    }
  }

  private func run(_ action: TasksRecurrenceAction) {
    guard let recurrence else { return }
    Task {
      isSubmitting = true
      errorMessage = nil
      if await model.changeRecurrence(recurrence, action: action) {
        self.recurrence = await model.refreshRecurrence(recurrence)
      } else {
        errorMessage = model.lastError ?? "Noema could not update the recurring schedule."
      }
      isSubmitting = false
    }
  }

  private func refresh() {
    guard let recurrence else { return }
    Task { self.recurrence = await model.refreshRecurrence(recurrence) }
  }

  private func occurrenceLabel(schedule: TasksScheduleSnapshot, recurrence: TasksRecurrenceSnapshot) -> String {
    let prefix = recurrence.lifecycle == "ENDED" ? "Ended" : recurrence.lifecycle == "PAUSED" ? "Paused" : "This occurrence"
    return "\(prefix) · \(label(schedule.scheduledFor, timeZone: recurrence.timeZone))"
  }

  private func occurrenceLabel(_ occurrence: TasksRecurrenceOccurrenceSnapshot) -> String {
    if occurrence.trigger == "MANUAL" { return "Run manually" }
    return switch occurrence.resolution {
    case "SKIPPED": "Skipped"
    case "COALESCED": "Combined"
    default: occurrence.taskId == nil ? "Pending" : "Open task"
    }
  }

  private func occurrenceRow(_ occurrence: TasksRecurrenceOccurrenceSnapshot, recurrence: TasksRecurrenceSnapshot) -> some View {
    HStack(spacing: NoemaSpacing.sm) {
      Text(label(occurrence.scheduledFor, timeZone: recurrence.timeZone))
        .font(NoemaFont.metadata)
      Spacer(minLength: NoemaSpacing.sm)
      Text(occurrenceLabel(occurrence))
        .font(NoemaFont.metadata)
        .foregroundStyle(NoemaColor.contentTertiary)
    }
    .contentShape(Rectangle())
  }

  private func label(_ raw: String, timeZone: String) -> String {
    guard let date = TasksISO8601.date(from: raw) else { return raw }
    let formatter = DateFormatter()
    formatter.locale = .current
    formatter.timeZone = TimeZone(identifier: timeZone) ?? .current
    formatter.dateFormat = "EEE, MMM d, h:mm a z"
    return formatter.string(from: date)
  }

}

struct TasksRecurrenceEditSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let recurrence: TasksRecurrenceSnapshot
  let onUpdated: () -> Void
  @State private var draft: TasksScheduleDraft
  @State private var isSubmitting = false
  @State private var errorMessage: String?

  init(model: TasksModel, recurrence: TasksRecurrenceSnapshot, onUpdated: @escaping () -> Void) {
    self.model = model
    self.recurrence = recurrence
    self.onUpdated = onUpdated
    _draft = State(initialValue: TasksScheduleDraft.initial(recurrence: recurrence))
  }

  var body: some View {
    NoemaNativeSheet(title: "Edit recurring schedule", dismissDisabled: isSubmitting, onDismiss: { dismiss() }) {
      ScrollView {
        VStack(alignment: .leading, spacing: 0) {
          Text("Changes apply to future occurrences. Existing tasks and history stay unchanged.")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, NoemaSpacing.lg)
            .padding(.top, NoemaSpacing.md)
            .padding(.bottom, 19)
          TasksScheduleFields(model: model, draft: $draft, recurringOnly: true)
            .padding(.horizontal, NoemaSpacing.lg)
          if let errorMessage {
            NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .warning)
              .padding(.horizontal, NoemaSpacing.lg)
              .padding(.top, NoemaSpacing.md)
          }
          HStack(spacing: NoemaSpacing.sm) {
            Spacer(minLength: 0)
            Button("Cancel") { dismiss() }.buttonStyle(.plain).disabled(isSubmitting)
            Button("Save") { save() }
              .font(NoemaFont.bodyEmphasized)
              .foregroundStyle(NoemaColor.white)
              .frame(minHeight: 32)
              .padding(.horizontal, NoemaSpacing.md)
              .background(NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
              .opacity(canSave ? 1 : 0.42)
              .disabled(!canSave)
          }
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
        }
      }
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.medium, .large], regularHeight: 680)
  }

  private var canSave: Bool { !isSubmitting && model.isConnected && draft.startsAt != nil && draft.cron != nil && !draft.preview.isEmpty }

  private func save() {
    guard let startsAt = draft.startsAt, let cron = draft.cron else { return }
    Task {
      isSubmitting = true
      errorMessage = nil
      let succeeded = await model.updateRecurrence(recurrence, startsAt: startsAt.iso8601String, cronExpression: cron, timeZone: draft.timeZone, missedRunPolicy: draft.missedRunPolicy, overlapPolicy: draft.overlapPolicy)
      isSubmitting = false
      if succeeded { onUpdated(); dismiss() } else { errorMessage = model.lastError ?? "Noema could not save this schedule." }
    }
  }
}

private extension Array {
  subscript(safe index: Index) -> Element? {
    indices.contains(index) ? self[index] : nil
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
