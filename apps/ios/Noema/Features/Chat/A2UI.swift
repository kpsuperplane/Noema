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
  let onSubmit: (String, String, Any?, Any?) -> Void
  @State private var dataModel: NativeJSON = .object([:])
  @State private var localValues: [String: NativeJSON] = [:]

  init(surface: A2UISurfaceModel, onSubmit: @escaping (String, String, Any?, Any?) -> Void) {
    self.surface = surface
    self.onSubmit = onSubmit
  }

  var body: some View {
    if let snapshot = A2UISnapshotModel(surface: surface) {
      A2UIContent(surface: surface, snapshot: snapshot, dataModel: $dataModel, localValues: $localValues, onSubmit: onSubmit)
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
    "\(surface.revision):\(surface.interactionRevision ?? -1):\(surface.lifecycle)"
  }
}

private struct A2UIContent: View {
  let surface: A2UISurfaceModel
  let snapshot: A2UISnapshotModel
  @Binding var dataModel: NativeJSON
  @Binding var localValues: [String: NativeJSON]
  let onSubmit: (String, String, Any?, Any?) -> Void

  private var interactive: Bool {
    surface.lifecycle == "pending" && surface.hasActions && surface.interactionID != nil && surface.interactionRevision != nil
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
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
          .foregroundStyle(NoemaColor.content))
      case "Row":
        return AnyView(ViewThatFits(in: .horizontal) {
          HStack(spacing: NoemaSpacing.sm) { renderChildren(component, ancestors: next) }
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) { renderChildren(component, ancestors: next) }
        })
      case "Column":
        return AnyView(VStack(alignment: .leading, spacing: NoemaSpacing.sm) { renderChildren(component, ancestors: next) })
      case "Card":
        return AnyView(NoemaCard(padding: NoemaSpacing.md) {
          render(component["child"]?.stringValue ?? "", ancestors: next)
        })
      case "Divider":
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
        .buttonStyle(.borderedProminent)
        .disabled(!interactive || actionName == nil)
    } else if variant == "borderless" {
      button
        .buttonStyle(.plain)
        .foregroundStyle(NoemaColor.accent)
        .disabled(!interactive || actionName == nil)
    } else {
      button
        .buttonStyle(.bordered)
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
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      TextField("", text: binding, axis: component["variant"]?.stringValue == "longText" ? .vertical : .horizontal)
        .textFieldStyle(.roundedBorder)
        .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
    }
  }

  private func checkBox(component: [String: NativeJSON], id: String) -> some View {
    let binding = Binding<Bool>(
      get: { value(for: id, dynamic: component["value"]).boolValue },
      set: { update(id: id, dynamic: component["value"], value: .bool($0)) }
    )
    return Toggle(resolve(component["label"]).stringValue, isOn: binding)
      .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
  }

  private func choicePicker(component: [String: NativeJSON], id: String) -> some View {
    let options = choiceOptions(component["options"])
    let label = resolve(component["label"] ?? .string("Choose an option")).stringValue
    let multiple = component["variant"]?.stringValue == "multipleSelection"
    let selected = value(for: id, dynamic: component["value"]).arrayValue.map(\.stringValue)
    return AnyView(VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.captionEmphasized)
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
            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
              .foregroundStyle(isSelected ? NoemaColor.accent : NoemaColor.contentTertiary)
            Text(option.1)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.content)
              .multilineTextAlignment(.leading)
            Spacer(minLength: 0)
          }
          .padding(.horizontal, NoemaSpacing.sm)
          .padding(.vertical, NoemaSpacing.compact)
          .frame(maxWidth: .infinity, alignment: .leading)
          .background(isSelected ? NoemaColor.pine50 : NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .overlay {
            RoundedRectangle(cornerRadius: NoemaRadius.element)
              .stroke(isSelected ? NoemaColor.accent.opacity(0.45) : NoemaColor.separatorSubtle, lineWidth: 1)
          }
        }
        .buttonStyle(.plain)
        .disabled(!interactive || !snapshot.sendDataModel || bindingPath(component["value"]) == nil)
      }
    })
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
  case "h1", "heading", "heading1", "title":
    return NoemaFont.pageTitle
  case "h2", "heading2", "subtitle":
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
