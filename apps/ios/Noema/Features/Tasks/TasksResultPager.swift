import SwiftUI
import UIKit

struct TasksResultPager<ResultContent: View, TranscriptContent: View>: UIViewControllerRepresentable {
  @Binding var selection: TaskResultTab
  let resultContent: ResultContent
  let transcriptContent: TranscriptContent

  init(
    selection: Binding<TaskResultTab>,
    @ViewBuilder result: () -> ResultContent,
    @ViewBuilder transcript: () -> TranscriptContent
  ) {
    _selection = selection
    resultContent = result()
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
    private var parent: TasksResultPager
    private let resultController: UIHostingController<ResultContent>
    private let transcriptController: UIHostingController<TranscriptContent>
    private var isTransitioning = false

    init(parent: TasksResultPager) {
      self.parent = parent
      resultController = UIHostingController(rootView: parent.resultContent)
      transcriptController = UIHostingController(rootView: parent.transcriptContent)
      super.init()
      resultController.view.backgroundColor = .clear
      transcriptController.view.backgroundColor = .clear
    }

    func install(on pageController: UIPageViewController) {
      pageController.dataSource = self
      pageController.delegate = self
      pageController.setViewControllers([controller(for: parent.selection)], direction: .forward, animated: false)
    }

    func update(parent: TasksResultPager, on pageController: UIPageViewController, animated: Bool) {
      self.parent = parent
      guard !isTransitioning else { return }

      updateHostedContent()
      guard currentTab(in: pageController) != parent.selection else { return }

      let direction: UIPageViewController.NavigationDirection = parent.selection == .result ? .reverse : .forward
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
      viewController === transcriptController ? resultController : nil
    }

    func pageViewController(
      _: UIPageViewController,
      viewControllerAfter viewController: UIViewController
    ) -> UIViewController? {
      viewController === resultController ? transcriptController : nil
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

    private func controller(for tab: TaskResultTab) -> UIViewController {
      switch tab {
      case .result: resultController
      case .transcript: transcriptController
      }
    }

    private func currentTab(in pageController: UIPageViewController) -> TaskResultTab? {
      guard let current = pageController.viewControllers?.first else { return nil }
      return current === resultController ? .result : .transcript
    }

    private func updateHostedContent() {
      resultController.rootView = parent.resultContent
      transcriptController.rootView = parent.transcriptContent
    }
  }
}
