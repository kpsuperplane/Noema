import Foundation
import SwiftUI

private enum NativeJSON: Equatable {
  case object([String: NativeJSON])
  case array([NativeJSON])
  case string(String)
  case number(Double)
  case bool(Bool)
  case null

  init?(_ value: Any) {
    switch value {
    case let value as [String: Any]: self = .object(value.compactMapValues(Self.init))
    case let value as [Any]: self = .array(value.compactMap(Self.init))
    case let value as String: self = .string(value)
    case let value as NSNumber:
      if CFGetTypeID(value) == CFBooleanGetTypeID() { self = .bool(value.boolValue) }
      else { self = .number(value.doubleValue) }
    case _ as NSNull: self = .null
    default: return nil
    }
  }

  var any: Any {
    switch self {
    case let .object(value): value.mapValues(\.any)
    case let .array(value): value.map(\.any)
    case let .string(value): value
    case let .number(value): value
    case let .bool(value): value
    case .null: NSNull()
    }
  }

  var stringValue: String {
    switch self {
    case let .string(value): value
    case let .number(value): String(value)
    case let .bool(value): String(value)
    default: ""
    }
  }

  var boolValue: Bool {
    if case let .bool(value) = self { return value }
    return false
  }

  var arrayValue: [NativeJSON] {
    if case let .array(value) = self { return value }
    return []
  }
}

private struct A2UIAction: Equatable {
  let sourceComponentID: String
  let name: String
  let context: NativeJSON?
}

private struct A2UISnapshotModel {
  let sendDataModel: Bool
  let components: [String: [String: NativeJSON]]
  let dataModel: NativeJSON
  let actions: [A2UIAction]

  init?(surface: A2UISurfaceModel) {
    let allowedComponents = Set(["Text", "Row", "Column", "Card", "Divider", "Button", "TextField", "CheckBox", "ChoicePicker"])
    guard surface.version == "v0.9.1",
          let catalog = Self.parse(surface.catalogJSON),
          case let .object(catalogObject) = catalog,
          catalogObject["catalog_id"]?.stringValue == "com.noema.a2ui/catalog/v0.9.1",
          catalogObject["protocol_version"]?.stringValue == "v0.9.1",
          Set(catalogObject["components"]?.arrayValue.map(\.stringValue) ?? []) == allowedComponents else { return nil }
    guard let snapshot = Self.parse(surface.snapshotJSON),
          case let .object(root) = snapshot,
          case let .object(components) = root["components"],
          case let .array(actions) = root["actions"],
          let dataModel = root["data_model"],
          components["root"] != nil else { return nil }
    let converted = components.compactMapValues { value -> [String: NativeJSON]? in
      guard case let .object(component) = value, Self.validComponent(value) else { return nil }
      return component
    }
    guard converted.count == components.count else { return nil }
    self.components = converted
    self.sendDataModel = root["send_data_model"]?.boolValue ?? false
    self.dataModel = dataModel
    self.actions = actions.compactMap { action in
      guard case let .object(value) = action,
            let source = value["source_component_id"]?.stringValue,
            let name = value["name"]?.stringValue else { return nil }
      return A2UIAction(sourceComponentID: source, name: name, context: value["context"])
    }
  }

  private static func parse(_ string: String) -> NativeJSON? {
    guard let data = string.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data),
          let value = NativeJSON(object) else { return nil }
    return value
  }

  private static func validComponent(_ value: NativeJSON) -> Bool {
    guard case let .object(component) = value,
          let kind = component["component"]?.stringValue else { return false }
    return ["Text", "Row", "Column", "Card", "Divider", "Button", "TextField", "CheckBox", "ChoicePicker"].contains(kind)
  }
}

struct A2UISurfaceView: View {
  let surface: A2UISurfaceModel
  let disabled: Bool
  let onSubmit: (String, String, Any?, Any?) -> Void
  @State private var dataModel: NativeJSON
  @State private var localValues: [String: NativeJSON]

  init(surface: A2UISurfaceModel, disabled: Bool, onSubmit: @escaping (String, String, Any?, Any?) -> Void) {
    self.surface = surface
    self.disabled = disabled
    self.onSubmit = onSubmit
    _dataModel = State(initialValue: A2UISnapshotModel(surface: surface)?.dataModel ?? .object([:]))
    _localValues = State(initialValue: [:])
  }

  var body: some View {
    if let snapshot = A2UISnapshotModel(surface: surface) {
      A2UIContent(surface: surface, snapshot: snapshot, disabled: disabled, dataModel: $dataModel, localValues: $localValues, onSubmit: onSubmit)
        .task(id: revisionIdentity) {
          dataModel = snapshot.dataModel
          localValues = [:]
        }
    } else {
      Text("This interactive surface could not be displayed.")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  private var revisionIdentity: String {
    [
      surface.interactionID ?? "none",
      surface.surfaceID,
      String(surface.revision),
      String(surface.interactionRevision ?? -1),
      surface.lifecycle
    ].joined(separator: ":")
  }
}

private struct A2UIContent: View {
  let surface: A2UISurfaceModel
  let snapshot: A2UISnapshotModel
  let disabled: Bool
  @Binding var dataModel: NativeJSON
  @Binding var localValues: [String: NativeJSON]
  let onSubmit: (String, String, Any?, Any?) -> Void

  private var interactive: Bool {
    !disabled && surface.lifecycle == "pending" && surface.hasActions && surface.interactionID != nil && surface.interactionRevision != nil
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      render("root", ancestors: [])
      if surface.lifecycle != "pending" {
        Text(lifecycleLabel(surface.lifecycle))
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
    }
  }

  private func render(_ id: String, ancestors: Set<String>) -> AnyView {
    if ancestors.contains(id) {
      return AnyView(EmptyView())
    } else if let component = snapshot.components[id] {
      let next = ancestors.union([id])
      switch component["component"]?.stringValue {
      case "Text":
        let variant = component["variant"]?.stringValue ?? "body"
        return AnyView(Text(resolve(component["text"]).stringValue)
          .font(a2uiTextFont(variant))
          .frame(minHeight: 20, alignment: .leading)
          .foregroundStyle(NoemaColor.content))
      case "Row":
        return AnyView(A2UIFlowLayout(
          justify: component["justify"]?.stringValue ?? "start",
          align: component["align"]?.stringValue ?? "center"
        ) { renderChildren(component, ancestors: next) })
      case "Column":
        return AnyView(A2UIColumnLayout(
          justify: component["justify"]?.stringValue ?? "start",
          align: component["align"]?.stringValue ?? "start"
        ) {
          renderChildren(component, ancestors: next)
        })
      case "Card":
        return AnyView(NoemaCard(padding: NoemaSpacing.md) {
          render(component["child"]?.stringValue ?? "", ancestors: next)
        })
      case "Divider":
        if component["axis"]?.stringValue == "vertical" {
          return AnyView(Rectangle().fill(NoemaColor.separatorSubtle).frame(width: 1, height: 24))
        }
        return AnyView(NoemaDivider())
      case "Button":
        return AnyView(button(component: component, id: id))
      case "TextField":
        return AnyView(textField(component: component, id: id))
      case "CheckBox":
        return AnyView(checkBox(component: component, id: id))
      case "ChoicePicker":
        return AnyView(choicePicker(component: component, id: id))
      default:
        return AnyView(EmptyView())
      }
    }
    return AnyView(EmptyView())
  }

  @ViewBuilder
  private func renderChildren(_ component: [String: NativeJSON], ancestors: Set<String>) -> some View {
    if case let .array(children) = component["children"] {
      ForEach(Array(children.enumerated()), id: \.offset) { _, child in
        render(child.stringValue, ancestors: ancestors)
      }
    }
  }

  @ViewBuilder
  private func button(component: [String: NativeJSON], id: String) -> some View {
    let child = component["child"]?.stringValue ?? ""
    let label = snapshot.components[child].map { resolve($0["text"]).stringValue }.flatMap { $0.isEmpty ? nil : $0 } ?? "Continue"
    let actionName = action(for: id)?.name
    let variant = component["variant"]?.stringValue ?? "default"
    let button = Button(label) {
      guard let actionName else { return }
      submit(componentID: id, actionName: actionName)
    }
    if variant == "primary" {
      button
        .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        .disabled(!interactive || actionName == nil)
    } else if variant == "borderless" {
      button
        .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
        .disabled(!interactive || actionName == nil)
    } else {
      button
        .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
        .disabled(!interactive || actionName == nil)
    }
  }

  private func textField(component: [String: NativeJSON], id: String) -> some View {
    let binding = Binding<String>(
      get: { value(for: id, dynamic: component["value"]).stringValue },
      set: { update(id: id, dynamic: component["value"], value: .string($0)) }
    )
    let label = resolve(component["label"]).stringValue
    return VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      if !label.isEmpty {
        Text(label)
          .font(NoemaFont.navigation)
          .frame(minHeight: 21, alignment: .leading)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      if component["variant"]?.stringValue == "obscured" {
        SecureField("", text: binding)
          .noemaTextField()
          .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
      } else {
        TextField("", text: binding, axis: component["variant"]?.stringValue == "longText" ? .vertical : .horizontal)
          .noemaTextField()
          .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
      }
    }
  }

  private func checkBox(component: [String: NativeJSON], id: String) -> some View {
    let binding = Binding<Bool>(
      get: { value(for: id, dynamic: component["value"]).boolValue },
      set: { update(id: id, dynamic: component["value"], value: .bool($0)) }
    )
    return Toggle(resolve(component["label"]).stringValue, isOn: binding)
      .toggleStyle(NoemaCheckboxToggleStyle())
      .frame(minHeight: 21)
      .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
  }

  private func choicePicker(component: [String: NativeJSON], id: String) -> some View {
    let options = choiceOptions(component["options"])
    let label = resolve(component["label"] ?? .string("Choose an option")).stringValue
    let multiple = component["variant"]?.stringValue == "multipleSelection"
    let selected = value(for: id, dynamic: component["value"]).arrayValue.map(\.stringValue)
    return AnyView(VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.navigation)
        .frame(minHeight: 21, alignment: .leading)
        .foregroundStyle(NoemaColor.contentSecondary)
      ForEach(options, id: \.0) { option in
        let isSelected = selected.contains(option.0)
        Button {
          guard interactive, snapshot.sendDataModel, bindingPath(component["value"]) != nil else { return }
          let next: [String]
          if multiple {
            next = isSelected ? selected.filter { $0 != option.0 } : selected + [option.0]
          } else {
            next = [option.0]
          }
          update(id: id, dynamic: component["value"], value: .array(next.map(NativeJSON.string)))
        } label: {
          HStack(spacing: NoemaSpacing.sm) {
            choiceIndicator(selected: isSelected, multiple: multiple)
            Text(option.1)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.content)
              .multilineTextAlignment(.leading)
            Spacer(minLength: 0)
          }
          .padding(.horizontal, NoemaSpacing.sm)
          .padding(.vertical, NoemaSpacing.compact)
          .frame(maxWidth: .infinity, minHeight: 32, alignment: .leading)
        }
        .buttonStyle(.plain)
        .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
      }
    })
  }

  @ViewBuilder
  private func choiceIndicator(selected: Bool, multiple: Bool) -> some View {
    if multiple {
      NoemaSuperellipse(cornerRadius: NoemaRadius.inner)
        .fill(selected ? NoemaColor.clay600 : NoemaColor.surface)
        .overlay {
          NoemaSuperellipse(cornerRadius: NoemaRadius.inner)
            .stroke(selected ? NoemaColor.clay600 : NoemaColor.content.opacity(0.24), lineWidth: 1)
        }
        .overlay {
          if selected {
            Image(systemName: "checkmark")
              .font(.system(size: 10, weight: .bold))
              .foregroundStyle(NoemaColor.white)
          }
        }
        .frame(width: 18, height: 18)
    } else {
      Circle()
        .fill(selected ? NoemaColor.clay600 : NoemaColor.surface)
        .overlay { Circle().stroke(selected ? NoemaColor.clay600 : NoemaColor.content.opacity(0.24), lineWidth: 1) }
        .overlay { if selected { Circle().fill(NoemaColor.white).padding(NoemaSpacing.xs) } }
        .frame(width: 18, height: 18)
    }
  }

  private func action(for componentID: String) -> A2UIAction? {
    snapshot.actions.first { $0.sourceComponentID == componentID }
  }

  private func submit(componentID: String, actionName: String) {
    guard interactive, let action = snapshot.actions.first(where: { $0.sourceComponentID == componentID && $0.name == actionName }) else { return }
    let context = action.context.map { resolveBindings($0).any }
    onSubmit(componentID, actionName, context, snapshot.sendDataModel ? dataModel.any : nil)
  }

  private func value(for id: String, dynamic: NativeJSON?) -> NativeJSON {
    if let local = localValues[id] { return local }
    return dynamic.map(resolve) ?? .null
  }

  private func update(id: String, dynamic: NativeJSON?, value: NativeJSON) {
    guard let path = bindingPath(dynamic) else { localValues[id] = value; return }
    dataModel = setPointer(dataModel, path: path, value: value)
  }

  private func resolve(_ value: NativeJSON?) -> NativeJSON {
    guard let value else { return .null }
    guard let path = bindingPath(value) else { return value }
    return getPointer(dataModel, path: path) ?? .null
  }

  private func resolveBindings(_ value: NativeJSON) -> NativeJSON {
    if bindingPath(value) != nil { return resolve(value) }
    switch value {
    case let .array(values): return .array(values.map(resolveBindings))
    case let .object(values): return .object(values.mapValues(resolveBindings))
    default: return value
    }
  }

  private func choiceOptions(_ value: NativeJSON?) -> [(String, String)] {
    guard case let .array(options) = value else { return [] }
    return options.compactMap { option in
      guard case let .object(values) = option,
            let id = values["value"]?.stringValue else { return nil }
      return (id, resolve(values["label"]).stringValue)
    }
  }
}

private func a2uiTextFont(_ variant: String) -> Font {
  switch variant {
  case "h1":
    return NoemaFont.pageTitle
  case "h2":
    return NoemaFont.title
  case "h3", "h4", "h5":
    return NoemaFont.bodyEmphasized
  case "caption", "supporting":
    return NoemaFont.caption
  default:
    return NoemaFont.body
  }
}

private func lifecycleLabel(_ lifecycle: String) -> String {
  if lifecycle == "completed" || lifecycle == "answered" { return "Submitted" }
  if lifecycle == "failed" { return "Submission failed" }
  return "No longer available"
}

private struct A2UIFlowLayout: Layout {
  let justify: String
  let align: String
  private let spacing = NoemaSpacing.sm

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let rows = makeRows(subviews: subviews, maxWidth: proposal.width ?? .greatestFiniteMagnitude)
    let contentWidth = rows.map(\.width).max() ?? 0
    let width = justify == "start" ? contentWidth : proposal.width ?? contentWidth
    return CGSize(width: width, height: rows.map(\.height).reduce(0, +) + spacing * CGFloat(max(0, rows.count - 1)))
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    let rows = makeRows(subviews: subviews, maxWidth: bounds.width)
    var y = bounds.minY
    for row in rows {
      let available = max(0, bounds.width - row.width)
      let distribution = stackDistribution(justify, available: available, count: row.indices.count)
      let leadingOffset: CGFloat = switch justify {
      case "center": available / 2
      case "end": available
      default: distribution.leading
      }
      var x = bounds.minX + leadingOffset
      for (offset, index) in row.indices.enumerated() {
        let size = row.sizes[offset]
        let crossOffset = switch align {
        case "start": CGFloat.zero
        case "end": row.height - size.height
        case "stretch": CGFloat.zero
        default: (row.height - size.height) / 2
        }
        let placedSize = align == "stretch" ? CGSize(width: size.width, height: row.height) : size
        subviews[index].place(at: CGPoint(x: x, y: y + crossOffset), proposal: ProposedViewSize(placedSize))
        x += size.width + (offset == row.indices.count - 1 ? 0 : spacing + distribution.gap)
      }
      y += row.height + spacing
    }
  }

  private func makeRows(subviews: Subviews, maxWidth: CGFloat) -> [FlowRow] {
    var rows: [FlowRow] = []
    var current = FlowRow()
    for index in subviews.indices {
      let size = subviews[index].sizeThatFits(.unspecified)
      let proposedWidth = current.indices.isEmpty ? size.width : current.width + spacing + size.width
      if !current.indices.isEmpty && proposedWidth > maxWidth {
        rows.append(current)
        current = FlowRow()
      }
      if !current.indices.isEmpty { current.width += spacing }
      current.indices.append(index)
      current.sizes.append(size)
      current.width += size.width
      current.height = max(current.height, size.height)
    }
    if !current.indices.isEmpty { rows.append(current) }
    return rows
  }

  private struct FlowRow {
    var indices: [Int] = []
    var sizes: [CGSize] = []
    var width: CGFloat = 0
    var height: CGFloat = 0
  }
}

private struct A2UIColumnLayout: Layout {
  let justify: String
  let align: String
  private let spacing = NoemaSpacing.sm

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let sizes = subviews.map { $0.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil)) }
    let contentWidth = sizes.map(\.width).max() ?? 0
    let contentHeight = sizes.map(\.height).reduce(0, +) + spacing * CGFloat(max(0, sizes.count - 1))
    let width = align == "stretch" ? proposal.width ?? contentWidth : contentWidth
    let height = justify == "start" ? contentHeight : proposal.height ?? contentHeight
    return CGSize(width: width, height: height)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    let sizes = subviews.map { $0.sizeThatFits(ProposedViewSize(width: bounds.width, height: nil)) }
    let contentHeight = sizes.map(\.height).reduce(0, +) + spacing * CGFloat(max(0, sizes.count - 1))
    let available = max(0, bounds.height - contentHeight)
    let distribution = stackDistribution(justify, available: available, count: subviews.count)
    let leadingOffset: CGFloat = switch justify {
    case "center": available / 2
    case "end": available
    default: distribution.leading
    }
    var y = bounds.minY + leadingOffset
    for (offset, index) in subviews.indices.enumerated() {
      let size = sizes[offset]
      let x: CGFloat = switch align {
      case "center": bounds.minX + (bounds.width - size.width) / 2
      case "end": bounds.maxX - size.width
      default: bounds.minX
      }
      let placedSize = align == "stretch" ? CGSize(width: bounds.width, height: size.height) : size
      subviews[index].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(placedSize))
      y += size.height + (offset == sizes.count - 1 ? 0 : spacing + distribution.gap)
    }
  }
}

private func stackDistribution(_ justify: String, available: CGFloat, count: Int) -> (leading: CGFloat, gap: CGFloat) {
  guard count > 0 else { return (0, 0) }
  switch justify {
  case "spaceBetween" where count > 1:
    return (0, available / CGFloat(count - 1))
  case "spaceAround":
    let gap = available / CGFloat(count)
    return (gap / 2, gap)
  case "spaceEvenly":
    let gap = available / CGFloat(count + 1)
    return (gap, gap)
  default:
    return (0, 0)
  }
}

private func bindingPath(_ value: NativeJSON?) -> String? {
  guard case let .object(object) = value, object.count == 1, let path = object["path"]?.stringValue else { return nil }
  return path
}

private func getPointer(_ value: NativeJSON, path: String) -> NativeJSON? {
  guard path != "" && path != "/" else { return value }
  var current = value
  for segment in path.split(separator: "/", omittingEmptySubsequences: true).map({ $0.replacingOccurrences(of: "~1", with: "/").replacingOccurrences(of: "~0", with: "~") }) {
    switch current {
    case let .object(object):
      guard let next = object[segment] else { return nil }
      current = next
    case let .array(array):
      guard let index = Int(segment), array.indices.contains(index) else { return nil }
      current = array[index]
    default: return nil
    }
  }
  return current
}

private func setPointer(_ value: NativeJSON, path: String, value next: NativeJSON) -> NativeJSON {
  guard path != "" && path != "/" else { return next }
  var segments = path.split(separator: "/", omittingEmptySubsequences: true).map {
    $0.replacingOccurrences(of: "~1", with: "/").replacingOccurrences(of: "~0", with: "~")
  }
  guard let first = segments.first else { return next }
  segments.removeFirst()
  switch value {
  case let .object(object):
    var copy = object
    copy[first] = setPointer(copy[first] ?? .object([:]), path: "/" + segments.joined(separator: "/"), value: next)
    return .object(copy)
  case let .array(array):
    guard let index = Int(first), array.indices.contains(index) else { return value }
    var copy = array
    copy[index] = setPointer(copy[index], path: "/" + segments.joined(separator: "/"), value: next)
    return .array(copy)
  default: return value
  }
}
