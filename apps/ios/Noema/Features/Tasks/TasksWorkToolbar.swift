import SwiftUI

struct TasksWorkToolbar: View {
  @Bindable var model: TasksModel
  @Binding var capturePresented: Bool
  @Binding var createProjectPresented: Bool
  @Binding var projectEditor: TasksProjectSnapshot?
  let wide: Bool

  var body: some View {
    HStack(alignment: .center, spacing: NoemaSpacing.sm) {
      if wide {
        Text("Tasks")
          .font(NoemaFont.pageTitle)
          .foregroundStyle(NoemaColor.content)
      } else {
        Button("New task", systemImage: "plus") { capturePresented = true }
          .font(NoemaFont.captionEmphasized)
          .buttonStyle(.bordered)
          .controlSize(.small)
          .disabled(!model.isConnected)
      }
      Spacer(minLength: NoemaSpacing.sm)
      Menu {
        Button("New project", systemImage: "folder.badge.plus") {
          createProjectPresented = true
        }
        if !model.projects.isEmpty {
          Divider()
          ForEach(model.projects) { project in
            Section(project.name) {
              Button("Edit project", systemImage: "pencil") {
                projectEditor = project
              }
              if project.archivedAt == nil {
                Button("Archive project", systemImage: "archivebox", role: .destructive) {
                  Task { await model.archiveProject(project) }
                }
              } else {
                Button("Reopen project", systemImage: "arrow.uturn.backward") {
                  Task { await model.reopenProject(project) }
                }
              }
            }
          }
        }
      } label: {
        Image(systemName: "folder")
          .frame(width: 32, height: 32)
      }
      .buttonStyle(.plain)
      .foregroundStyle(NoemaColor.contentSecondary)
      .disabled(!model.isConnected)
      .accessibilityLabel("Project actions")
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.top, NoemaSpacing.lg)
    .padding(.bottom, NoemaSpacing.sm)
  }
}
