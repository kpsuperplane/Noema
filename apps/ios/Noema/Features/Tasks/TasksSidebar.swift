import SwiftUI

struct TasksProjectSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let project: TasksProjectSnapshot?
  @State private var name: String
  @State private var description: String
  @State private var folder: String
  @FocusState private var focusedField: Field?
  @State private var isSaving = false
  @State private var discardPresented = false
  @State private var errorMessage: String?

  private enum Field: Hashable {
    case name
    case description
  }

  init(model: TasksModel, project: TasksProjectSnapshot?) {
    self.model = model
    self.project = project
    _name = State(initialValue: project?.name ?? "")
    _description = State(initialValue: project?.description ?? "")
    _folder = State(initialValue: project?.folder ?? "")
  }

  var body: some View {
    NoemaNativeSheet(
      title: project == nil ? "New project" : "Edit project",
      dismissDisabled: isSaving,
      onDismiss: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: 0) {
        ScrollView {
          VStack(alignment: .leading, spacing: NoemaSpacing.md) {
            TasksSheetField("Name") {
              TextField("", text: $name)
                .textInputAutocapitalization(.sentences)
                .focused($focusedField, equals: .name)
                .noemaTaskSheetField(focused: focusedField == .name, height: 42)
            }
            TasksSheetField("Description (optional)") {
              TextField("", text: $description, axis: .vertical)
                .lineLimit(3...6)
                .focused($focusedField, equals: .description)
                .noemaTaskSheetField(focused: focusedField == .description, height: 76)
            }
            TasksSheetField("Project folder (optional)") {
              TextField("/absolute/path/to/project", text: $folder)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .noemaTaskSheetField(focused: false, height: 42)
            }
            Text("Must be an absolute path in the selected executor's filesystem.")
              .font(NoemaFont.metadata)
              .foregroundStyle(NoemaColor.contentTertiary)

            if let project {
              Button {
                Task {
                  isSaving = true
                  errorMessage = nil
                  let succeeded: Bool
                  if project.archivedAt == nil {
                    succeeded = await model.archiveProject(project)
                  } else {
                    succeeded = await model.reopenProject(project)
                  }
                  isSaving = false
                  if succeeded {
                    dismiss()
                  } else {
                    errorMessage = model.lastError ?? "Noema could not update this project."
                  }
                }
              } label: {
                Label(
                  project.archivedAt == nil ? "Archive project" : "Reopen project",
                  systemImage: project.archivedAt == nil ? "archivebox" : "arrow.uturn.backward"
                )
                .font(NoemaFont.body)
                .foregroundStyle(project.archivedAt == nil ? NoemaColor.danger : NoemaColor.content)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, NoemaSpacing.sm)
              }
              .buttonStyle(.plain)
              .disabled(isSaving || !model.isConnected)
            }

            if let errorMessage {
              Text(errorMessage)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.danger)
                .fixedSize(horizontal: false, vertical: true)
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
              errorMessage = nil
              let succeeded: Bool
              if let project {
                let trimmedFolder = folder.trimmingCharacters(in: .whitespacesAndNewlines)
                succeeded = await model.updateProject(project, name: name.trimmingCharacters(in: .whitespacesAndNewlines), description: description, folder: trimmedFolder.isEmpty ? nil : trimmedFolder, clearFolder: trimmedFolder.isEmpty && project.folder != nil)
              } else {
                let trimmedFolder = folder.trimmingCharacters(in: .whitespacesAndNewlines)
                succeeded = await model.createProject(name: name.trimmingCharacters(in: .whitespacesAndNewlines), description: description, folder: trimmedFolder.isEmpty ? nil : trimmedFolder)
              }
              isSaving = false
              if succeeded {
                dismiss()
              } else {
                errorMessage = model.lastError ?? "Noema could not save this project."
              }
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
    }
    .noemaTaskSheetPresentation([.height(project == nil ? 430 : 480)], regularHeight: project == nil ? 560 : 620)
    .interactiveDismissDisabled(isDirty || isSaving)
    .sheet(isPresented: $discardPresented) {
      TasksDiscardSheet(title: "Discard changes?", message: "Your project edits will be lost.") {
        dismiss()
      }
    }
    .task {
      focusedField = .name
    }
  }

  private var isDirty: Bool {
    name != (project?.name ?? "") || description != (project?.description ?? "") || folder != (project?.folder ?? "")
  }

  private var canSave: Bool {
    !isSaving && isDirty && !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.isConnected
  }

  private func requestDismissal() {
    if isDirty {
      discardPresented = true
    } else {
      dismiss()
    }
  }
}
