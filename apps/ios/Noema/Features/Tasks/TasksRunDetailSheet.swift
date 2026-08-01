import SwiftUI

struct TasksRunDetailSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  let run: TasksRunSnapshot

  var body: some View {
    NavigationStack {
      List {
        Section("Run") {
          LabeledContent("Activity", value: run.activity.isEmpty ? run.kind.capitalized : run.activity)
          LabeledContent("Status", value: run.status.capitalized)
          LabeledContent("Attempt", value: "\(run.attempt + 1)")
          if let startedAt = run.startedAt {
            LabeledContent("Started", value: startedAt)
          }
          if let endedAt = run.endedAt {
            LabeledContent("Ended", value: endedAt)
          }
          if let error = run.error, !error.isEmpty {
            Text(error)
              .foregroundStyle(NoemaColor.warning)
          }
        }

        Section("Runtime transcript") {
          if model.runItems.isEmpty {
            if model.isConnected {
              ProgressView("Loading run items…")
            } else {
              Text("Cached run items are unavailable for this run.")
                .foregroundStyle(NoemaColor.contentSecondary)
            }
          } else {
            ForEach(model.runItems) { item in
              VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                HStack {
                  Text(item.kind.capitalized)
                    .font(NoemaFont.captionEmphasized)
                  Spacer(minLength: NoemaSpacing.sm)
                  Text(item.status.capitalized)
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                if let content = item.content, !content.isEmpty {
                  Text(content)
                    .font(NoemaFont.body)
                    .textSelection(.enabled)
                }
                Text("#\(item.sequence + 1) · \(item.createdAt)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentTertiary)
              }
              .padding(.vertical, NoemaSpacing.xs)
            }
          }
        }

        Section("Task history") {
          if model.history.isEmpty {
            Text("No completed task history.")
              .foregroundStyle(NoemaColor.contentSecondary)
          } else {
            ForEach(model.history) { item in
              VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                Text(item.title)
                  .font(NoemaFont.bodyEmphasized)
                Text("\(item.stage.name) · \(item.updatedAt)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
            }
          }
        }
      }
      .navigationTitle("Run detail")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Done", systemImage: "xmark") { dismiss() }
            .labelStyle(.iconOnly)
        }
      }
    }
    .presentationDetents([.medium, .large])
    .task(id: run.id) {
      await model.loadRunItems(runId: run.id)
      await model.loadHistory()
    }
    .onDisappear {
      model.clearRunItems()
    }
  }
}
