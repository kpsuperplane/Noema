import Foundation
import SwiftUI

struct ProviderCitation: Identifiable, Hashable, Sendable {
  let title: String
  let destination: URL
  let startIndex: Int?
  let endIndex: Int?

  var id: String {
    "\(destination.absoluteString)#\(startIndex.map { String($0) } ?? "")#\(endIndex.map { String($0) } ?? "")"
  }

  init?(title: String, url: String, startIndex: Int?, endIndex: Int?) {
    let title = title.trimmingCharacters(in: .whitespacesAndNewlines)
    let url = url.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !title.isEmpty,
          let destination = URL(string: url),
          destination.scheme == "http" || destination.scheme == "https" else { return nil }
    self.title = title
    self.destination = destination
    self.startIndex = startIndex.flatMap { $0 >= 0 ? $0 : nil }
    self.endIndex = endIndex.flatMap { $0 >= 0 ? $0 : nil }
  }

  static func from(metadata: String?) -> [ProviderCitation] {
    guard let metadata,
          let data = metadata.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let values = object["citations"] as? [[String: Any]] else { return [] }
    return values.compactMap { value in
      guard let title = value["title"] as? String,
            let url = value["url"] as? String else { return nil }
      return ProviderCitation(
        title: title,
        url: url,
        startIndex: nonNegativeInteger(value["start_index"]),
        endIndex: nonNegativeInteger(value["end_index"])
      )
    }
  }

  private static func nonNegativeInteger(_ value: Any?) -> Int? {
    guard let value = value as? Int, value >= 0 else { return nil }
    return value
  }
}

struct ProviderCitationMarkdown: View {
  let text: String
  let citations: [ProviderCitation]
  let role: NoemaMarkdown.Role
  let sourcesInline: Bool
  @State private var sourcesPresented = false

  init(
    text: String,
    citations: [ProviderCitation],
    role: NoemaMarkdown.Role,
    sourcesInline: Bool = false
  ) {
    self.text = text
    self.citations = citations
    self.role = role
    self.sourcesInline = sourcesInline
  }

  @ViewBuilder
  var body: some View {
    let content = ProviderCitationContent(text: text, citations: citations)
    if content.citations.isEmpty {
      NoemaMarkdown(content.text, role: role)
    } else if sourcesInline {
      HStack(alignment: .bottom, spacing: NoemaSpacing.xs) {
        NoemaMarkdown(content.text, role: role)
          .layoutPriority(1)
        CitationSourcesButton {
          sourcesPresented = true
        }
      }
      .noemaSheet(isPresented: $sourcesPresented) {
        ProviderCitationSheet(citations: content.citations)
      }
    } else {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        NoemaMarkdown(content.text, role: role)
        CitationSourcesButton {
          sourcesPresented = true
        }
      }
      .noemaSheet(isPresented: $sourcesPresented) {
        ProviderCitationSheet(citations: content.citations)
      }
    }
  }
}

private struct ProviderCitationContent {
  let text: String
  let citations: [ProviderCitation]

  init(text: String, citations: [ProviderCitation]) {
    var sourceNumberByURL: [String: Int] = [:]
    var uniqueCitations: [ProviderCitation] = []
    var markersByOffset: [Int: Set<Int>] = [:]
    var fallbackMarkers = Set<Int>()

    let orderedCitations = citations.enumerated().sorted { left, right in
      let leftEnd = Self.validEndOffset(text: text, citation: left.element)
      let rightEnd = Self.validEndOffset(text: text, citation: right.element)
      if leftEnd == nil, rightEnd != nil { return false }
      if leftEnd != nil, rightEnd == nil { return true }
      if leftEnd != rightEnd { return (leftEnd ?? 0) < (rightEnd ?? 0) }
      return left.offset < right.offset
    }

    for (_, citation) in orderedCitations {
      let url = citation.destination.absoluteString
      let number: Int
      if let existing = sourceNumberByURL[url] {
        number = existing
      } else {
        number = uniqueCitations.count + 1
        sourceNumberByURL[url] = number
        uniqueCitations.append(citation)
      }

      guard let endIndex = Self.validEndOffset(text: text, citation: citation) else {
        fallbackMarkers.insert(number)
        continue
      }
      markersByOffset[endIndex, default: []].insert(number)
    }

    var citedText = text
    for (offset, numbers) in markersByOffset.sorted(by: { $0.key > $1.key }) {
      guard let utf16Index = citedText.utf16.index(
        citedText.utf16.startIndex,
        offsetBy: offset,
        limitedBy: citedText.utf16.endIndex
      ), let index = String.Index(utf16Index, within: citedText) else {
        fallbackMarkers.formUnion(numbers)
        continue
      }
      citedText.insert(contentsOf: Self.markers(numbers), at: index)
    }
    if !fallbackMarkers.isEmpty {
      let separator = citedText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "" : " "
      citedText.append(separator + Self.markers(fallbackMarkers))
    }

    self.text = citedText
    self.citations = uniqueCitations
  }

  private static func markers(_ numbers: Set<Int>) -> String {
    numbers.sorted().map(citationSuperscript).joined()
  }

  private static func validEndOffset(text: String, citation: ProviderCitation) -> Int? {
    guard let endIndex = citation.endIndex,
          endIndex <= text.utf16.count,
          citation.startIndex.map({ $0 < endIndex }) ?? true else { return nil }
    return endIndex
  }
}

struct CitationSourcesButton: View {
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      NoemaIcon(.bookOpen, size: 14)
        .foregroundStyle(NoemaColor.contentSecondary)
        .frame(width: NoemaSpacing.xxl, height: NoemaSpacing.xxl)
        .background(NoemaColor.controlFill, in: NoemaSuperellipse.full)
        .overlay { NoemaSuperellipse.full.stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .contentShape(.interaction, NoemaSuperellipse.full.inset(by: -NoemaSpacing.md))
    .accessibilityLabel("Sources")
    .accessibilityHint("Shows citation sources")
  }
}

func citationSuperscript(_ number: Int) -> String {
  String(number).map { digit in
    switch digit {
    case "0": "⁰"
    case "1": "¹"
    case "2": "²"
    case "3": "³"
    case "4": "⁴"
    case "5": "⁵"
    case "6": "⁶"
    case "7": "⁷"
    case "8": "⁸"
    case "9": "⁹"
    default: digit
    }
  }.map(String.init).joined()
}

private struct ProviderCitationSheet: View {
  let citations: [ProviderCitation]
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    NoemaNativeSheet(title: "Sources", onDismiss: { dismiss() }) {
      ScrollView {
        VStack(alignment: .leading, spacing: 0) {
          ForEach(Array(citations.enumerated()), id: \.element.destination) { index, citation in
            Link(destination: citation.destination) {
              HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
                Text("\(index + 1)")
                  .font(NoemaFont.metadata.weight(.semibold))
                  .foregroundStyle(NoemaColor.accent)
                  .frame(minWidth: NoemaSpacing.lg, alignment: .leading)
                VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                  Text(citation.title)
                    .font(NoemaFont.body.weight(.semibold))
                    .foregroundStyle(NoemaColor.content)
                    .fixedSize(horizontal: false, vertical: true)
                  Text(citation.destination.host ?? citation.destination.absoluteString)
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                    .lineLimit(2)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
              }
              .padding(.vertical, NoemaSpacing.md)
              .contentShape(Rectangle())
            }
            .accessibilityLabel("Source \(index + 1): \(citation.title)")
            if index + 1 < citations.count {
              Divider()
            }
          }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, NoemaSpacing.lg)
      }
    }
    .presentationDetents([.medium, .large])
    .presentationDragIndicator(.visible)
  }
}
