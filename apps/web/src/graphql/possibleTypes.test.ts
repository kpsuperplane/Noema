import assert from "node:assert/strict";
import { test } from "node:test";
import { InMemoryCache, gql } from "@apollo/client";
import possibleTypes from "@/generated/possibleTypes.json";

test("generated union metadata preserves human intervention fragments", () => {
  const query = gql`
    query HumanInterventionCacheFixture {
      pendingHumanInterventions {
        __typename
        ... on AdapterDefinition {
          semanticDigest
          connections { connectionId }
        }
      }
    }
  `;
  const cache = new InMemoryCache({ possibleTypes: possibleTypes.possibleTypes });
  const data = {
    pendingHumanInterventions: [{
      __typename: "AdapterDefinition",
      semanticDigest: "definition-digest",
      connections: [{ __typename: "AdapterConnection", connectionId: "connection-1" }]
    }]
  };

  cache.writeQuery({ query, data });

  assert.deepEqual(cache.readQuery({ query }), data);
});
