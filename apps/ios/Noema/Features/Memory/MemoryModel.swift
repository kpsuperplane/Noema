import Apollo
import ApolloAPI
import Foundation
import Observation
import NoemaAPI

struct MemorySource: Identifiable, Hashable {
  let id: String
  let excerpt: String?
}

struct MemoryPageRef: Identifiable, Hashable {
  let id: String
  let path: String
  let title: String
  let icon: String
  let excerpt: String
  let hash: String
}

struct MemoryArticle: Identifiable, Hashable {
  let id: String
  let path: String
  let title: String
  let icon: String
  let body: String
  let hash: String
  let sources: [MemorySource]
  let parent: String?
  let ancestors: [MemoryPageRef]
  let children: [MemoryPageRef]
}

struct MemoryUpdateStatus {
  let state: String
  let active: Bool
  let pendingCount: Int
  let lastConsolidatedSequence: Int
  let lastConsolidatedItem: String?
  let error: String?
  let updatedAt: String?
}

struct MemoryTreeSnapshot {
  let root: MemoryArticle?
  let pages: [MemoryPageRef]
  let update: MemoryUpdateStatus
}

@MainActor
@Observable
final class MemoryModel {
  private(set) var tree: MemoryTreeSnapshot?
  private(set) var article: MemoryArticle?
  var selectedPageID: String?
  private(set) var isLoading = false
  private(set) var isUpdating = false
  private(set) var errorMessage: String?
  private(set) var pageErrorMessage: String?
  private(set) var updateErrorMessage: String?
  private(set) var subscriptionErrorMessage: String?
  private(set) var isOffline = false
  private var client: ApolloClient?
  private var subscriptionTask: Task<Void, Never>?

  var pages: [MemoryPageRef] { tree?.pages ?? [] }
  var update: MemoryUpdateStatus? { tree?.update }
  var updateRetryable: Bool {
    updateErrorMessage != nil || update?.state.lowercased() == "error" || update?.error != nil
  }

  var canUpdate: Bool {
    client != nil && !(update?.active ?? false) && !isUpdating
      && ((update?.pendingCount ?? 0) > 0 || updateRetryable)
  }

  func load(client: ApolloClient?) async {
    guard let client else {
      self.client = nil
      isOffline = true
      errorMessage = "Pair this device with Noema to read memory."
      return
    }
    self.client = client
    isLoading = true
    errorMessage = nil
    do {
      let stream = try client.fetch(query: NoemaAPI.MemoryTreeQuery(), cachePolicy: .cacheAndNetwork)
      var received = false
      for try await response in stream {
        if let data = response.data {
          apply(data.memoryTree)
          received = true
          isOffline = false
        }
        if let firstError = response.errors?.first?.message, tree == nil {
          errorMessage = firstError
          isOffline = true
        }
      }
      if !received {
        isOffline = true
        if tree == nil { errorMessage = "Memory is not available yet." }
      }
    } catch {
      isOffline = true
      if tree == nil { errorMessage = "Memory could not be loaded." }
    }
    isLoading = false
    if selectedPageID == nil { selectedPageID = tree?.root?.id }
    if article == nil { article = tree?.root }
    startSubscription()
  }

  func select(pageID: String) async {
    pageErrorMessage = nil
    if pageID == tree?.root?.id {
      selectedPageID = pageID
      article = tree?.root
      return
    }
    guard let client else { return }
    let query = NoemaAPI.MemoryPageQuery(pageId: pageID)
    if article?.id != pageID {
      do {
        article = try await client.fetch(query: query, cachePolicy: .cacheOnly)?
          .data?.memoryPage.map(Self.article(from:))
      } catch {
        article = nil
      }
    }
    selectedPageID = pageID
    isLoading = true
    defer { isLoading = false }
    do {
      let stream = try client.fetch(
        query: query,
        cachePolicy: .cacheAndNetwork
      )
      var received = false
      for try await response in stream {
        if let data = response.data, let page = data.memoryPage {
          article = Self.article(from: page)
          received = true
          isOffline = false
        }
        if let firstError = response.errors?.first?.message, !received {
          pageErrorMessage = firstError
          isOffline = true
        }
      }
      if !received {
        isOffline = true
        pageErrorMessage = "This memory article is no longer available."
      }
    } catch {
      isOffline = true
      pageErrorMessage = "This memory article could not be loaded."
    }
  }

  func updateMemory() async {
    guard canUpdate, let client else { return }
    isUpdating = true
    updateErrorMessage = nil
    defer { isUpdating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.UpdateMemoryMutation())
      if let error = response.errors?.first?.message { throw MemoryError.server(error) }
      if let status = response.data?.updateMemory.status {
        tree = tree.map {
          MemoryTreeSnapshot(root: $0.root, pages: $0.pages, update: Self.status(from: status, pendingCount: $0.update.pendingCount))
        }
      }
      isOffline = false
    } catch {
      isOffline = true
      updateErrorMessage = error.localizedDescription
    }
  }

  private func startSubscription() {
    subscriptionTask?.cancel()
    guard let client else { return }
    subscriptionTask = Task { [weak self] in
      var attempt = 0
      while !Task.isCancelled {
        do {
          let stream = try client.subscribe(subscription: NoemaAPI.MemoryEventsSubscription())
          for try await response in stream {
            guard let data = response.data else { continue }
            attempt = 0
            self?.subscriptionErrorMessage = nil
            await self?.applySubscription(data.memoryEvents)
          }
          guard !Task.isCancelled else { return }
          self?.setSubscriptionError()
        } catch is CancellationError {
          return
        } catch {
          self?.setSubscriptionError()
        }
        attempt = min(attempt + 1, 6)
        let delay = min(1 << min(attempt - 1, 5), 30)
        try? await Task.sleep(for: .seconds(delay))
      }
    }
  }

  private func applySubscription(_ value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents) async {
    apply(value)
    if let selectedPageID, selectedPageID != tree?.root?.id {
      await select(pageID: selectedPageID)
    }
  }

  private func setSubscriptionError() {
    isOffline = true
    subscriptionErrorMessage = "Update status is reconnecting. The article will refresh when the connection returns."
  }

  private func apply(_ value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree) {
    let root = value.root.map(Self.article(from:))
    let pages = value.pages.map(Self.pageRef(from:))
    let update = Self.status(from: value.updateStatus, pendingCount: value.pendingCount)
    tree = MemoryTreeSnapshot(root: root, pages: pages, update: update)
    isOffline = false
    if selectedPageID == nil { selectedPageID = root?.id }
    if selectedPageID == root?.id { article = root }
  }

  private func apply(_ value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents) {
    let root = value.root.map(Self.article(from:))
    let pages = value.pages.map(Self.pageRef(from:))
    let update = Self.status(from: value.updateStatus, pendingCount: value.pendingCount)
    tree = MemoryTreeSnapshot(root: root, pages: pages, update: update)
    isOffline = false
    if selectedPageID == nil { selectedPageID = root?.id }
    if selectedPageID == root?.id { article = root }
  }

  private static func article(from value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree.Root) -> MemoryArticle {
    MemoryArticle(
      id: value.id,
      path: value.path,
      title: value.title,
      icon: value.icon,
      body: value.body,
      hash: value.hash,
      sources: value.sourceReferences.map { MemorySource(id: $0.source, excerpt: $0.excerpt) },
      parent: value.parent,
      ancestors: value.ancestors.map(Self.pageRef(from:)),
      children: value.children.map(Self.pageRef(from:))
    )
  }

  private static func article(from value: NoemaAPI.MemoryPageQuery.Data.MemoryPage) -> MemoryArticle {
    MemoryArticle(
      id: value.id,
      path: value.path,
      title: value.title,
      icon: value.icon,
      body: value.body,
      hash: value.hash,
      sources: value.sourceReferences.map { MemorySource(id: $0.source, excerpt: $0.excerpt) },
      parent: value.parent,
      ancestors: value.ancestors.map(Self.pageRef(from:)),
      children: value.children.map(Self.pageRef(from:))
    )
  }

  private static func pageRef(from value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree.Page) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: value.icon, excerpt: value.excerpt, hash: value.hash)
  }

  private static func pageRef(from value: NoemaAPI.MemoryPageQuery.Data.MemoryPage.Child) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: value.icon, excerpt: value.excerpt, hash: value.hash)
  }

  private static func pageRef(from value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree.Root.Child) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: value.icon, excerpt: value.excerpt, hash: value.hash)
  }

  private static func pageRef(from value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree.Root.Ancestor) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: "", excerpt: "", hash: "")
  }

  private static func pageRef(from value: NoemaAPI.MemoryPageQuery.Data.MemoryPage.Ancestor) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: "", excerpt: "", hash: "")
  }

  private static func article(from value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents.Root) -> MemoryArticle {
    MemoryArticle(
      id: value.id,
      path: value.path,
      title: value.title,
      icon: value.icon,
      body: value.body,
      hash: value.hash,
      sources: value.sourceReferences.map { MemorySource(id: $0.source, excerpt: $0.excerpt) },
      parent: value.parent,
      ancestors: value.ancestors.map(Self.pageRef(from:)),
      children: value.children.map(Self.pageRef(from:))
    )
  }

  private static func pageRef(from value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents.Page) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: value.icon, excerpt: value.excerpt, hash: value.hash)
  }

  private static func pageRef(from value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents.Root.Child) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: value.icon, excerpt: value.excerpt, hash: value.hash)
  }

  private static func pageRef(from value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents.Root.Ancestor) -> MemoryPageRef {
    MemoryPageRef(id: value.id, path: value.path, title: value.title, icon: "", excerpt: "", hash: "")
  }

  private static func status(
    from value: NoemaAPI.MemoryEventsSubscription.Data.MemoryEvents.UpdateStatus,
    pendingCount: Int
  ) -> MemoryUpdateStatus {
    MemoryUpdateStatus(
      state: value.state,
      active: value.active,
      pendingCount: pendingCount,
      lastConsolidatedSequence: value.lastConsolidatedSequence,
      lastConsolidatedItem: value.lastConsolidatedItem,
      error: value.error,
      updatedAt: value.updatedAt
    )
  }

  private static func status(
    from value: NoemaAPI.MemoryTreeQuery.Data.MemoryTree.UpdateStatus,
    pendingCount: Int
  ) -> MemoryUpdateStatus {
    MemoryUpdateStatus(
      state: value.state,
      active: value.active,
      pendingCount: pendingCount,
      lastConsolidatedSequence: value.lastConsolidatedSequence,
      lastConsolidatedItem: value.lastConsolidatedItem,
      error: value.error,
      updatedAt: value.updatedAt
    )
  }

  private static func status(
    from value: NoemaAPI.UpdateMemoryMutation.Data.UpdateMemory.Status,
    pendingCount: Int
  ) -> MemoryUpdateStatus {
    MemoryUpdateStatus(
      state: value.state,
      active: value.active,
      pendingCount: pendingCount,
      lastConsolidatedSequence: value.lastConsolidatedSequence,
      lastConsolidatedItem: value.lastConsolidatedItem,
      error: value.error,
      updatedAt: value.updatedAt
    )
  }
}

private enum MemoryError: LocalizedError {
  case server(String)

  var errorDescription: String? {
    switch self {
    case .server(let message): message
    }
  }
}
