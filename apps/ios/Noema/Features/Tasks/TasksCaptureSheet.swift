import SwiftUI

struct TasksCaptureSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  @State private var title = ""
  @State private var description = ""
  @State private var projectId: String?
  @FocusState private var focusedField: Field?
  @State private var isSaving = false
  @State private var discardPresented = false

  private enum Field: Hashable {
    case title
    case description
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Capture") {
          TextField("What needs doing?", text: $title)
            .textInputAutocapitalization(.sentences)
            .focused($focusedField, equals: .title)
          TextField("Description", text: $description, axis: .vertical)
            .lineLimit(4...10)
            .focused($focusedField, equals: .description)
        }
        Section("Project") {
          Picker("Project", selection: $projectId) {
            Text("Personal").tag(Optional<String>.none)
            ForEach(model.projects) { project in
              Text(project.name).tag(Optional(project.id))
            }
          }
        }
      }
      .navigationTitle("Capture task")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Cancel", systemImage: "xmark") {
            if isDirty { discardPresented = true } else { dismiss() }
          }
            .labelStyle(.iconOnly)
        }
        ToolbarItem(placement: .confirmationAction) {
          Button("Capture", systemImage: "checkmark") {
            Task {
              isSaving = true
              await model.capture(title: title.trimmingCharacters(in: .whitespacesAndNewlines), description: description, projectId: projectId)
              isSaving = false
              dismiss()
            }
          }
          .labelStyle(.iconOnly)
          .buttonStyle(.borderedProminent)
          .overlay {
            if isSaving { ProgressView().controlSize(.small) }
          }
          .disabled(isSaving || title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !model.isConnected)
        }
      }
    }
    .presentationDetents([.medium, .large])
    .interactiveDismissDisabled(isDirty || isSaving)
    .confirmationDialog("Discard capture?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard capture", role: .destructive) { dismiss() }
    }
    .task {
      focusedField = .title
    }
  }

  private var isDirty: Bool {
    !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      !description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      projectId != nil
  }
}
