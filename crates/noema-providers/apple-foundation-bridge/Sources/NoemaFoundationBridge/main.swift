import Foundation

#if canImport(FoundationModels)
@preconcurrency import FoundationModels
#endif

struct BridgeToolDefinition: Decodable, Sendable {
    let name: String
    let description: String
    let parameters: String
}

struct BridgeReplayToolCall: Decodable, Sendable {
    let callID: String
    let toolName: String
    let arguments: String

    enum CodingKeys: String, CodingKey {
        case callID = "call_id"
        case toolName = "tool_name"
        case arguments
    }
}

struct BridgeReplayToolResult: Decodable, Sendable {
    let callID: String
    let toolName: String
    let output: String

    enum CodingKeys: String, CodingKey {
        case callID = "call_id"
        case toolName = "tool_name"
        case output
    }
}

struct BridgeRequest: Decodable, Sendable {
    let id: String
    let payload: Payload

    struct ReplayTurn: Decodable, Sendable {
        let role: Role
        let text: String
        let toolCall: BridgeReplayToolCall?
        let toolResult: BridgeReplayToolResult?

        enum CodingKeys: String, CodingKey {
            case role
            case text
            case toolCall = "tool_call"
            case toolResult = "tool_result"
        }
    }

    enum Role: String, Decodable, Sendable {
        case applicationContext = "application_context"
        case user
        case assistant
    }

    enum Payload: Decodable, Sendable {
        case handshake(protocolVersion: Int)
        case health
        case createSession(conversationID: String, modelProfile: String, instructions: String?, tools: [BridgeToolDefinition], toolCatalogFingerprint: String)
        case replayTurns(sessionID: String, turns: [ReplayTurn])
        case generate(sessionID: String, input: String, maxOutputTokens: Int?)
        case toolResult(sessionID: String, callID: String, output: String, isError: Bool)
        case countTokens(instructions: String?, input: String)
        case cancel(requestID: String)
        case closeSession(sessionID: String)
        case shutdown
        case unsupported

        enum CodingKeys: String, CodingKey {
            case type
            case protocolVersion = "protocol_version"
            case conversationID = "conversation_id"
            case modelProfile = "model_profile"
            case instructions
            case tools
            case toolCatalogFingerprint = "tool_catalog_fingerprint"
            case sessionID = "session_id"
            case callID = "call_id"
            case turns
            case input
            case maxOutputTokens = "max_output_tokens"
            case output
            case isError = "is_error"
            case requestID = "request_id"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let type = try container.decode(String.self, forKey: .type)

            switch type {
            case "handshake":
                let protocolVersion = try container.decode(Int.self, forKey: .protocolVersion)
                self = .handshake(protocolVersion: protocolVersion)
            case "health":
                self = .health
            case "create_session":
                let conversationID = try container.decode(String.self, forKey: .conversationID)
                let modelProfile = try container.decode(String.self, forKey: .modelProfile)
                let instructions = try container.decodeIfPresent(String.self, forKey: .instructions)
                let tools = try container.decodeIfPresent([BridgeToolDefinition].self, forKey: .tools) ?? []
                let toolCatalogFingerprint = try container.decodeIfPresent(String.self, forKey: .toolCatalogFingerprint) ?? ""
                self = .createSession(
                    conversationID: conversationID,
                    modelProfile: modelProfile,
                    instructions: instructions,
                    tools: tools,
                    toolCatalogFingerprint: toolCatalogFingerprint
                )
            case "replay_turns":
                let sessionID = try container.decode(String.self, forKey: .sessionID)
                let turns = try container.decode([ReplayTurn].self, forKey: .turns)
                self = .replayTurns(sessionID: sessionID, turns: turns)
            case "generate":
                let sessionID = try container.decode(String.self, forKey: .sessionID)
                let input = try container.decode(String.self, forKey: .input)
                let maxOutputTokens = try container.decodeIfPresent(Int.self, forKey: .maxOutputTokens)
                self = .generate(
                    sessionID: sessionID,
                    input: input,
                    maxOutputTokens: maxOutputTokens
                )
            case "tool_result":
                let sessionID = try container.decode(String.self, forKey: .sessionID)
                let callID = try container.decode(String.self, forKey: .callID)
                let output = try container.decode(String.self, forKey: .output)
                let isError = try container.decodeIfPresent(Bool.self, forKey: .isError) ?? false
                self = .toolResult(
                    sessionID: sessionID,
                    callID: callID,
                    output: output,
                    isError: isError
                )
            case "count_tokens":
                let instructions = try container.decodeIfPresent(String.self, forKey: .instructions)
                let input = try container.decode(String.self, forKey: .input)
                self = .countTokens(instructions: instructions, input: input)
            case "cancel":
                let requestID = try container.decode(String.self, forKey: .requestID)
                self = .cancel(requestID: requestID)
            case "close_session":
                let sessionID = try container.decode(String.self, forKey: .sessionID)
                self = .closeSession(sessionID: sessionID)
            case "shutdown":
                self = .shutdown
            default:
                self = .unsupported
            }
        }
    }
}

protocol BridgeRequestHandling: Sendable {
    func healthPayload() async -> sending [String: Any]
    func createSession(
        conversationID: String,
        modelProfile: String,
        instructions: String?,
        tools: [BridgeToolDefinition],
        toolCatalogFingerprint: String
    ) async -> sending [String: Any]
    func replayTurns(sessionID: String, turns: [BridgeRequest.ReplayTurn]) async -> sending [String: Any]
    func countTokens(instructions: String?, input: String) async -> sending [String: Any]
    func generate(
        sessionID: String,
        input: String,
        maxOutputTokens: Int?
    ) async -> sending [[String: Any]]
    func toolResult(sessionID: String, callID: String, output: String, isError: Bool) async -> sending [String: Any]
    func cancel(requestID: String) async -> sending [String: Any]
    func closeSession(sessionID: String) async -> sending [String: Any]
}

final class BridgeEmitter: @unchecked Sendable {
    private let lock = NSLock()

    func emit(_ id: String, _ payload: [String: Any]) {
        lock.lock()
        defer { lock.unlock() }
    let response: [String: Any] = [
        "id": id,
        "payload": payload
    ]

    guard JSONSerialization.isValidJSONObject(response),
          let data = try? JSONSerialization.data(withJSONObject: response) else {
        return
    }

    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0A]))
    }
}

func errorPayload(code: String, message: String) -> [String: Any] {
    [
        "type": "error",
        "code": code,
        "message": message
    ]
}

func makeHealthPayload(available: Bool, unavailableReason: String?) -> [String: Any] {
    let reason: Any = unavailableReason.map { $0 as Any } ?? NSNull()
    return [
        "type": "health",
        "available": available,
        "profiles": [
            [
                "id": "default",
                "label": "Default on-device"
            ]
        ],
        "unavailable_reason": reason
    ]
}

final class UnavailableHandler: BridgeRequestHandling {
    private let reason: String

    init(reason: String) {
        self.reason = reason
    }

    func healthPayload() async -> sending [String: Any] {
        makeHealthPayload(available: false, unavailableReason: reason)
    }

    func createSession(
        conversationID: String,
        modelProfile: String,
        instructions: String?,
        tools: [BridgeToolDefinition],
        toolCatalogFingerprint: String
    ) async -> sending [String: Any] {
        errorPayload(code: "foundation_unavailable", message: reason)
    }

    func replayTurns(sessionID: String, turns: [BridgeRequest.ReplayTurn]) async -> sending [String: Any] {
        errorPayload(code: "foundation_unavailable", message: reason)
    }

    func countTokens(instructions: String?, input: String) async -> sending [String: Any] {
        errorPayload(code: "foundation_unavailable", message: reason)
    }

    func generate(
        sessionID: String,
        input: String,
        maxOutputTokens: Int?
    ) async -> sending [[String: Any]] {
        [errorPayload(code: "foundation_unavailable", message: reason)]
    }

    func toolResult(sessionID: String, callID: String, output: String, isError: Bool) async -> sending [String: Any] {
        errorPayload(code: "foundation_unavailable", message: reason)
    }

    func cancel(requestID: String) async -> sending [String: Any] {
        errorPayload(
            code: "cancellation_unsupported",
            message: "Foundation Models cancellation is unavailable because the runtime is unavailable."
        )
    }

    func closeSession(sessionID: String) async -> sending [String: Any] {
        [
            "type": "replay_complete"
        ]
    }
}

#if canImport(FoundationModels)
@available(macOS 26.0, *)
func makeToolGenerationSchema(from schemaJSON: String, name: String) -> GenerationSchema? {
    guard
        let data = schemaJSON.data(using: .utf8),
        let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    else {
        return nil
    }

    let definitions = root["$defs"] as? [String: Any] ?? [:]
    let dependencies = definitions.compactMap { name, value in
        makeDynamicSchema(value, name: name)
    }
    guard let rootSchema = makeDynamicSchema(root, name: name) else {
        return nil
    }
    return try? GenerationSchema(root: rootSchema, dependencies: dependencies)
}

@available(macOS 26.0, *)
private func makeDynamicSchema(_ raw: Any, name: String) -> DynamicGenerationSchema? {
    guard let object = raw as? [String: Any] else {
        return nil
    }
    if let reference = object["$ref"] as? String {
        return DynamicGenerationSchema(referenceTo: reference.split(separator: "/").last.map(String.init) ?? reference)
    }
    if let choices = (object["oneOf"] as? [Any]) ?? (object["anyOf"] as? [Any]) {
        let schemas = choices.enumerated().compactMap { index, value in
            makeDynamicSchema(value, name: "\(name)_choice\(index)")
        }
        guard !schemas.isEmpty else { return nil }
        if schemas.count == 1 { return schemas[0] }
        return DynamicGenerationSchema(name: name, anyOf: schemas)
    }
    if let types = object["type"] as? [String] {
        let schemas = types.enumerated().compactMap { index, type in
            makeDynamicSchema(["type": type], name: "\(name)_type\(index)")
        }
        guard !schemas.isEmpty else { return nil }
        if schemas.count == 1 { return schemas[0] }
        return DynamicGenerationSchema(name: name, anyOf: schemas)
    }
    if let type = object["type"] as? String {
        switch type {
        case "null":
            // Nullable fields remain represented by their concrete branch on
            // macOS 26.0; the Rust parser accepts the value and applies the
            // canonical optional-field semantics after generation.
            return nil
        case "string":
            return DynamicGenerationSchema(type: String.self)
        case "integer":
            return DynamicGenerationSchema(type: Int.self)
        case "number":
            return DynamicGenerationSchema(type: Double.self)
        case "boolean":
            return DynamicGenerationSchema(type: Bool.self)
        case "array":
            guard let items = object["items"],
                  let itemSchema = makeDynamicSchema(items, name: "\(name)_item") else {
                return nil
            }
            let minimum = object["minItems"] as? Int
            let maximum = object["maxItems"] as? Int
            return DynamicGenerationSchema(
                arrayOf: itemSchema,
                minimumElements: minimum,
                maximumElements: maximum
            )
        case "object":
            let properties = object["properties"] as? [String: Any] ?? [:]
            let required = Set(object["required"] as? [String] ?? [])
            let dynamicProperties = properties.keys.sorted().compactMap { key -> DynamicGenerationSchema.Property? in
                guard let child = properties[key],
                      let childSchema = makeDynamicSchema(child, name: "\(name)_\(key)") else {
                    return nil
                }
                return DynamicGenerationSchema.Property(
                    name: key,
                    description: (child as? [String: Any])?["description"] as? String,
                    schema: childSchema,
                    isOptional: !required.contains(key)
                )
            }
            return DynamicGenerationSchema(
                name: name,
                properties: dynamicProperties
            )
        default:
            return nil
        }
    }
    return nil
}

@available(macOS 26.0, *)
private struct BridgeToolCallError: Error, LocalizedError, Sendable {
    let message: String

    var errorDescription: String? { message }
}

@available(macOS 26.0, *)
private actor ToolCallBroker {
    private let emitter: BridgeEmitter
    private var generationID: String?
    private var pending: [String: CheckedContinuation<String, Error>] = [:]

    init(emitter: BridgeEmitter) {
        self.emitter = emitter
    }

    func begin(generationID: String) {
        self.generationID = generationID
    }

    func request(toolName: String, arguments: GeneratedContent) async throws -> String {
        guard let generationID else {
            throw BridgeToolCallError(message: "Foundation Models tool call has no active generation")
        }
        let callID = "tool:\(UUID().uuidString)"
        return try await withCheckedThrowingContinuation { continuation in
            pending[callID] = continuation
            emitter.emit(generationID, [
                "type": "tool_call",
                "call_id": callID,
                "tool_name": toolName,
                "arguments": arguments.jsonString
            ])
        }
    }

    func resolve(callID: String, output: String, isError: Bool) {
        guard let continuation = pending.removeValue(forKey: callID) else { return }
        if isError {
            continuation.resume(throwing: BridgeToolCallError(message: output))
        } else {
            continuation.resume(returning: output)
        }
    }
}

@available(macOS 26.0, *)
private struct DynamicBridgeTool: FoundationModels.Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name: String
    let description: String
    let parameters: GenerationSchema
    private let broker: ToolCallBroker

    init(definition: BridgeToolDefinition, broker: ToolCallBroker) throws {
        guard let parameters = makeToolGenerationSchema(
            from: definition.parameters,
            name: definition.name
        ) else {
            throw BridgeToolCallError(
                message: "Foundation Models could not build the schema for native tool \(definition.name)"
            )
        }
        self.name = definition.name
        self.description = definition.description
        self.parameters = parameters
        self.broker = broker
    }

    @concurrent
    func call(arguments: GeneratedContent) async throws -> String {
        try await broker.request(toolName: name, arguments: arguments)
    }
}

@available(macOS 26.0, *)
actor FoundationModelsHandler: BridgeRequestHandling {
    private final class SessionState: @unchecked Sendable {
        let tools: [any FoundationModels.Tool]
        let broker: ToolCallBroker
        var session: LanguageModelSession

        init(session: LanguageModelSession, tools: [any FoundationModels.Tool], broker: ToolCallBroker) {
            self.session = session
            self.tools = tools
            self.broker = broker
        }
    }

    private let emitter: BridgeEmitter
    private var sessions: [String: SessionState] = [:]

    init(emitter: BridgeEmitter) {
        self.emitter = emitter
    }

    func healthPayload() async -> sending [String: Any] {
        switch SystemLanguageModel.default.availability {
        case .available:
            return makeHealthPayload(available: true, unavailableReason: nil)
        case .unavailable(let reason):
            return makeHealthPayload(
                available: false,
                unavailableReason: unavailableMessage(for: reason)
            )
        }
    }

    func createSession(
        conversationID: String,
        modelProfile: String,
        instructions: String?,
        tools: [BridgeToolDefinition],
        toolCatalogFingerprint: String
    ) async -> sending [String: Any] {
        switch SystemLanguageModel.default.availability {
        case .available:
            let sessionID = "session:\(UUID().uuidString)"
            let broker = ToolCallBroker(emitter: emitter)
            do {
                let dynamicTools = try tools.map { try DynamicBridgeTool(definition: $0, broker: broker) }
                let existentialTools: [any FoundationModels.Tool] = dynamicTools
                sessions[sessionID] = SessionState(
                    session: LanguageModelSession(
                        model: .default,
                        tools: existentialTools,
                        instructions: instructions
                    ),
                    tools: existentialTools,
                    broker: broker
                )
                return [
                    "type": "session_created",
                    "session_id": sessionID,
                    "tool_catalog_fingerprint": toolCatalogFingerprint
                ]
            } catch {
                return errorPayload(code: "unsupported_tool_schema", message: error.localizedDescription)
            }
        case .unavailable(let reason):
            return errorPayload(
                code: "foundation_unavailable",
                message: unavailableMessage(for: reason)
            )
        }
    }

    func replayTurns(sessionID: String, turns: [BridgeRequest.ReplayTurn]) async -> sending [String: Any] {
        guard let state = sessions[sessionID] else {
            return errorPayload(
                code: "session_not_found",
                message: "Foundation Models session was not found."
            )
        }
        guard !state.session.isResponding else {
            return errorPayload(
                code: "session_busy",
                message: "Foundation Models session is already responding."
            )
        }

        var entries = Array(state.session.transcript)
        entries.append(contentsOf: turns.compactMap(replayEntry(for:)))
        sessions[sessionID] = SessionState(
            session: LanguageModelSession(
                model: .default,
                tools: state.tools,
                transcript: Transcript(entries: entries)
            ),
            tools: state.tools,
            broker: state.broker
        )
        return [
            "type": "replay_complete"
        ]
    }

    func countTokens(instructions: String?, input: String) async -> sending [String: Any] {
        guard #available(macOS 26.4, *) else {
            return errorPayload(
                code: "token_count_failed",
                message: "Foundation Models token counting requires macOS 26.4 or newer."
            )
        }

        do {
            var total = try await SystemLanguageModel.default.tokenCount(for: Prompt(input))
            if let instructions {
                total += try await SystemLanguageModel.default.tokenCount(for: Instructions(instructions))
            }
            return [
                "type": "token_count",
                "tokens": total
            ]
        } catch {
            return errorPayload(
                code: "token_count_failed",
                message: "Foundation Models token count failed: \(error.localizedDescription)"
            )
        }
    }

    func generate(
        sessionID: String,
        input: String,
        maxOutputTokens: Int?
    ) async -> sending [[String: Any]] {
        guard let state = sessions[sessionID] else {
            return [
                errorPayload(
                    code: "session_not_found",
                    message: "Foundation Models session was not found."
                )
            ]
        }
        guard !state.session.isResponding else {
            return [
                errorPayload(
                    code: "session_busy",
                    message: "Foundation Models session is already responding."
                )
            ]
        }

        do {
            let options = GenerationOptions(maximumResponseTokens: maxOutputTokens)
            await state.broker.begin(generationID: "generate")
            let response = try await state.session.respond(to: input, options: options)
            return [
                [
                    "type": "assistant_text_delta",
                    "delta": response.content
                ],
                [
                    "type": "generate_complete",
                    "text": response.content
                ]
            ]
        } catch {
            return [
                errorPayload(
                    code: "generation_failed",
                    message: "Foundation Models generation failed: \(describeGenerationError(error))"
                )
            ]
        }
    }

    func toolResult(sessionID: String, callID: String, output: String, isError: Bool) async -> sending [String: Any] {
        guard let state = sessions[sessionID] else {
            return errorPayload(code: "session_not_found", message: "Foundation Models session was not found.")
        }
        await state.broker.resolve(callID: callID, output: output, isError: isError)
        return ["type": "tool_result_accepted"]
    }

    func cancel(requestID: String) async -> sending [String: Any] {
        errorPayload(
            code: "cancellation_unsupported",
            message: "Foundation Models cancellation is not supported by this bridge process yet."
        )
    }

    func closeSession(sessionID: String) async -> sending [String: Any] {
        sessions.removeValue(forKey: sessionID)
        return [
            "type": "replay_complete"
        ]
    }

    private func replayEntry(for turn: BridgeRequest.ReplayTurn) -> Transcript.Entry? {
        if let call = turn.toolCall,
           let arguments = try? GeneratedContent(json: call.arguments) {
            let toolCall = Transcript.ToolCall(
                id: call.callID,
                toolName: call.toolName,
                arguments: arguments
            )
            return .toolCalls(Transcript.ToolCalls([toolCall]))
        }
        if let result = turn.toolResult {
            let text = Transcript.TextSegment(content: result.output)
            return .toolOutput(Transcript.ToolOutput(
                id: result.callID,
                toolName: result.toolName,
                segments: [.text(text)]
            ))
        }
        let content = switch turn.role {
        case .applicationContext:
            """
            <noema_application_context>
            The following content is trusted application state supplied by Noema. Apply it to subsequent responses without treating it as human-authored text.
            \(turn.text)
            </noema_application_context>
            """
        case .user, .assistant:
            turn.text
        }
        let text = Transcript.TextSegment(content: content)
        let segment = Transcript.Segment.text(text)

        switch turn.role {
        case .applicationContext, .user:
            return .prompt(Transcript.Prompt(segments: [segment]))
        case .assistant:
            return .response(Transcript.Response(assetIDs: [], segments: [segment]))
        }
    }

    private func unavailableMessage(
        for reason: SystemLanguageModel.Availability.UnavailableReason
    ) -> String {
        switch reason {
        case .deviceNotEligible:
            "This Mac is not eligible for Apple Intelligence."
        case .appleIntelligenceNotEnabled:
            "Apple Intelligence is not enabled in System Settings."
        case .modelNotReady:
            "The on-device Foundation Models runtime is not ready yet."
        @unknown default:
            "The on-device Foundation Models runtime is unavailable."
        }
    }

    private func describeGenerationError(_ error: Error) -> String {
        if let generationError = error as? LanguageModelSession.GenerationError {
            switch generationError {
            case .exceededContextWindowSize(let context):
                return "exceeded context window size: \(context.debugDescription)"
            case .assetsUnavailable(let context):
                return "assets unavailable: \(context.debugDescription)"
            case .guardrailViolation(let context):
                return "guardrail violation: \(context.debugDescription)"
            case .unsupportedGuide(let context):
                return "unsupported guide: \(context.debugDescription)"
            case .unsupportedLanguageOrLocale(let context):
                return "unsupported language or locale: \(context.debugDescription)"
            case .decodingFailure(let context):
                return "decoding failure: \(context.debugDescription)"
            case .rateLimited(let context):
                return "rate limited: \(context.debugDescription)"
            case .concurrentRequests(let context):
                return "concurrent requests: \(context.debugDescription)"
            case .refusal(_, let context):
                return "refusal: \(context.debugDescription)"
            @unknown default:
                return "unknown generation error: \(generationError.localizedDescription)"
            }
        }

        let nsError = error as NSError
        return "\(error.localizedDescription) [domain=\(nsError.domain) code=\(nsError.code)]"
    }
}
#endif

func runBridge(handler: BridgeRequestHandling, emitter: BridgeEmitter) async {
    while let line = readLine() {
        guard let data = line.data(using: .utf8) else {
            emitter.emit("unknown", errorPayload(
                code: "malformed_request",
                message: "Malformed bridge request."
            ))
            continue
        }

        guard let request = try? JSONDecoder().decode(BridgeRequest.self, from: data) else {
            emitter.emit("unknown", errorPayload(
                code: "malformed_request",
                message: "Malformed bridge request."
            ))
            continue
        }

        Task {
            await dispatch(request, handler: handler, emitter: emitter)
        }
    }
}

func dispatch(_ request: BridgeRequest, handler: BridgeRequestHandling, emitter: BridgeEmitter) async {
    switch request.payload {
    case .handshake(let protocolVersion):
        emitter.emit(request.id, [
            "type": "handshake_ok",
            "protocol_version": protocolVersion
        ])
    case .health:
        emitter.emit(request.id, await handler.healthPayload())
    case .createSession(let conversationID, let modelProfile, let instructions, let tools, let fingerprint):
        emitter.emit(request.id, await handler.createSession(
            conversationID: conversationID,
            modelProfile: modelProfile,
            instructions: instructions,
            tools: tools,
            toolCatalogFingerprint: fingerprint
        ))
    case .replayTurns(let sessionID, let turns):
        emitter.emit(request.id, await handler.replayTurns(sessionID: sessionID, turns: turns))
    case .countTokens(let instructions, let input):
        emitter.emit(request.id, await handler.countTokens(instructions: instructions, input: input))
    case .generate(let sessionID, let input, let maxOutputTokens):
        for payload in await handler.generate(
            sessionID: sessionID,
            input: input,
            maxOutputTokens: maxOutputTokens
        ) {
            emitter.emit(request.id, payload)
        }
    case .toolResult(let sessionID, let callID, let output, let isError):
        emitter.emit(request.id, await handler.toolResult(
            sessionID: sessionID,
            callID: callID,
            output: output,
            isError: isError
        ))
    case .cancel(let requestID):
        emitter.emit(request.id, await handler.cancel(requestID: requestID))
    case .closeSession(let sessionID):
        emitter.emit(request.id, await handler.closeSession(sessionID: sessionID))
    case .shutdown:
        emitter.emit(request.id, ["type": "shutdown_ok"])
        exit(0)
    case .unsupported:
        emitter.emit(request.id, errorPayload(
            code: "unsupported_request",
            message: "Unsupported bridge request."
        ))
    }
}

@main
struct NoemaFoundationBridge {
    static func main() async {
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            let emitter = BridgeEmitter()
            await runBridge(handler: FoundationModelsHandler(emitter: emitter), emitter: emitter)
            return
        }
        #endif

        let emitter = BridgeEmitter()
        await runBridge(
            handler: UnavailableHandler(reason: "Foundation Models requires macOS 26 or newer."),
            emitter: emitter
        )
    }
}
