import type { CodegenConfig } from "@graphql-codegen/cli";

const config: CodegenConfig = {
  schema: "../../graphql/schema.graphql",
  documents: ["src/graphql/**/*.ts"],
  generates: {
    "src/generated/graphql.ts": {
      plugins: ["typescript-operations", "typed-document-node"],
      config: {
        avoidOptionals: false,
        maybeValue: "T | null",
        scalars: {
          JSON: "unknown"
        }
      }
    },
    "src/generated/possibleTypes.json": {
      plugins: ["fragment-matcher"],
      config: {
        apolloClientVersion: 3,
        deterministic: true
      }
    }
  }
};

export default config;
