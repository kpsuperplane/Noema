import Foundation
import SwiftUI

struct MemoryRootView: View {
  private let appModel: NoemaAppModel
  @Environment(NoemaShellCoordinator.self) private var shell
  @State private var memory: MemoryModel

  init(model: NoemaAppModel, memory: MemoryModel) {
    appModel = model
    _memory = State(initialValue: memory)
  }

  var body: some View {
    MemoryArticleView(model: memory)
      .task {
        installShellNavigation()
        await memory.load(client: appModel.graphQLClient?.client)
        installShellNavigation()
      }
      .onAppear { installShellNavigation() }
      .onChange(of: appModel.recoveryGeneration) { _, _ in
        Task {
          await memory.load(client: appModel.graphQLClient?.client)
          installShellNavigation()
        }
      }
      .onChange(of: memory.selectedPageID) { _, _ in installShellNavigation() }
      .onChange(of: memory.article?.title) { _, _ in installShellNavigation() }
      .onChange(of: memory.pages.map(\.id)) { _, _ in installShellNavigation() }
  }

  private func installShellNavigation() {
    guard let root = memory.tree?.root else {
      shell.clearSecondary(for: .memory)
      return
    }
    guard memory.pages.contains(where: { $0.id != root.id }) else {
      shell.clearSecondary(for: .memory)
      return
    }

    let model = memory
    shell.show(NoemaSecondaryNavigation(
      title: memory.article?.title ?? root.title,
      icon: memoryIcon(memory.article?.icon ?? root.icon),
      entries: memoryEntries(root: root, pages: model.pages, selectedPageID: model.selectedPageID) { pageID in
        Task { await model.select(pageID: pageID) }
      }
    ), for: .memory)
  }

  private func memoryEntries(
    root: MemoryArticle,
    pages: [MemoryPageRef],
    selectedPageID: String?,
    select: @escaping @MainActor (String) -> Void
  ) -> [NoemaSidebarEntry] {
    let rootRef = MemoryPageRef(
      id: root.id,
      path: root.path,
      title: root.title,
      icon: root.icon,
      excerpt: "",
      hash: root.hash
    )
    let allPages = ([rootRef] + pages).reduce(into: [String: MemoryPageRef]()) { result, page in
      result[page.id] = page
    }
    var children: [String: [MemoryPageRef]] = [:]
    for page in allPages.values where page.id != root.id {
      let parentPath = parentPagePath(page.path) ?? root.path
      let parentID = allPages.values.first(where: { $0.path == parentPath })?.id ?? root.id
      children[parentID, default: []].append(page)
    }

    func flatten(_ page: MemoryPageRef, depth: Int) -> [NoemaSidebarEntry] {
      let entry = NoemaSidebarEntry.item(
        id: "memory-\(page.id)",
        label: page.title,
        icon: memoryIcon(page.icon),
        depth: depth,
        selected: selectedPageID == page.id,
        pinned: depth == 0
      ) { select(page.id) }
      let childEntries = (children[page.id] ?? []).sorted {
        $0.path.localizedStandardCompare($1.path) == .orderedAscending
      }.flatMap { flatten($0, depth: depth + 1) }
      return [entry] + childEntries
    }

    return flatten(rootRef, depth: 0)
  }
}

private struct MemoryArticleView: View {
  let model: MemoryModel
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var sourcesPresented = false

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        NoemaPageTrack(
          maxWidth: 860,
          horizontalPadding: horizontalSizeClass == .compact ? NoemaSpacing.md : NoemaSpacing.xl
        ) {
          if let message = model.errorMessage {
            NoemaInlineState(message: message, symbol: "wifi.slash", tone: .warning)
              .padding(.vertical, NoemaSpacing.sm)
          }
          if let message = model.pageErrorMessage {
            NoemaInlineState(message: message, symbol: "exclamationmark.triangle", tone: .warning)
              .padding(.vertical, NoemaSpacing.sm)
          } else if let article = model.article {
            articleContent(article) { id in
              withAnimation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion)) {
                proxy.scrollTo(id, anchor: .top)
              }
            }
          } else if model.isLoading {
            NoemaInlineState(message: "Loading memory article…", symbol: "arrow.triangle.2.circlepath")
              .padding(.vertical, NoemaSpacing.xxl)
          } else if model.errorMessage == nil {
            NoemaInlineState(message: "No memory article is available.", symbol: "book.closed")
              .padding(.vertical, NoemaSpacing.xxl)
          }
        }
        .padding(.top, NoemaSpacing.sm)
        .padding(.bottom, NoemaSpacing.lg)
      }
      .tracksNoemaSurfaceTop(for: .memory)
    }
    .background(NoemaColor.surface)
    .scrollContentBackground(.hidden)
    .noemaSheet(isPresented: $sourcesPresented) {
      MemoryCitationSheet(citations: model.article?.citations ?? [])
    }
  }

  @ViewBuilder
  private func articleContent(_ article: MemoryArticle, scrollTo: @escaping (String) -> Void) -> some View {
    let prepared = MemoryMarkdown.prepare(body: article.body, citations: article.citations)
    let sections = MemoryMarkdown.sections(prepared.content)
    let hasContents = !prepared.outline.isEmpty || !article.children.isEmpty
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if horizontalSizeClass != .compact {
        Text(article.title)
          .font(NoemaFont.articleTitle)
          .foregroundStyle(NoemaColor.content)
          .textSelection(.enabled)
      }
      Text("From Noema, the private memory encyclopedia")
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.contentTertiary)
      MemoryUpdateNotice(model: model)

      if hasContents, horizontalSizeClass != .compact {
        MemoryArticleFloatLayout {
          MemoryContents(
            outline: prepared.outline,
            hasRelatedArticles: !article.children.isEmpty,
            select: scrollTo
          )
          memoryBody(content: prepared.content, sections: sections)
        }
      } else {
        if hasContents {
          MemoryContents(
            outline: prepared.outline,
            hasRelatedArticles: !article.children.isEmpty,
            select: scrollTo
          )
        }
        memoryBody(content: prepared.content, sections: sections)
      }

      if !article.citations.isEmpty {
        CitationSourcesButton {
          sourcesPresented = true
        }
      }

      if !article.children.isEmpty {
        MemoryRelatedPages(pages: article.children) { pageID in
          Task { await model.select(pageID: pageID) }
        }
        .id("related-articles")
      }
    }
  }

  @ViewBuilder
  private func memoryBody(content: String, sections: [MemoryMarkdownSection]) -> some View {
    if content.isEmpty {
      NoemaInlineState(message: "This biographical article is a stub. It will expand once the first durable facts are recorded.", symbol: "text.book.closed")
        .padding(.vertical, NoemaSpacing.md)
    } else {
      ForEach(sections) { section in
        NoemaMarkdown(section.content, role: .document)
          .padding(.top, section.content.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("#") ? 14 : 0)
          .id(section.id)
      }
    }
  }

}

/// SwiftUI has no text float primitive. This preserves the web geometry at
/// block boundaries: sections wrap beside the 220-point contents rail until
/// they clear it, then return to the full article width.
private struct MemoryArticleFloatLayout: Layout {
  private let contentsWidth: CGFloat = 220
  private let gap = NoemaSpacing.lg

  func sizeThatFits(
    proposal: ProposedViewSize,
    subviews: Subviews,
    cache: inout ()
  ) -> CGSize {
    let measurement = measure(width: proposal.width ?? contentsWidth, subviews: subviews)
    return CGSize(width: proposal.width ?? measurement.width, height: measurement.height)
  }

  func placeSubviews(
    in bounds: CGRect,
    proposal: ProposedViewSize,
    subviews: Subviews,
    cache: inout ()
  ) {
    let measurement = measure(width: bounds.width, subviews: subviews)
    for item in measurement.items {
      subviews[item.index].place(
        at: CGPoint(x: bounds.minX + item.origin.x, y: bounds.minY + item.origin.y),
        proposal: ProposedViewSize(item.size)
      )
    }
  }

  private func measure(width: CGFloat, subviews: Subviews) -> (width: CGFloat, height: CGFloat, items: [Item]) {
    guard !subviews.isEmpty else { return (width, 0, []) }
    let railWidth = min(contentsWidth, width)
    let railSize = subviews[0].sizeThatFits(ProposedViewSize(width: railWidth, height: nil))
    var items = [Item(index: 0, origin: .zero, size: railSize)]
    var y: CGFloat = 0
    for index in subviews.indices.dropFirst() {
      let besideRail = y < railSize.height && width > railWidth + gap
      let x = besideRail ? railWidth + gap : 0
      let availableWidth = max(0, width - x)
      let size = subviews[index].sizeThatFits(ProposedViewSize(width: availableWidth, height: nil))
      items.append(Item(index: index, origin: CGPoint(x: x, y: y), size: size))
      y += size.height + gap
    }
    return (width, max(railSize.height, max(0, y - gap)), items)
  }

  private struct Item {
    let index: Int
    let origin: CGPoint
    let size: CGSize
  }
}

private struct MemoryContents: View {
  let outline: [MemoryOutlineItem]
  let hasRelatedArticles: Bool
  let select: (String) -> Void

  var body: some View {
    NoemaOpaqueSurface {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Contents")
          .font(NoemaFont.taskTitle.weight(.bold))
          .foregroundStyle(NoemaColor.content)
          .frame(minHeight: 21)
          .frame(maxWidth: .infinity, alignment: .center)
        ForEach(outline) { item in
          Button { select(item.id) } label: {
            Text(item.label)
              .font(NoemaFont.taskTitle.weight(.regular))
              .foregroundStyle(NoemaColor.clay600)
              .lineLimit(2)
              .frame(minHeight: 21)
          }
          .buttonStyle(.plain)
          .padding(.leading, item.level > 2 ? NoemaSpacing.md : NoemaSpacing.lg)
        }
        if hasRelatedArticles {
          Button { select("related-articles") } label: {
            Text("Related Articles")
              .font(NoemaFont.taskTitle.weight(.regular))
              .foregroundStyle(NoemaColor.clay600)
              .frame(minHeight: 21)
          }
          .buttonStyle(.plain)
          .padding(.leading, NoemaSpacing.lg)
        }
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.top, NoemaSpacing.md)
      .padding(.bottom, 14)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surfaceSecondary)
      .overlay { Rectangle().stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.bottom, 3)
  }
}

private struct MemoryCitationSheet: View {
  let citations: [MemoryCitation]
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    NoemaNativeSheet(title: "Sources", onDismiss: { dismiss() }) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          ForEach(citations) { citation in
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              Text("Citation \(citation.id)")
                .font(NoemaFont.metadata.weight(.semibold))
                .foregroundStyle(NoemaColor.clay600)
              ForEach(citation.sources) { source in
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  Text("“\(source.excerpt ?? "This source is no longer available.")”")
                    .font(NoemaFont.article)
                    .foregroundStyle(NoemaColor.content)
                    .fixedSize(horizontal: false, vertical: true)
                  Text(sourceMetadata(source))
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                  Text(source.id)
                    .font(NoemaFont.caption.monospaced())
                    .foregroundStyle(NoemaColor.contentTertiary)
                    .textSelection(.enabled)
                }
                if source.id != citation.sources.last?.id {
                  Divider()
                }
              }
            }
            if citation.id != citations.last?.id {
              Divider()
            }
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(NoemaSpacing.lg)
      }
    }
    .presentationDetents([.medium, .large])
    .presentationDragIndicator(.visible)
  }

  private func sourceMetadata(_ source: MemorySource) -> String {
    guard let createdAt = source.createdAt, let date = evidenceDate(createdAt) else {
      return source.kind.label
    }
    return "\(source.kind.label) · \(date)"
  }

  private func evidenceDate(_ value: String) -> String? {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let date = formatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    return date?.formatted(date: .abbreviated, time: .shortened)
  }
}

private struct MemoryRelatedPages: View {
  let pages: [MemoryPageRef]
  let select: (String) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text("Related Articles")
        .font(NoemaFont.title)
        .foregroundStyle(NoemaColor.content)
      LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: NoemaSpacing.sm)], alignment: .leading, spacing: NoemaSpacing.sm) {
        ForEach(pages) { page in
        Button { select(page.id) } label: {
          HStack(alignment: .top, spacing: NoemaSpacing.sm) {
            VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
              Text(page.title)
                .font(NoemaFont.bodyEmphasized)
                .foregroundStyle(NoemaColor.content)
                .lineLimit(1)
              Text(page.excerpt.isEmpty ? "Focused memory article" : page.excerpt)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                .lineLimit(2)
            }
            Spacer(minLength: NoemaSpacing.sm)
            Image(systemName: "arrow.right")
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.contentTertiary)
          }
          .padding(NoemaSpacing.md)
          .frame(maxWidth: .infinity, minHeight: 84, alignment: .leading)
          .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
        }
        .buttonStyle(.plain)
        }
      }
    }
  }
}

private struct MemoryUpdateNotice: View {
  let model: MemoryModel

  var body: some View {
    HStack(alignment: .center, spacing: NoemaSpacing.compact) {
      Text(statusTitle)
        .font(NoemaFont.caption.weight(.bold))
        .foregroundStyle(NoemaColor.content)
      if let update = model.update {
        Text(statusDetail(update))
          .font(NoemaFont.taskPreview)
          .foregroundStyle(update.error == nil ? NoemaColor.contentTertiary : NoemaColor.warning)
          .lineLimit(2)
      }
      Spacer(minLength: NoemaSpacing.sm)
      if model.isUpdating {
        ProgressView().controlSize(.small)
      } else {
        Button(model.updateRetryable ? "Retry" : "Update") { Task { await model.updateMemory() } }
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.content)
          .padding(.horizontal, NoemaSpacing.md)
          .frame(minHeight: 28)
          .background(NoemaColor.content.opacity(0.08), in: NoemaSuperellipse.full)
          .buttonStyle(.plain)
          .disabled(!model.canUpdate)
      }
    }
    .padding(.horizontal, NoemaSpacing.sm)
    .padding(.vertical, NoemaSpacing.xs)
    .frame(minHeight: 38)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(model.updateRetryable ? NoemaColor.red100 : NoemaColor.pine50)
    .overlay {
      Rectangle().stroke(
        model.updateRetryable ? NoemaColor.danger.opacity(0.48) : NoemaColor.separatorSubtle,
        lineWidth: 1
      )
    }
  }

  private var statusTitle: String {
    guard let update = model.update else { return "Memory updates" }
    if update.active || model.isUpdating { return "Updating memory…" }
    if model.updateRetryable { return "Update failed" }
    if update.pendingCount > 0 { return "Not up to date" }
    if update.updatedAt == nil { return "Not updated yet" }
    return "Up to date"
  }

  private func statusDetail(_ update: MemoryUpdateStatus) -> String {
    if let error = model.updateErrorMessage { return error }
    if let error = update.error, !error.isEmpty { return error }
    if update.pendingCount > 0 {
      let age = update.updatedAt.flatMap(relativeAge)
      return age.map { "\(update.pendingCount) pending · \($0)" } ?? "\(update.pendingCount) pending"
    }
    if let updatedAt = update.updatedAt {
      return relativeAge(updatedAt).map { "Last updated \($0)" } ?? "Last updated"
    }
    return update.state.capitalized
  }

  private func relativeAge(_ value: String) -> String? {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let date = formatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    guard let date else { return nil }
    let seconds = max(0, Int(Date().timeIntervalSince(date)))
    if seconds < 60 { return "now" }
    if seconds < 3_600 { return "\(seconds / 60)m ago" }
    if seconds < 86_400 { return "\(seconds / 3_600)h ago" }
    return "\(seconds / 86_400)d ago"
  }
}

private struct MemoryOutlineItem: Identifiable {
  let id: String
  let label: String
  let level: Int
}

private struct MemoryMarkdownSection: Identifiable {
  let id: String
  let content: String
}

private enum MemoryMarkdown {
  static func prepare(body: String, citations: [MemoryCitation]) -> (content: String, outline: [MemoryOutlineItem]) {
    var content = body
    for citation in citations {
      let token = "[^\(citation.id)]"
      if content.contains(token) {
        content = content.replacingOccurrences(
          of: token,
          with: citationSuperscript(citation.id)
        )
      }
    }

    let outline = content.components(separatedBy: .newlines).compactMap { line -> MemoryOutlineItem? in
      let hashes = line.prefix { $0 == "#" }
      guard !hashes.isEmpty, hashes.count <= 6 else { return nil }
      let label = String(line.dropFirst(hashes.count)).trimmingCharacters(in: .whitespaces)
      guard !label.isEmpty else { return nil }
      return MemoryOutlineItem(id: memoryHeadingID(label), label: label, level: hashes.count)
    }
    return (content.trimmingCharacters(in: .whitespacesAndNewlines), outline: outline)
  }

  static func sections(_ content: String) -> [MemoryMarkdownSection] {
    var sections: [MemoryMarkdownSection] = []
    var current: [String] = []
    var currentID = "memory-article-start"
    for line in content.components(separatedBy: .newlines) {
      let hashes = line.prefix { $0 == "#" }
      if !hashes.isEmpty, hashes.count <= 6 {
        if !current.isEmpty {
          sections.append(MemoryMarkdownSection(id: currentID, content: current.joined(separator: "\n")))
        }
        let label = String(line.dropFirst(hashes.count)).trimmingCharacters(in: .whitespaces)
        currentID = memoryHeadingID(label)
        current = [line]
      } else {
        current.append(line)
      }
    }
    if !current.isEmpty {
      sections.append(MemoryMarkdownSection(id: currentID, content: current.joined(separator: "\n")))
    }
    return sections
  }
}

private func parentPagePath(_ path: String) -> String? {
  guard path != "root.md" else { return nil }
  let stem = path.hasSuffix(".md") ? String(path.dropLast(3)) : path
  guard let slash = stem.lastIndex(of: "/") else { return "root.md" }
  return "\(stem[..<slash]).md"
}

private func memoryHeadingID(_ label: String) -> String {
  let value = label
    .lowercased()
    .replacingOccurrences(of: "[^a-z0-9]+", with: "-", options: .regularExpression)
    .trimmingCharacters(in: CharacterSet(charactersIn: "-"))
  return value.isEmpty ? "section" : value
}

private func memoryIcon(_ key: String) -> NoemaIcon.Name {
  NoemaIcon.Name(rawValue: key) ?? .fileText
}
