import type { CodegenConfig } from "@graphql-codegen/cli";

const config: CodegenConfig = {
  schema: "src/generated/schema.graphql",
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
    }
  }
};

export default config;
