import SwiftUI

struct PhaseStyle {
  let state: NoemaTasksActivityAttributes.ContentState

  func color(for appearance: ActivityAppearance, colorScheme: ColorScheme = .light) -> Color {
    if state.requiresAttention == true || state.phase == .cancelled {
      return appearance == .island
        ? NoemaActivityPalette.islandClay
        : NoemaActivityPalette.lockClay(for: colorScheme)
    }
    if state.phase == .reviewing {
      return appearance == .island
        ? NoemaActivityPalette.islandBlue
        : NoemaActivityPalette.lockBlue(for: colorScheme)
    }
    if state.phase == .completed {
      return appearance == .island
        ? NoemaActivityPalette.islandPine
        : NoemaActivityPalette.lockPine(for: colorScheme)
    }
    return appearance == .island
      ? NoemaActivityPalette.islandAmber
      : NoemaActivityPalette.lockAmber(for: colorScheme)
  }
}

enum ActivityAppearance {
  case lockScreen
  case island

  var primary: Color {
    self == .island ? NoemaActivityPalette.islandPrimary : .primary
  }

  var secondary: Color {
    self == .island ? NoemaActivityPalette.islandSecondary : .secondary
  }

  var tertiary: Color {
    self == .island ? NoemaActivityPalette.islandTertiary : .secondary.opacity(0.82)
  }

  var divider: Color {
    self == .island ? Color.white.opacity(0.09) : Color.secondary.opacity(0.18)
  }

  func surface(for colorScheme: ColorScheme) -> Color {
    if self == .island { return .black }
    return colorScheme == .dark
      ? NoemaActivityPalette.lockSurface
      : Color.white
  }

  func stationBorder(for colorScheme: ColorScheme) -> Color {
    self == .island || colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.07)
  }

  func stationShadow(for colorScheme: ColorScheme) -> Color {
    self == .island || colorScheme == .dark ? Color.black.opacity(0.22) : Color.black.opacity(0.10)
  }
}

enum NoemaActivityPalette {
  static let lockSurface = Color(red: 0.078, green: 0.082, blue: 0.080)
  static let islandPrimary = Color.white
  static let islandSecondary = Color(red: 0.710, green: 0.741, blue: 0.725)
  static let islandTertiary = Color(red: 0.573, green: 0.612, blue: 0.592)
  static let islandPine = Color(red: 0.561, green: 0.706, blue: 0.643)
  static let islandAmber = Color(red: 0.851, green: 0.663, blue: 0.408)
  static let islandClay = Color(red: 0.941, green: 0.604, blue: 0.475)
  static let islandBlue = Color(red: 0.663, green: 0.745, blue: 0.831)
  static let done = Color(red: 0.722, green: 0.831, blue: 0.776)
  static let doneRing = Color(red: 0.443, green: 0.592, blue: 0.525)
  static let doneInk = Color(red: 0.082, green: 0.141, blue: 0.114)

  static func lockPine(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandPine : Color(red: 0.184, green: 0.435, blue: 0.329)
  }

  static func lockAmber(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandAmber : Color(red: 0.604, green: 0.412, blue: 0.157)
  }

  static func lockClay(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandClay : Color(red: 0.655, green: 0.267, blue: 0.153)
  }

  static func lockBlue(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandBlue : Color(red: 0.192, green: 0.365, blue: 0.506)
  }

  static func track(for appearance: ActivityAppearance, colorScheme: ColorScheme) -> Color {
    appearance == .island || colorScheme == .dark
      ? Color(red: 0.400, green: 0.443, blue: 0.420)
      : Color(red: 0.667, green: 0.714, blue: 0.686)
  }

  static func marker(for appearance: ActivityAppearance, colorScheme: ColorScheme) -> Color {
    appearance == .island || colorScheme == .dark
      ? Color(red: 0.467, green: 0.514, blue: 0.490)
      : Color(red: 0.482, green: 0.533, blue: 0.506)
  }

  static func station(for appearance: ActivityAppearance, colorScheme: ColorScheme) -> Color {
    appearance == .island || colorScheme == .dark
      ? Color(red: 0.141, green: 0.169, blue: 0.157)
      : Color(red: 0.914, green: 0.933, blue: 0.914)
  }

  static func doneStation(for appearance: ActivityAppearance, colorScheme: ColorScheme) -> Color {
    appearance == .island || colorScheme == .dark
      ? Color(red: 0.125, green: 0.204, blue: 0.169)
      : Color(red: 0.863, green: 0.914, blue: 0.886)
  }

  static func attention(for appearance: ActivityAppearance) -> Color {
    appearance == .island ? islandClay : Color(red: 0.655, green: 0.267, blue: 0.153)
  }

  static func doneText(for appearance: ActivityAppearance) -> Color {
    appearance == .island
      ? Color(red: 0.675, green: 0.816, blue: 0.749)
      : Color(red: 0.255, green: 0.431, blue: 0.353)
  }
}
