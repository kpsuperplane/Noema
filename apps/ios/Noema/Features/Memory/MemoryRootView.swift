import MarkdownUI
import SwiftUI

struct MemoryRootView: View {
  private let model: NoemaAppModel
  @State private var memory = MemoryModel()

  init(model: NoemaAppModel) {
    self.model = model
  }

  var body: some View {
    NavigationSplitView {
      MemoryOutline(model: memory)
    } detail: {
      MemoryArticleView(model: memory)
    }
    .navigationSplitViewStyle(.balanced)
    .task {
      await memory.load(client: model.graphQLClient?.client)
    }
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task { await memory.load(client: model.graphQLClient?.client) }
    }
  }
}

private struct MemoryOutline: View {
  @Bindable var model: MemoryModel

  var body: some View {
    List(selection: $model.selectedPageID) {
      if model.isLoading && model.pages.isEmpty {
        ProgressView("Loading memory…")
      }
      if let message = model.errorMessage, model.pages.isEmpty {
        ContentUnavailableView("Memory unavailable", systemImage: "books.vertical", description: Text(message))
      }
      ForEach(model.pages.sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }) { page in
        NavigationLink(value: page.id) {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text(page.title)
              .font(NoemaFont.bodyEmphasized)
              .lineLimit(1)
            if !page.excerpt.isEmpty {
              Text(page.excerpt)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                .lineLimit(2)
            }
          }
          .padding(.leading, indentation(for: page.path))
        }
        .tag(page.id)
      }
    }
    .overlay(alignment: .bottom) {
      if let message = model.errorMessage, !model.pages.isEmpty {
        Text(message)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.warning)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.vertical, NoemaSpacing.sm)
          .frame(maxWidth: .infinity)
          .background(.thinMaterial)
      }
    }
    .navigationTitle("Memory")
    .onChange(of: model.selectedPageID) { _, pageID in
      guard let pageID else { return }
      Task { await model.select(pageID: pageID) }
    }
  }

  private func indentation(for path: String) -> CGFloat {
    let depth = max(0, path.split(separator: "/").count - 1)
    return CGFloat(depth) * NoemaSpacing.md
  }
}

private struct MemoryArticleView: View {
  let model: MemoryModel

  var body: some View {
    Group {
      if let article = model.article {
        ScrollView {
          VStack(alignment: .leading, spacing: NoemaSpacing.xl) {
            header(article)
            if !article.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
              Markdown(article.body)
                .markdownTextStyle {
                  ForegroundColor(NoemaColor.content)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            if !article.children.isEmpty {
              relatedPages(article.children)
            }
            if !article.sources.isEmpty {
              references(article.sources)
            }
          }
          .frame(maxWidth: 760, alignment: .leading)
          .padding(.horizontal, NoemaSpacing.xl)
          .padding(.vertical, NoemaSpacing.xxl)
          .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(NoemaColor.surface)
      } else if model.isLoading {
        ProgressView("Loading article…")
          .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        ContentUnavailableView("Select a memory article", systemImage: "book.closed", description: Text("Choose a page from the hierarchy."))
          .frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
    .navigationTitle(model.article?.title ?? "Memory")
    .navigationBarTitleDisplayMode(.inline)
  }

  @ViewBuilder
  private func header(_ article: MemoryArticle) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      if !article.ancestors.isEmpty {
        Text(article.ancestors.map(\.title).joined(separator: "  /  "))
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      Text(article.title)
        .font(.largeTitle.weight(.semibold))
        .foregroundStyle(NoemaColor.content)
        .textSelection(.enabled)
      MemoryUpdateNotice(model: model)
    }
  }

  @ViewBuilder
  private func relatedPages(_ pages: [MemoryPageRef]) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text("Related pages")
        .font(NoemaFont.title)
      ForEach(pages) { page in
        Button {
          Task { await model.select(pageID: page.id) }
        } label: {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text(page.title)
              .font(NoemaFont.bodyEmphasized)
              .frame(maxWidth: .infinity, alignment: .leading)
            if !page.excerpt.isEmpty {
              Text(page.excerpt)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
          }
          .padding(NoemaSpacing.md)
          .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))
        }
        .buttonStyle(.plain)
        .foregroundStyle(NoemaColor.content)
      }
    }
  }

  @ViewBuilder
  private func references(_ sources: [MemorySource]) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text("References")
        .font(NoemaFont.title)
      ForEach(Array(sources.enumerated()), id: \.element.id) { index, source in
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("\(index + 1). \(source.id)")
            .font(NoemaFont.bodyEmphasized)
          if let excerpt = source.excerpt, !excerpt.isEmpty {
            Text(excerpt)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
      }
    }
  }
}

private struct MemoryUpdateNotice: View {
  let model: MemoryModel

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text(statusTitle)
          .font(NoemaFont.bodyEmphasized)
        if let update = model.update {
          Text(statusDetail(update))
            .font(NoemaFont.caption)
            .foregroundStyle(update.error == nil ? NoemaColor.contentSecondary : NoemaColor.warning)
        }
        if model.isOffline {
          Text("Cached data · reconnect to edit memory.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
      }
      Spacer(minLength: NoemaSpacing.sm)
      Button {
        Task { await model.updateMemory() }
      } label: {
        if model.isUpdating {
          ProgressView()
            .controlSize(.small)
        } else {
          Label("Update", systemImage: "arrow.triangle.2.circlepath")
        }
      }
      .buttonStyle(.borderedProminent)
      .controlSize(.small)
      .disabled(!model.canUpdate)
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))
  }

  private var statusTitle: String {
    guard let update = model.update else { return "Memory updates" }
    if update.active { return "Updating memory…" }
    if update.error != nil { return "Memory update needs attention" }
    if update.pendingCount > 0 { return "Memory has new source messages" }
    return "Memory is up to date"
  }

  private func statusDetail(_ update: MemoryUpdateStatus) -> String {
    if let error = update.error, !error.isEmpty { return error }
    if update.pendingCount > 0 { return "\(update.pendingCount) pending message(s)" }
    if let updatedAt = update.updatedAt { return "Last updated \(updatedAt)" }
    return update.state.capitalized
  }
}
