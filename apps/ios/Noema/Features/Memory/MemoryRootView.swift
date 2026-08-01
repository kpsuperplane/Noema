import MarkdownUI
import Foundation
import SwiftUI

struct MemoryRootView: View {
  private let appModel: NoemaAppModel
  @Environment(NoemaShellCoordinator.self) private var shell
  @State private var memory = MemoryModel()

  init(model: NoemaAppModel) {
    appModel = model
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
      shell.show(NoemaSecondaryNavigation(
        title: "Memory",
        symbol: "brain",
        entries: [
          .item(id: "memory-root", label: "Memory", symbol: "brain", selected: true) {}
        ]
      ))
      return
    }

    let model = memory
    shell.show(NoemaSecondaryNavigation(
      title: memory.article?.title ?? root.title,
      symbol: "brain",
      entries: memoryEntries(root: root, pages: model.pages, selectedPageID: model.selectedPageID) { pageID in
        Task { await model.select(pageID: pageID) }
      }
    ))
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
        symbol: memorySymbol(page.icon),
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
  @State private var selectedCitation: MemoryCitation?

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        NoemaPageTrack(
          maxWidth: 860,
          horizontalPadding: horizontalSizeClass == .compact ? NoemaSpacing.md : NoemaSpacing.xl
        ) {
          if let article = model.article {
            articleContent(article) { id in
              withAnimation(NoemaSpring.standard) { proxy.scrollTo(id, anchor: .top) }
            }
          } else if model.isLoading {
            NoemaInlineState(message: "Loading memory article…", symbol: "arrow.triangle.2.circlepath")
              .padding(.vertical, NoemaSpacing.xxl)
          } else if let message = model.errorMessage {
            NoemaInlineState(message: message, symbol: "wifi.slash", tone: .warning)
              .padding(.vertical, NoemaSpacing.xxl)
          } else {
            NoemaInlineState(message: "No memory article is available.", symbol: "book.closed")
              .padding(.vertical, NoemaSpacing.xxl)
          }
        }
        .padding(.top, NoemaSpacing.md)
        .padding(.bottom, NoemaSpacing.lg)
      }
      .environment(\.openURL, OpenURLAction { url in
        guard url.scheme == "noema-citation", let number = url.host else { return .systemAction }
        selectedCitation = citation(number: number)
        return .handled
      })
    }
    .background(NoemaColor.surface)
    .scrollContentBackground(.hidden)
    .sheet(item: $selectedCitation) { citation in
      MemoryCitationSheet(citation: citation)
    }
  }

  @ViewBuilder
  private func articleContent(_ article: MemoryArticle, scrollTo: @escaping (String) -> Void) -> some View {
    let prepared = MemoryMarkdown.prepare(body: article.body, sources: article.sources)
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if horizontalSizeClass != .compact, article.id != model.tree?.root?.id {
        HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
          Image(systemName: memorySymbol(article.icon))
            .font(.system(size: 18, weight: .semibold))
            .foregroundStyle(NoemaColor.accent)
            .accessibilityHidden(true)
          Text(article.title)
            .font(NoemaFont.articleTitle)
            .foregroundStyle(NoemaColor.content)
            .textSelection(.enabled)
        }
      }
      Text("From Noema, the private memory encyclopedia")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
      MemoryUpdateNotice(model: model)

      if !prepared.outline.isEmpty || !article.children.isEmpty {
        MemoryContents(
          outline: prepared.outline,
          hasRelatedArticles: !article.children.isEmpty,
          select: scrollTo
        )
      }

      if prepared.content.isEmpty {
        NoemaInlineState(message: "This article is a stub. It will expand as durable facts are recorded.", symbol: "text.book.closed")
          .padding(.vertical, NoemaSpacing.md)
      } else {
        ForEach(MemoryMarkdown.sections(prepared.content)) { section in
          MemoryMarkdownBody(content: section.content)
            .id(section.id)
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

  private func citation(number: String) -> MemoryCitation? {
    guard let index = Int(number).map({ $0 - 1 }), index >= 0, let article = model.article else { return nil }
    let citations = MemoryMarkdown.prepare(body: article.body, sources: article.sources).citations
    guard citations.indices.contains(index) else { return nil }
    return citations[index]
  }
}

private struct MemoryContents: View {
  let outline: [MemoryOutlineItem]
  let hasRelatedArticles: Bool
  let select: (String) -> Void

  var body: some View {
    NoemaOpaqueSurface {
      VStack(alignment: .leading, spacing: 10) {
        Text("Contents")
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.content)
          .frame(maxWidth: .infinity, alignment: .center)
        ForEach(outline) { item in
          Button { select(item.id) } label: {
            Text(item.label)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.clay600)
              .lineLimit(2)
          }
          .buttonStyle(.plain)
          .padding(.leading, item.level > 2 ? NoemaSpacing.md : NoemaSpacing.lg)
        }
        if hasRelatedArticles {
          Button { select("related-articles") } label: {
            Text("Related Articles")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.clay600)
          }
          .buttonStyle(.plain)
          .padding(.leading, NoemaSpacing.lg)
        }
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.vertical, NoemaSpacing.lg)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surfaceSecondary)
      .overlay { Rectangle().stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.bottom, 3)
  }
}

private struct MemoryMarkdownHeading<Label: View>: View {
  let label: Label
  let major: Bool

  var body: some View {
    label
      .markdownTextStyle {
        FontFamily(.custom("Georgia"))
        FontSize(major ? 24 : 19)
        FontWeight(.regular)
        ForegroundColor(NoemaColor.content)
      }
      .lineSpacing(3)
      .markdownMargin(top: major ? 32 : NoemaSpacing.lg, bottom: NoemaSpacing.sm)
      .padding(.bottom, NoemaSpacing.xs)
      .overlay(alignment: .bottom) {
        Rectangle()
          .fill(NoemaColor.separator)
          .frame(height: 1)
      }
  }
}

private struct MemoryMarkdownBody: View {
  let content: String

  var body: some View {
    Markdown(content)
      .markdownTextStyle {
        FontFamily(.custom("Georgia"))
        FontSize(15)
        ForegroundColor(NoemaColor.content)
      }
      .markdownTextStyle(\.link) {
        FontFamily(.custom("Georgia"))
        FontSize(10)
        ForegroundColor(NoemaColor.clay600)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        configuration.label
          .lineSpacing(8.25)
          .markdownMargin(top: NoemaSpacing.md, bottom: NoemaSpacing.sm)
      }
      .markdownBlockStyle(\.heading1) { configuration in
        MemoryMarkdownHeading(label: configuration.label, major: true)
      }
      .markdownBlockStyle(\.heading2) { configuration in
        MemoryMarkdownHeading(label: configuration.label, major: true)
      }
      .markdownBlockStyle(\.heading3) { configuration in
        MemoryMarkdownHeading(label: configuration.label, major: false)
      }
      .markdownBlockStyle(\.heading4) { configuration in
        MemoryMarkdownHeading(label: configuration.label, major: false)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .textSelection(.enabled)
  }
}

private struct MemoryCitationSheet: View {
  let citation: MemoryCitation
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      HStack {
        Text("Source").font(NoemaFont.title)
        Spacer(minLength: NoemaSpacing.sm)
        Button("Close", systemImage: "xmark") { dismiss() }
          .labelStyle(.iconOnly)
          .buttonStyle(.glass)
      }
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          Text("“\(citation.excerpt ?? "The source conversation message is no longer available.")”")
            .font(NoemaFont.article)
            .foregroundStyle(NoemaColor.content)
            .fixedSize(horizontal: false, vertical: true)
          Text("Your message in the primary conversation")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
      }
    }
    .padding(NoemaSpacing.lg)
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    .background(NoemaColor.surface)
    .presentationDetents([.height(210), .medium])
    .presentationDragIndicator(.visible)
    .presentationCornerRadius(NoemaRadius.page)
    .presentationBackground(NoemaColor.surface)
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
          .frame(maxWidth: 300, minHeight: 84, alignment: .leading)
          .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .overlay { RoundedRectangle(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
        }
        .buttonStyle(.plain)
      }
    }
  }
}

private struct MemoryUpdateNotice: View {
  let model: MemoryModel

  var body: some View {
    HStack(alignment: .center, spacing: NoemaSpacing.sm) {
      Text(statusTitle)
        .font(NoemaFont.captionEmphasized)
      if let update = model.update {
        Text(statusDetail(update))
          .font(NoemaFont.caption)
          .foregroundStyle(update.error == nil ? NoemaColor.contentSecondary : NoemaColor.warning)
          .lineLimit(2)
      }
      Spacer(minLength: NoemaSpacing.sm)
      if model.isUpdating {
        ProgressView().controlSize(.small)
      } else {
        Button("Update") { Task { await model.updateMemory() } }
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
          .padding(.horizontal, NoemaSpacing.md)
          .frame(minHeight: 28)
          .background(NoemaColor.paper200, in: Capsule())
          .buttonStyle(.plain)
          .disabled(!model.canUpdate)
      }
    }
    .padding(.horizontal, NoemaSpacing.sm)
    .padding(.vertical, NoemaSpacing.xs)
    .frame(minHeight: 38)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NoemaColor.pine50)
    .overlay { Rectangle().stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
  }

  private var statusTitle: String {
    guard let update = model.update else { return "Memory updates" }
    if update.active || model.isUpdating { return "Updating memory…" }
    if update.error != nil { return "Update needs attention" }
    if update.pendingCount > 0 { return "Not up to date" }
    return "Up to date"
  }

  private func statusDetail(_ update: MemoryUpdateStatus) -> String {
    if let error = update.error, !error.isEmpty { return error }
    if update.pendingCount > 0 {
      let age = update.updatedAt.flatMap(relativeAge)
      return age.map { "\(update.pendingCount) pending · \($0)" } ?? "\(update.pendingCount) pending"
    }
    if model.isOffline { return "Cached data · reconnect to update" }
    if let updatedAt = update.updatedAt { return "Last updated \(updatedAt)" }
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

private struct MemoryCitation: Identifiable {
  let id: String
  let excerpt: String?
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
  static func prepare(body: String, sources: [MemorySource]) -> (content: String, outline: [MemoryOutlineItem], citations: [MemoryCitation]) {
    var definitions: [String: String] = [:]
    let contentLines = body.components(separatedBy: .newlines).filter { line in
      guard line.hasPrefix("[^"), let close = line.firstIndex(of: "]") else { return true }
      let labelStart = line.index(line.startIndex, offsetBy: 2)
      let label = String(line[labelStart..<close])
      let afterClose = line.index(after: close)
      guard afterClose < line.endIndex, line[afterClose] == ":" else { return true }
      let value = String(line[line.index(after: afterClose)...])
        .trimmingCharacters(in: .whitespaces)
        .trimmingCharacters(in: CharacterSet(charactersIn: "`"))
      definitions[label] = value
      return false
    }

    var content = contentLines.joined(separator: "\n")
    var citations: [MemoryCitation] = []
    var numberBySource: [String: Int] = [:]
    let expression = try? NSRegularExpression(pattern: #"\[\^([^\]]+)\]"#)
    let originalRange = NSRange(content.startIndex..<content.endIndex, in: content)
    let orderedLabels = expression?.matches(in: content, range: originalRange).compactMap { match -> String? in
      guard let range = Range(match.range(at: 1), in: content) else { return nil }
      return String(content[range])
    } ?? []
    for label in orderedLabels {
      guard let source = definitions[label] else { continue }
      let token = "[^\(label)]"
      guard content.contains(token), let reference = sources.first(where: { $0.id == source }) else { continue }
      if numberBySource[source] == nil {
        citations.append(MemoryCitation(id: reference.id, excerpt: reference.excerpt))
        numberBySource[source] = citations.count
      }
      let number = numberBySource[source] ?? citations.count
      content = content.replacingOccurrences(
        of: token,
        with: "[\\[\(number)\\]](noema-citation://\(number))"
      )
    }
    if citations.isEmpty {
      citations = sources.map { MemoryCitation(id: $0.id, excerpt: $0.excerpt) }
    }

    let outline = content.components(separatedBy: .newlines).compactMap { line -> MemoryOutlineItem? in
      let hashes = line.prefix { $0 == "#" }
      guard !hashes.isEmpty, hashes.count <= 6 else { return nil }
      let label = String(line.dropFirst(hashes.count)).trimmingCharacters(in: .whitespaces)
      guard !label.isEmpty else { return nil }
      return MemoryOutlineItem(id: memoryHeadingID(label), label: label, level: hashes.count)
    }
    return (content.trimmingCharacters(in: .whitespacesAndNewlines), outline: outline, citations: citations)
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

private func memorySymbol(_ key: String) -> String {
  switch key {
  case "brain": "brain"
  case "file-text": "doc.text"
  case "user": "person"
  case "users": "person.2"
  case "heart": "heart"
  case "house": "house"
  case "briefcase-business": "briefcase"
  case "graduation-cap": "graduationcap"
  case "book-open": "book"
  case "lightbulb": "lightbulb"
  case "target": "target"
  case "calendar-days": "calendar"
  case "map-pin": "mappin"
  case "plane": "airplane"
  case "heart-pulse": "heart.text.square"
  case "dumbbell": "figure.strengthtraining.traditional"
  case "utensils": "fork.knife"
  case "music": "music.note"
  case "palette": "paintpalette"
  case "camera": "camera"
  case "gamepad-2": "gamecontroller"
  case "mountain": "mountain.2"
  case "paw-print": "pawprint"
  case "code-2": "chevron.left.forwardslash.chevron.right"
  case "wallet-cards": "wallet.pass"
  case "sparkles": "sparkles"
  case "compass": "safari"
  case "notebook-pen": "note.text"
  default: "doc.text"
  }
}
