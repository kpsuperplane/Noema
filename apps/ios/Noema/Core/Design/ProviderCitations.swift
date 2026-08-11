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

struct ProviderCitationLinks: View {
  let citations: [ProviderCitation]

  var body: some View {
    if !uniqueCitations.isEmpty {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Sources")
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(NoemaColor.contentTertiary)
        ForEach(Array(uniqueCitations.enumerated()), id: \.element.id) { index, citation in
          Link(destination: citation.destination) {
            HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.xs) {
              Text("\(index + 1)")
                .font(NoemaFont.metadata.weight(.semibold))
                .foregroundStyle(NoemaColor.accent)
              Text(citation.title)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                .lineLimit(2)
            }
          }
          .accessibilityLabel("Source \(index + 1): \(citation.title)")
        }
      }
      .accessibilityElement(children: .contain)
      .accessibilityLabel("Sources")
    }
  }

  private var uniqueCitations: [ProviderCitation] {
    var seen = Set<String>()
    return citations.filter { seen.insert($0.destination.absoluteString).inserted }
  }
}
