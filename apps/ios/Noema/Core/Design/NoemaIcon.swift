import SwiftUI

struct NoemaIcon: View {
  let name: Name
  let size: CGFloat

  init(_ name: Name, size: CGFloat = 18) {
    self.name = name
    self.size = size
  }

  var body: some View {
    Image("lucide-\(name.rawValue)")
      .resizable()
      .renderingMode(.template)
      .scaledToFit()
      .frame(width: size, height: size)
      .accessibilityHidden(true)
  }
}
