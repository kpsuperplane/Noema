import SwiftUI
import UIKit

struct TasksDetailPager<WorkspaceContent: View, TranscriptContent: View>: UIViewControllerRepresentable {
  @Binding var selection: TaskDetailTab
  let tabs: [TaskDetailTab]
  let workspaceContent: WorkspaceContent
  let transcriptContent: TranscriptContent

  init(
    selection: Binding<TaskDetailTab>,
    tabs: [TaskDetailTab],
    @ViewBuilder workspace: () -> WorkspaceContent,
    @ViewBuilder transcript: () -> TranscriptContent
  ) {
    _selection = selection
    self.tabs = tabs
    workspaceContent = workspace()
    transcriptContent = transcript()
  }

  func makeCoordinator() -> Coordinator {
    Coordinator(parent: self)
  }

  func makeUIViewController(context: Context) -> UIPageViewController {
    let controller = UIPageViewController(
      transitionStyle: .scroll,
      navigationOrientation: .horizontal
    )
    context.coordinator.install(on: controller)
    return controller
  }

  func updateUIViewController(_ controller: UIPageViewController, context: Context) {
    context.coordinator.update(parent: self, on: controller, animated: context.transaction.animation != nil)
  }

  final class Coordinator: NSObject, UIPageViewControllerDataSource, UIPageViewControllerDelegate {
    private var parent: TasksDetailPager
    private let workspaceController: UIHostingController<WorkspaceContent>
    private let transcriptController: UIHostingController<TranscriptContent>
    private var isTransitioning = false

    init(parent: TasksDetailPager) {
      self.parent = parent
      workspaceController = UIHostingController(rootView: parent.workspaceContent)
      transcriptController = UIHostingController(rootView: parent.transcriptContent)
      super.init()
      workspaceController.view.backgroundColor = .clear
      transcriptController.view.backgroundColor = .clear
    }

    func install(on pageController: UIPageViewController) {
      pageController.dataSource = self
      pageController.delegate = self
      pageController.setViewControllers([controller(for: parent.selection)], direction: .forward, animated: false)
    }

    func update(parent: TasksDetailPager, on pageController: UIPageViewController, animated: Bool) {
      self.parent = parent
      guard !isTransitioning else { return }

      updateHostedContent()
      guard currentTab(in: pageController) != parent.selection else { return }

      let oldIndex = currentTab(in: pageController).flatMap { parent.tabs.firstIndex(of: $0) } ?? 0
      let newIndex = parent.tabs.firstIndex(of: parent.selection) ?? 0
      let direction: UIPageViewController.NavigationDirection = newIndex < oldIndex ? .reverse : .forward
      isTransitioning = true
      pageController.setViewControllers([controller(for: parent.selection)], direction: direction, animated: animated) { [weak self] _ in
        guard let self else { return }
        isTransitioning = false
        update(parent: parent, on: pageController, animated: false)
      }
    }

    func pageViewController(
      _: UIPageViewController,
      viewControllerBefore viewController: UIViewController
    ) -> UIViewController? {
      adjacentController(before: viewController)
    }

    func pageViewController(
      _: UIPageViewController,
      viewControllerAfter viewController: UIViewController
    ) -> UIViewController? {
      adjacentController(after: viewController)
    }

    func pageViewController(
      _: UIPageViewController,
      willTransitionTo _: [UIViewController]
    ) {
      isTransitioning = true
    }

    func pageViewController(
      _ pageController: UIPageViewController,
      didFinishAnimating _: Bool,
      previousViewControllers _: [UIViewController],
      transitionCompleted completed: Bool
    ) {
      isTransitioning = false
      if completed, let tab = currentTab(in: pageController) {
        parent.selection = tab
      }
      update(parent: parent, on: pageController, animated: false)
    }

    private func controller(for tab: TaskDetailTab) -> UIViewController {
      switch tab {
      case .workspace: workspaceController
      case .transcript: transcriptController
      }
    }

    private func currentTab(in pageController: UIPageViewController) -> TaskDetailTab? {
      guard let current = pageController.viewControllers?.first else { return nil }
      if current === workspaceController { return .workspace }
      return .transcript
    }

    private func adjacentController(before viewController: UIViewController) -> UIViewController? {
      guard let tab = tab(for: viewController), let index = parent.tabs.firstIndex(of: tab), index > 0 else {
        return nil
      }
      return controller(for: parent.tabs[index - 1])
    }

    private func adjacentController(after viewController: UIViewController) -> UIViewController? {
      guard let tab = tab(for: viewController), let index = parent.tabs.firstIndex(of: tab), index + 1 < parent.tabs.count else {
        return nil
      }
      return controller(for: parent.tabs[index + 1])
    }

    private func tab(for viewController: UIViewController) -> TaskDetailTab? {
      if viewController === workspaceController { return .workspace }
      if viewController === transcriptController { return .transcript }
      return nil
    }

    private func updateHostedContent() {
      workspaceController.rootView = parent.workspaceContent
      transcriptController.rootView = parent.transcriptContent
    }
  }
}
