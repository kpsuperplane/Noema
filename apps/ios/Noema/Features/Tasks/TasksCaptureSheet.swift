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
  @State private var errorMessage: String?

  private enum Field: Hashable {
    case title
    case description
  }

  init(model: TasksModel) {
    self.model = model
    _projectId = State(initialValue: model.selectedProjectId)
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 0) {
        HStack(alignment: .top, spacing: NoemaSpacing.md) {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("New task")
              .font(NoemaFont.mobileTitle)
              .foregroundStyle(NoemaColor.content)
            Text("Saved to Inbox until you queue it.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
          Spacer(minLength: 0)
          Button {
            requestDismissal()
          } label: {
            Image(systemName: "xmark")
              .font(.system(size: 15, weight: .medium))
              .frame(width: 32, height: 32)
          }
          .buttonStyle(.plain)
          .disabled(isSaving)
          .accessibilityLabel("Close")
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.md)
        .padding(.bottom, 19)

        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          field("Title") {
            TextField("", text: $title)
              .textInputAutocapitalization(.sentences)
              .focused($focusedField, equals: .title)
              .noemaSheetField(focused: focusedField == .title, height: 46)
          }
          field("Description (optional)") {
            TextField("", text: $description, axis: .vertical)
              .lineLimit(3...3)
              .focused($focusedField, equals: .description)
              .noemaSheetField(focused: focusedField == .description, height: 76)
          }
          field("Project (optional)") {
            Picker(selection: $projectId) {
              Text("No project").tag(Optional<String>.none)
              ForEach(model.projects.filter { $0.archivedAt == nil }) { project in
                Text(project.name).tag(Optional(project.id))
              }
            } label: {
              Text("Project (optional)")
            }
            .pickerStyle(.menu)
            .tint(NoemaColor.content)
            .frame(maxWidth: .infinity, minHeight: 40, maxHeight: 40, alignment: .leading)
            .padding(.horizontal, NoemaSpacing.md)
            .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
            .overlay {
              RoundedRectangle(cornerRadius: NoemaRadius.element)
                .stroke(NoemaColor.separator, lineWidth: 1)
            }
          }
        }
        .padding(.horizontal, NoemaSpacing.lg)

        if let errorMessage {
          NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .warning)
            .padding(.horizontal, NoemaSpacing.lg)
            .padding(.top, NoemaSpacing.md)
        }

        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { requestDismissal() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .disabled(isSaving)
          Button {
            Task {
              isSaving = true
              errorMessage = nil
              let succeeded = await model.capture(
                title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                description: description,
                projectId: projectId
              )
              isSaving = false
              if succeeded {
                dismiss()
              } else {
                errorMessage = model.lastError ?? "The task could not be captured. Try again."
              }
            }
          } label: {
            Text("Add to Inbox")
              .frame(minHeight: 32)
              .padding(.horizontal, NoemaSpacing.md)
            .overlay {
              if isSaving { ProgressView().controlSize(.small) }
            }
          }
          .buttonStyle(.plain)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.clay600, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .opacity(canSave ? 1 : 0.42)
          .disabled(!canSave)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, 32)
        .padding(.bottom, NoemaSpacing.sm)
      }
    }
    .scrollBounceBehavior(.basedOnSize)
    .frame(maxHeight: .infinity, alignment: .top)
    .background(NoemaColor.surface)
    .presentationDetents([.height(379)])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
    .interactiveDismissDisabled(isDirty || isSaving)
    .sheet(isPresented: $discardPresented) {
      TasksDiscardSheet(
        title: "Discard capture?",
        message: "Your new task draft will be lost.",
        confirmTitle: "Discard capture"
      ) {
        dismiss()
      }
    }
    .task {
      focusedField = .title
    }
  }

  private var canSave: Bool {
    !isSaving && !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.isConnected
  }

  @ViewBuilder
  private func field<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.content)
      content()
    }
  }

  private func requestDismissal() {
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private var isDirty: Bool {
    !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      !description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
      projectId != nil
  }
}

private extension View {
  func noemaSheetField(focused: Bool, height: CGFloat) -> some View {
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

/// The web form drawer uses a compact, opaque header instead of a navigation bar.
/// Keep this primitive in the Tasks feature so every task editor shares the same
/// geometry without pulling a second sheet system into the app shell.
struct TasksSheetHeader: View {
  let title: String
  let subtitle: String?
  let onClose: () -> Void
  var isDisabled = false

  var body: some View {
    HStack(alignment: .top, spacing: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: subtitle == nil ? 0 : NoemaSpacing.xs) {
        Text(title)
          .font(NoemaFont.mobileTitle)
          .foregroundStyle(NoemaColor.content)
        if let subtitle {
          Text(subtitle)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      Spacer(minLength: 0)
      Button(action: onClose) {
        Image(systemName: "xmark")
          .font(.system(size: 15, weight: .medium))
          .frame(width: 32, height: 32)
      }
      .buttonStyle(.plain)
      .foregroundStyle(NoemaColor.contentSecondary)
      .disabled(isDisabled)
      .accessibilityLabel("Close")
    }
    .padding(.horizontal, NoemaSpacing.lg)
    .padding(.top, NoemaSpacing.md)
    .padding(.bottom, subtitle == nil ? NoemaSpacing.sm : 19)
  }
}

struct TasksSheetField<Content: View>: View {
  let label: String
  @ViewBuilder let content: () -> Content

  init(_ label: String, @ViewBuilder content: @escaping () -> Content) {
    self.label = label
    self.content = content
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.taskTitle)
        .foregroundStyle(NoemaColor.content)
      content()
    }
  }
}

struct TasksDiscardSheet: View {
  @Environment(\.dismiss) private var dismiss
  let title: String
  let message: String
  let confirmTitle: String
  let onDiscard: () -> Void

  init(title: String, message: String, confirmTitle: String = "Discard", onDiscard: @escaping () -> Void) {
    self.title = title
    self.message = message
    self.confirmTitle = confirmTitle
    self.onDiscard = onDiscard
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(title: title, subtitle: nil, onClose: { dismiss() })
      Text(message)
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.contentSecondary)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.horizontal, NoemaSpacing.lg)
      HStack(spacing: NoemaSpacing.sm) {
        Spacer(minLength: 0)
        Button("Keep editing") { dismiss() }
          .buttonStyle(.plain)
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.content)
        Button(confirmTitle, role: .destructive) {
          onDiscard()
          dismiss()
        }
        .buttonStyle(.plain)
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.danger)
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.top, NoemaSpacing.xl)
      .padding(.bottom, NoemaSpacing.sm)
    }
    .frame(maxHeight: .infinity, alignment: .top)
    .background(NoemaColor.surface)
    .presentationDetents([.height(194)])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
  }
}
