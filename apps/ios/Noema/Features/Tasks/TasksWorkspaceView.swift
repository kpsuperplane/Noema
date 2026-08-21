import SwiftUI

private let taskDocumentPath = "TASK.md"
private let taskResultPath = "RESULT.md"
private let taskReviewPath = "REVIEW.md"

struct TasksWorkspaceView: View {
  let model: TasksModel
  let detail: TasksDetailSnapshot
  @State private var selectedPath: String

  init(model: TasksModel, detail: TasksDetailSnapshot) {
    self.model = model
    self.detail = detail
    _selectedPath = State(initialValue: Self.defaultPath(for: detail))
  }

  var body: some View {
    VStack(spacing: 0) {
      TasksWorkspaceFileBar(
        files: files,
        selectedPath: $selectedPath
      )
      if detail.workspaceFilesTruncated {
        Text("Some files are not shown.")
          .font(NoemaFont.compact)
          .foregroundStyle(NoemaColor.contentSecondary)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.vertical, NoemaSpacing.xs)
          .overlay(alignment: .bottom) {
            Rectangle().fill(NoemaColor.separatorSubtle).frame(height: 1)
          }
      }
      filePreview
    }
    .onChange(of: detail.id) { _, _ in
      selectedPath = Self.defaultPath(for: detail)
    }
    .onChange(of: completedResultAvailable) { wasAvailable, isAvailable in
      if !wasAvailable, isAvailable, selectedPath == taskDocumentPath {
        selectedPath = taskResultPath
      }
    }
    .onChange(of: files.map(\.path)) { _, paths in
      if !paths.contains(selectedPath) {
        selectedPath = Self.defaultPath(for: detail)
      }
    }
  }

  @ViewBuilder
  private var filePreview: some View {
    ScrollView {
      switch selectedPath {
      case taskDocumentPath:
        VStack(spacing: 0) {
          TasksDocumentView(
            profile: model.profile,
            document: detail.taskDocument,
            fileName: taskDocumentPath
          )
          TasksTaskMetadataView(detail: detail)
        }
      case taskResultPath:
        TasksDocumentView(
          citations: detail.resultCitations,
          profile: model.profile,
          document: detail.resultDocument ?? "",
          fileName: taskResultPath
        )
      case taskReviewPath where detail.reviewDocument != nil:
        TasksDocumentView(
          profile: model.profile,
          document: detail.reviewDocument ?? "",
          fileName: taskReviewPath
        )
      default:
        TasksWorkspaceSupportFileView(
          model: model,
          taskID: detail.id,
          path: selectedPath,
          taskRevision: detail.updatedAt
        )
      }
    }
    .scrollDismissesKeyboard(.interactively)
  }

  private var files: [TasksWorkspaceFileSnapshot] {
    var byPath: [String: TasksWorkspaceFileSnapshot] = [:]
    for file in detail.workspaceFiles where !file.isDirectory {
      byPath[file.path] = file
    }
    byPath[taskDocumentPath] = TasksWorkspaceFileSnapshot(path: taskDocumentPath, isDirectory: false)
    if detail.resultDocument != nil {
      byPath[taskResultPath] = TasksWorkspaceFileSnapshot(path: taskResultPath, isDirectory: false)
    }
    if detail.reviewDocument != nil {
      byPath[taskReviewPath] = TasksWorkspaceFileSnapshot(path: taskReviewPath, isDirectory: false)
    }
    return byPath.values.sorted { left, right in
      let leftPriority = filePriority(left.path)
      let rightPriority = filePriority(right.path)
      if leftPriority != rightPriority { return leftPriority < rightPriority }
      return left.path.localizedStandardCompare(right.path) == .orderedAscending
    }
  }

  private var completedResultAvailable: Bool {
    detail.stage.behavior == .terminalSuccess && detail.resultDocument?.contains { !$0.isWhitespace } == true
  }

  private func filePriority(_ path: String) -> Int {
    switch path {
    case taskResultPath: 0
    case taskDocumentPath: 1
    default: 2
    }
  }

  private static func defaultPath(for detail: TasksDetailSnapshot) -> String {
    detail.stage.behavior == .terminalSuccess && detail.resultDocument?.contains { !$0.isWhitespace } == true
      ? taskResultPath
      : taskDocumentPath
  }
}

private struct TasksWorkspaceFileBar: View {
  let files: [TasksWorkspaceFileSnapshot]
  @Binding var selectedPath: String

  var body: some View {
    ScrollView(.horizontal) {
      HStack(spacing: NoemaSpacing.xxs) {
        ForEach(files) { file in
          Button {
            selectedPath = file.path
          } label: {
            Text(fileLabel(file.path))
              .font(selectedPath == file.path ? NoemaFont.compactEmphasized : NoemaFont.compact)
              .foregroundStyle(selectedPath == file.path ? NoemaColor.content : NoemaColor.contentSecondary)
              .lineLimit(1)
              .padding(.horizontal, NoemaSpacing.sm)
              .frame(height: 28)
              .background(
                selectedPath == file.path ? NoemaColor.controlFill : Color.clear,
                in: NoemaSuperellipse.full
              )
          }
          .buttonStyle(.plain)
          .accessibilityAddTraits(selectedPath == file.path ? .isSelected : [])
          .help(file.path)
        }
      }
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.xs)
    }
    .scrollIndicators(.hidden)
    .overlay(alignment: .bottom) {
      Rectangle().fill(NoemaColor.separator).frame(height: 1)
    }
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Task workspace files")
  }

  private func fileLabel(_ path: String) -> String {
    let separator = path.lastIndex(of: "/")
    let parent = separator.map { String(path[...$0]) } ?? ""
    let name = separator.map { String(path[path.index(after: $0)...]) } ?? path
    let stem = name.lowercased().hasSuffix(".md") ? String(name.dropLast(3)) : name
    let label = stem == stem.uppercased()
      ? stem.prefix(1).uppercased() + stem.dropFirst().lowercased()
      : stem
    return parent + label
  }
}

private struct TasksWorkspaceSupportFileView: View {
  private enum LoadState: Equatable {
    case loading
    case loaded(String)
    case failed
  }

  let model: TasksModel
  let taskID: String
  let path: String
  let taskRevision: String
  @State private var state: LoadState = .loading

  var body: some View {
    Group {
      switch state {
      case .loading:
        ProgressView("Loading file…")
          .frame(maxWidth: .infinity, alignment: .center)
          .padding(NoemaSpacing.xxl)
      case .loaded(let content):
        if path.lowercased().hasSuffix(".md") {
          TasksDocumentView(document: content, fileName: path)
        } else {
          TasksWorkspacePlainTextView(document: content, fileName: path)
        }
      case .failed:
        NoemaInlineState(
          message: "This file cannot be previewed.",
          symbol: "exclamationmark.triangle",
          tone: .warning
        )
        .padding(NoemaSpacing.lg)
      }
    }
    .task(id: "\(taskID):\(path):\(taskRevision)") {
      state = .loading
      do {
        let content = try await model.workspaceFileText(taskId: taskID, path: path)
        guard !Task.isCancelled else { return }
        state = .loaded(content)
      } catch {
        guard !Task.isCancelled else { return }
        state = .failed
      }
    }
  }
}

private struct TasksWorkspacePlainTextView: View {
  let document: String
  let fileName: String

  var body: some View {
    Group {
      if document.isEmpty {
        Text("\(fileName) has no text content.")
          .font(NoemaFont.taskTitle)
          .foregroundStyle(NoemaColor.contentSecondary)
      } else {
        Text(verbatim: document)
          .font(NoemaFont.mono)
          .foregroundStyle(NoemaColor.content)
          .textSelection(.enabled)
      }
    }
    .frame(maxWidth: 760, alignment: .leading)
    .padding(.horizontal, NoemaSpacing.xxl)
    .padding(.vertical, NoemaSpacing.lg)
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}
