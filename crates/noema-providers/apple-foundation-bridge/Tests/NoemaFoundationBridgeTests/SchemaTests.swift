#if canImport(FoundationModels)
import FoundationModels
import XCTest
@testable import NoemaFoundationBridge

@available(macOS 26.0, *)
final class SchemaTests: XCTestCase {
    func testOneOfSchemaBuildsWithDistinctNestedTypes() {
        let schema = """
        {
          "oneOf": [
            {
              "type": "object",
              "properties": {
                "query": { "type": "string" }
              },
              "required": ["query"]
            },
            {
              "type": "object",
              "properties": {
                "limit": { "type": "integer" }
              },
              "required": ["limit"]
            }
          ]
        }
        """

        XCTAssertNotNil(makeToolGenerationSchema(from: schema, name: "search_memory"))
    }
}
#endif
