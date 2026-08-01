import SwiftUI
import VisionKit

struct QRScannerSheet: UIViewControllerRepresentable {
  let onCode: (String) -> Void

  static var isAvailable: Bool {
    DataScannerViewController.isSupported && DataScannerViewController.isAvailable
  }

  func makeCoordinator() -> Coordinator {
    Coordinator(onCode: onCode)
  }

  func makeUIViewController(context: Context) -> UIViewController {
    guard Self.isAvailable else {
      return unavailableController()
    }
    let scanner = DataScannerViewController(
      recognizedDataTypes: [.barcode(symbologies: [.qr])],
      qualityLevel: .balanced,
      recognizesMultipleItems: false,
      isHighFrameRateTrackingEnabled: false,
      isPinchToZoomEnabled: true,
      isGuidanceEnabled: true,
      isHighlightingEnabled: true
    )
    scanner.delegate = context.coordinator
    context.coordinator.scanner = scanner
    scanner.view.backgroundColor = .systemBackground
    return scanner
  }

  func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
    guard let scanner = context.coordinator.scanner else { return }
    if !scanner.isScanning {
      do { try scanner.startScanning() } catch { context.coordinator.error = error }
    }
  }

  static func dismantleUIViewController(_ uiViewController: UIViewController, coordinator: Coordinator) {
    coordinator.scanner?.stopScanning()
  }

  private func unavailableController() -> UIViewController {
    let controller = UIHostingController(rootView: ContentUnavailableView(
      "Camera unavailable",
      systemImage: "camera.slash",
      description: Text("This device cannot scan QR codes. Paste the pairing link instead.")
    ))
    controller.view.backgroundColor = .systemBackground
    return controller
  }

  final class Coordinator: NSObject, DataScannerViewControllerDelegate {
    let onCode: (String) -> Void
    weak var scanner: DataScannerViewController?
    var error: Error?

    init(onCode: @escaping (String) -> Void) {
      self.onCode = onCode
    }

    func dataScanner(_ dataScanner: DataScannerViewController, didTapOn item: RecognizedItem) {
      guard case .barcode(let barcode) = item, let value = barcode.payloadStringValue else { return }
      onCode(value)
    }

    func dataScanner(_ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
      guard let barcode = addedItems.compactMap({ item -> String? in
        guard case .barcode(let barcode) = item else { return nil }
        return barcode.payloadStringValue
      }).first else { return }
      onCode(barcode)
    }
  }
}

