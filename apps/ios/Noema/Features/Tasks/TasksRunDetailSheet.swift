import SwiftUI

struct TasksRunDetailSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  let run: TasksRunSnapshot

  var body: some View {
    NavigationStack {
      ScrollView {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          runSummary
          transcript
          history
        }
        .padding(NoemaSpacing.lg)
        .frame(maxWidth: 760, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .center)
      }
      .background(NoemaColor.surface)
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

  private var runSummary: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      sectionTitle("Run")
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
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.warning)
      }
    }
  }

  private var transcript: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      sectionTitle("Runtime transcript")
      if model.runItems.isEmpty {
        if model.isConnected {
          ProgressView("Loading run items…")
        } else {
          Text("Cached run items are unavailable for this run.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
      } else {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
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
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(NoemaSpacing.md)
            .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
            .overlay {
              RoundedRectangle(cornerRadius: NoemaRadius.element)
                .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
            }
          }
        }
      }
    }
  }

  private var history: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      sectionTitle("Task history")
      if model.history.isEmpty {
        Text("No completed task history.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      } else {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          ForEach(model.history) { item in
            VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
              Text(item.title)
                .font(NoemaFont.bodyEmphasized)
              Text("\(item.stage.name) · \(item.updatedAt)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
          }
        }
      }
    }
  }

  private func sectionTitle(_ title: String) -> some View {
    Text(title)
      .font(NoemaFont.captionEmphasized)
      .foregroundStyle(NoemaColor.contentSecondary)
  }
}
