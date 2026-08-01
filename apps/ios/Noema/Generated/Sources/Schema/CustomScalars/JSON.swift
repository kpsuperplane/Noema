// @generated
// This file was automatically generated and can be edited to
// implement advanced custom scalar functionality.
//
// Any changes to this file will not be overwritten by future
// code generation execution.

@_spi(Internal) @_spi(Execution) import ApolloAPI
import Foundation

/// A scalar that can represent any JSON value.
public indirect enum JSON: CustomScalarType {
  case object([String: JSON])
  case array([JSON])
  case string(String)
  case integer(Int)
  case number(Double)
  case bool(Bool)
  case null

  @_spi(Internal)
  public init(_jsonValue value: JSONValue) throws {
    guard let decoded = Self(foundationValue: value) else {
      throw JSONDecodingError.couldNotConvert(value: value, to: Self.self)
    }
    self = decoded
  }

  public init?(foundationValue value: Any) {
    switch value {
    case let value as [String: JSONValue]:
      var result: [String: JSON] = [:]
      for (key, item) in value {
        guard let converted = Self(foundationValue: item) else { return nil }
        result[key] = converted
      }
      self = .object(result)
    case let value as [String: Any]:
      var result: [String: JSON] = [:]
      for (key, item) in value {
        guard let converted = Self(foundationValue: item) else { return nil }
        result[key] = converted
      }
      self = .object(result)
    case let value as [JSONValue]:
      let result = value.compactMap(Self.init(foundationValue:))
      guard result.count == value.count else { return nil }
      self = .array(result)
    case let value as [Any]:
      let result = value.compactMap(Self.init(foundationValue:))
      guard result.count == value.count else { return nil }
      self = .array(result)
    case let value as String: self = .string(value)
    case let value as Bool: self = .bool(value)
    case let value as Int: self = .integer(value)
    case let value as Int32: self = .integer(Int(value))
    case let value as Int64:
      guard let converted = Int(exactly: value) else { return nil }
      self = .integer(converted)
    case let value as Float: self = .number(Double(value))
    case let value as Double: self = .number(value)
    case let value as NSNumber: self = .number(value.doubleValue)
    case _ as NSNull: self = .null
    default: return nil
    }
  }

  @_spi(Internal)
  public var _jsonValue: JSONValue {
    switch self {
    case let .object(value):
      let encodable: JSONEncodableDictionary = value.mapValues { $0 }
      return encodable._jsonValue
    case let .array(value):
      return value._jsonValue
    case let .string(value): return value
    case let .integer(value): return value
    case let .number(value): return value
    case let .bool(value): return value
    case .null: return NSNull()
    }
  }

  public var encodedString: String {
    guard let data = try? JSONSerialization.data(
      withJSONObject: foundationValue,
      options: [.fragmentsAllowed, .sortedKeys]
    ) else { return "null" }
    return String(decoding: data, as: UTF8.self)
  }

  public var foundationValue: Any {
    switch self {
    case let .object(value): return value.mapValues(\.foundationValue)
    case let .array(value): return value.map(\.foundationValue)
    case let .string(value): return value
    case let .integer(value): return value
    case let .number(value): return value
    case let .bool(value): return value
    case .null: return NSNull()
    }
  }
}
