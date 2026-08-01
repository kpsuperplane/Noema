// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Closed durable task-run transcript item kind.
nonisolated public enum TaskRunItemKind: String, EnumType {
  case modelInput = "MODEL_INPUT"
  case assistantOutput = "ASSISTANT_OUTPUT"
  case toolCall = "TOOL_CALL"
  case toolResult = "TOOL_RESULT"
  case progressNotice = "PROGRESS_NOTICE"
  case contextCheckpoint = "CONTEXT_CHECKPOINT"
  case taskSubmission = "TASK_SUBMISSION"
  case taskReview = "TASK_REVIEW"
  case artifactReference = "ARTIFACT_REFERENCE"
  case failure = "FAILURE"
  case cancellation = "CANCELLATION"
}
