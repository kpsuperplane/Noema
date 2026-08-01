// @generated
// This file was automatically generated and should not be edited.

import ApolloAPI

public extension Unions {
  /// Transcript item union.
  nonisolated static let TranscriptItem = Union(
    name: "TranscriptItem",
    possibleTypes: [
      Objects.UserText.self,
      Objects.AssistantText.self,
      Objects.Activity.self,
      Objects.A2UISurface.self,
      Objects.MultipleChoicePrompt.self,
      Objects.MultipleChoiceSelection.self,
      Objects.ErrorNotice.self,
      Objects.ArtifactReference.self,
      Objects.TaskReference.self
    ]
  )
}