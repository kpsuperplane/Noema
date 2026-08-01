import SwiftUI

struct TasksSidebar: View {
  @Bindable var model: TasksModel
  @Binding var selectedProjectId: String?
  @Binding var createProjectPresented: Bool
  @Binding var projectEditor: TasksProjectSnapshot?

  var body: some View {
    ScrollView {
      LazyVStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text("Workspaces")
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(NoemaColor.contentTertiary)
          .textCase(.uppercase)
          .padding(.horizontal, NoemaSpacing.md)
          .padding(.top, NoemaSpacing.md)
          .padding(.bottom, NoemaSpacing.xs)

        projectButton(id: nil, label: "Personal", symbol: "person")

        if !model.projects.isEmpty {
          Text("Projects")
            .font(NoemaFont.metadata.weight(.semibold))
            .foregroundStyle(NoemaColor.contentTertiary)
            .textCase(.uppercase)
            .padding(.horizontal, NoemaSpacing.md)
            .padding(.top, NoemaSpacing.md)
            .padding(.bottom, NoemaSpacing.xs)
          ForEach(model.projects) { project in
            projectButton(id: project.id, label: project.name, symbol: project.archivedAt == nil ? "folder" : "archivebox")
              .contextMenu {
                Button("Edit Project", systemImage: "pencil") { projectEditor = project }
                if project.archivedAt == nil {
                  Button("Archive Project", systemImage: "archivebox", role: .destructive) {
                    Task { await model.archiveProject(project) }
                  }
                } else {
                  Button("Reopen Project", systemImage: "arrow.uturn.backward") {
                    Task { await model.reopenProject(project) }
                  }
                }
              }
          }
        }
      }
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.bottom, NoemaSpacing.lg)
    }
    .scrollIndicators(.hidden)
    .background(NoemaColor.pine50)
    .navigationTitle("Tasks")
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) {
        Button("New project", systemImage: "folder.badge.plus") {
          createProjectPresented = true
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.glass)
        .disabled(!model.isConnected)
        .accessibilityLabel("New project")
      }
    }
    .onChange(of: selectedProjectId) { _, value in
      Task { await model.selectProject(value) }
    }
  }

  private func projectButton(id: String?, label: String, symbol: String) -> some View {
    Button {
      selectedProjectId = id
    } label: {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: symbol)
          .frame(width: 18)
          .accessibilityHidden(true)
        Text(label)
          .lineLimit(1)
        Spacer(minLength: 0)
      }
      .font(NoemaFont.body)
      .foregroundStyle(selectedProjectId == id ? NoemaColor.pine700 : NoemaColor.contentSecondary)
      .padding(.horizontal, NoemaSpacing.md)
      .frame(minHeight: 38)
      .background(selectedProjectId == id ? NoemaColor.pine100.opacity(0.48) : Color.clear, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .accessibilityAddTraits(selectedProjectId == id ? .isSelected : [])
  }
}

struct TasksProjectSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let project: TasksProjectSnapshot?
  @State private var name: String
  @State private var description: String
  @FocusState private var focusedField: Field?
  @State private var isSaving = false
  @State private var discardPresented = false

  private enum Field: Hashable {
    case name
    case description
  }

  init(model: TasksModel, project: TasksProjectSnapshot?) {
    self.model = model
    self.project = project
    _name = State(initialValue: project?.name ?? "")
    _description = State(initialValue: project?.description ?? "")
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Project") {
          TextField("Name", text: $name)
            .focused($focusedField, equals: .name)
          TextField("Description", text: $description, axis: .vertical)
            .lineLimit(3...8)
            .focused($focusedField, equals: .description)
        }

        if let project {
          Section("Project actions") {
            if project.archivedAt == nil {
              Button("Archive project", systemImage: "archivebox", role: .destructive) {
                Task {
                  isSaving = true
                  await model.archiveProject(project)
                  isSaving = false
                  dismiss()
                }
              }
              .disabled(isSaving || !model.isConnected)
            } else {
              Button("Reopen project", systemImage: "arrow.uturn.backward") {
                Task {
                  isSaving = true
                  await model.reopenProject(project)
                  isSaving = false
                  dismiss()
                }
              }
              .disabled(isSaving || !model.isConnected)
            }
          }
        }
      }
      .navigationTitle(project == nil ? "New project" : "Edit project")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Cancel", systemImage: "xmark") {
            if isDirty { discardPresented = true } else { dismiss() }
          }
            .labelStyle(.iconOnly)
        }
        ToolbarItem(placement: .confirmationAction) {
          Button("Save", systemImage: "checkmark") {
            Task {
              isSaving = true
              if let project {
                await model.updateProject(project, name: name, description: description)
              } else {
                await model.createProject(name: name, description: description)
              }
              isSaving = false
              dismiss()
            }
          }
          .labelStyle(.iconOnly)
          .buttonStyle(.borderedProminent)
          .overlay {
            if isSaving { ProgressView().controlSize(.small) }
          }
          .disabled(isSaving || !isDirty || name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !model.isConnected)
        }
      }
    }
    .presentationDetents([.medium, .large])
    .interactiveDismissDisabled(isDirty || isSaving)
    .confirmationDialog("Discard changes?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard changes", role: .destructive) { dismiss() }
    }
    .task {
      focusedField = .name
    }
  }

  private var isDirty: Bool {
    name != (project?.name ?? "") || description != (project?.description ?? "")
  }
}
