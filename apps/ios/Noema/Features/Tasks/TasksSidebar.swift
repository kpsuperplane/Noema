import SwiftUI

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
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(
        title: project == nil ? "New project" : "Edit project",
        subtitle: nil,
        onClose: requestDismissal,
        isDisabled: isSaving
      )

      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          TasksSheetField("Name") {
            TextField("", text: $name)
              .textInputAutocapitalization(.sentences)
              .focused($focusedField, equals: .name)
              .noemaProjectSheetField(focused: focusedField == .name, height: 42)
          }
          TasksSheetField("Description (optional)") {
            TextField("", text: $description, axis: .vertical)
              .lineLimit(3...6)
              .focused($focusedField, equals: .description)
              .noemaProjectSheetField(focused: focusedField == .description, height: 76)
          }

          if let project {
            Button {
              Task {
                isSaving = true
                if project.archivedAt == nil {
                  await model.archiveProject(project)
                } else {
                  await model.reopenProject(project)
                }
                isSaving = false
                dismiss()
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
            if let project {
              await model.updateProject(project, name: name.trimmingCharacters(in: .whitespacesAndNewlines), description: description)
            } else {
              await model.createProject(name: name.trimmingCharacters(in: .whitespacesAndNewlines), description: description)
            }
            isSaving = false
            dismiss()
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
    .presentationDetents([.height(project == nil ? 330 : 382)])
    .presentationSizing(.page)
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.container)
    .presentationBackground(NoemaColor.surface)
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
    name != (project?.name ?? "") || description != (project?.description ?? "")
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

private extension View {
  func noemaProjectSheetField(focused: Bool, height: CGFloat) -> some View {
    font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .textFieldStyle(.plain)
      .padding(.horizontal, NoemaSpacing.md)
      .frame(height: height, alignment: .topLeading)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.element)
          .stroke(focused ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focused ? 2 : 1)
      }
  }
}
