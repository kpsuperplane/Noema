import SwiftUI

struct TasksWorkToolbar: View {
  @Bindable var model: TasksModel
  @Binding var capturePresented: Bool
  @Binding var createProjectPresented: Bool
  @Binding var projectEditor: TasksProjectSnapshot?
  let wide: Bool

  @ViewBuilder
  var body: some View {
    if wide {
      HStack(alignment: .center, spacing: NoemaSpacing.sm) {
        Text("Tasks")
          .font(NoemaFont.pageTitle)
          .foregroundStyle(NoemaColor.content)
        Spacer(minLength: NoemaSpacing.sm)
        Button("New task", systemImage: "plus") {
          capturePresented = true
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        .disabled(!model.isConnected)
        projectMenu
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.top, NoemaSpacing.lg)
      .padding(.bottom, NoemaSpacing.sm)
    }
  }

  private var projectMenu: some View {
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
                projectEditor = project
              }
            } else {
              Button("Reopen project", systemImage: "arrow.uturn.backward") {
                projectEditor = project
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
}
