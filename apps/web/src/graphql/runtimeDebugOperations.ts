import { gql } from "@apollo/client";

export const RuntimeDebugProfileDocument = gql`
  query RuntimeDebugProfile($input: RuntimeDebugProfileInput!) {
    runtimeDebugProfile(input: $input) {
      kind
      scopeId
      status
      startedAt
      endedAt
      elapsedMilliseconds
      accountedMilliseconds
      uninstrumentedMilliseconds
      spans {
        id
        category
        name
        status
        startedAt
        endedAt
        startOffsetMilliseconds
        durationMilliseconds
        provider
        model
        phase
        responseIndex
        roundIndex
        toolName
        correlationId
        inputTokens
        cachedInputTokens
        outputTokens
        totalTokens
      }
    }
  }
`;
