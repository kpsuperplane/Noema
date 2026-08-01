// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Complete first-run model assignment input.
nonisolated public struct ConfirmOnboardingModelSelectionsInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerAccountId: String,
    noema: OnboardingModelSelectionInput,
    simpleTasks: OnboardingModelSelectionInput,
    mediumTasks: OnboardingModelSelectionInput,
    difficultTasks: OnboardingModelSelectionInput,
    taskReviewer: OnboardingModelSelectionInput,
    webFetchSummarizer: OnboardingModelSelectionInput,
    toolProgressAudit: OnboardingModelSelectionInput,
    actionReviewer: GraphQLNullable<OnboardingModelSelectionInput> = nil,
    memoryConsolidation: OnboardingModelSelectionInput
  ) {
    __data = InputDict([
      "providerAccountId": providerAccountId,
      "noema": noema,
      "simpleTasks": simpleTasks,
      "mediumTasks": mediumTasks,
      "difficultTasks": difficultTasks,
      "taskReviewer": taskReviewer,
      "webFetchSummarizer": webFetchSummarizer,
      "toolProgressAudit": toolProgressAudit,
      "actionReviewer": actionReviewer,
      "memoryConsolidation": memoryConsolidation
    ])
  }

  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  public var noema: OnboardingModelSelectionInput {
    get { __data["noema"] }
    set { __data["noema"] = newValue }
  }

  public var simpleTasks: OnboardingModelSelectionInput {
    get { __data["simpleTasks"] }
    set { __data["simpleTasks"] = newValue }
  }

  public var mediumTasks: OnboardingModelSelectionInput {
    get { __data["mediumTasks"] }
    set { __data["mediumTasks"] = newValue }
  }

  public var difficultTasks: OnboardingModelSelectionInput {
    get { __data["difficultTasks"] }
    set { __data["difficultTasks"] = newValue }
  }

  public var taskReviewer: OnboardingModelSelectionInput {
    get { __data["taskReviewer"] }
    set { __data["taskReviewer"] = newValue }
  }

  public var webFetchSummarizer: OnboardingModelSelectionInput {
    get { __data["webFetchSummarizer"] }
    set { __data["webFetchSummarizer"] = newValue }
  }

  public var toolProgressAudit: OnboardingModelSelectionInput {
    get { __data["toolProgressAudit"] }
    set { __data["toolProgressAudit"] = newValue }
  }

  public var actionReviewer: GraphQLNullable<OnboardingModelSelectionInput> {
    get { __data["actionReviewer"] }
    set { __data["actionReviewer"] = newValue }
  }

  public var memoryConsolidation: OnboardingModelSelectionInput {
    get { __data["memoryConsolidation"] }
    set { __data["memoryConsolidation"] = newValue }
  }
}
