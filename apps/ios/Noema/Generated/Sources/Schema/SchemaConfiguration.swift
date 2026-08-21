// @generated
// This file was automatically generated and can be edited to
// provide custom configuration for a generated GraphQL schema.
//
// Any changes to this file will not be overwritten by future
// code generation execution.

import ApolloAPI

nonisolated public enum SchemaConfiguration: ApolloAPI.SchemaConfiguration {
  public static func cacheKeyInfo(for type: ApolloAPI.Object, object: ApolloAPI.ObjectData) -> CacheKeyInfo? {
    switch type {
    case Objects.TaskDetail:
      return try? CacheKeyInfo(jsonValue: object["taskId"])
    case Objects.TaskSummary:
      return try? CacheKeyInfo(jsonValue: object["taskId"])
    case Objects.WorkflowStage:
      return try? CacheKeyInfo(jsonValue: object["stageId"])
    case Objects.CurrentRunSummary:
      return try? CacheKeyInfo(jsonValue: object["runId"])
    default:
      return nil
    }
  }
}
