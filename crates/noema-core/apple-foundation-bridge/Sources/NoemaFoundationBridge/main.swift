import Foundation

#if canImport(FoundationModels)
@preconcurrency import FoundationModels
#endif

struct BridgeRequest: Decodable {
    let id: String
    let payload: Payload

    enum Payload: Decodable {
        case handshake(protocolVersion: Int)
        case health
        case createSession(conversationID: String, modelProfile: String, instructions: String?)
        case generate(sessionID: String, input: String)
        case closeSession(sessionID: String)
        case shutdown
        case unsupported

        enum CodingKeys: String, CodingKey {
            case type
            case protocolVersion = "protocol_version"
            case conversationID = "conversation_id"
            case modelProfile = "model_profile"
            case instructions
            case sessionID = "session_id"
            case input
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
                self = .createSession(
                    conversationID: conversationID,
                    modelProfile: modelProfile,
                    instructions: instructions
                )
            case "generate":
                let sessionID = try container.decode(String.self, forKey: .sessionID)
                let input = try container.decode(String.self, forKey: .input)
                self = .generate(sessionID: sessionID, input: input)
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

protocol BridgeRequestHandling {
    func healthPayload() -> [String: Any]
    func createSession(
        conversationID: String,
        modelProfile: String,
        instructions: String?
    ) -> [String: Any]
    func generate(sessionID: String, input: String) async -> [[String: Any]]
    func closeSession(sessionID: String) -> [String: Any]
}

func emit(_ id: String, _ payload: [String: Any]) {
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

    func healthPayload() -> [String: Any] {
        makeHealthPayload(available: false, unavailableReason: reason)
    }

    func createSession(
        conversationID: String,
        modelProfile: String,
        instructions: String?
    ) -> [String: Any] {
        errorPayload(code: "foundation_unavailable", message: reason)
    }

    func generate(sessionID: String, input: String) async -> [[String: Any]] {
        [errorPayload(code: "foundation_unavailable", message: reason)]
    }

    func closeSession(sessionID: String) -> [String: Any] {
        [
            "type": "replay_complete"
        ]
    }
}

#if canImport(FoundationModels)
@available(macOS 26.0, *)
final class FoundationModelsHandler: BridgeRequestHandling {
    private var sessions: [String: LanguageModelSession] = [:]

    func healthPayload() -> [String: Any] {
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
        instructions: String?
    ) -> [String: Any] {
        switch SystemLanguageModel.default.availability {
        case .available:
            let sessionID = "session:\(UUID().uuidString)"
            sessions[sessionID] = LanguageModelSession(
                model: .default,
                instructions: instructions
            )
            return [
                "type": "session_created",
                "session_id": sessionID
            ]
        case .unavailable(let reason):
            return errorPayload(
                code: "foundation_unavailable",
                message: unavailableMessage(for: reason)
            )
        }
    }

    func generate(sessionID: String, input: String) async -> [[String: Any]] {
        guard let session = sessions[sessionID] else {
            return [
                errorPayload(
                    code: "session_not_found",
                    message: "Foundation Models session was not found."
                )
            ]
        }
        guard !session.isResponding else {
            return [
                errorPayload(
                    code: "session_busy",
                    message: "Foundation Models session is already responding."
                )
            ]
        }

        do {
            let response = try await session.respond(to: input)
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

    func closeSession(sessionID: String) -> [String: Any] {
        sessions.removeValue(forKey: sessionID)
        return [
            "type": "replay_complete"
        ]
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

func runBridge(handler: BridgeRequestHandling) async {
    while let line = readLine() {
        guard let data = line.data(using: .utf8) else {
            emit("unknown", errorPayload(
                code: "malformed_request",
                message: "Malformed bridge request."
            ))
            continue
        }

        guard let request = try? JSONDecoder().decode(BridgeRequest.self, from: data) else {
            emit("unknown", errorPayload(
                code: "malformed_request",
                message: "Malformed bridge request."
            ))
            continue
        }

        switch request.payload {
        case .handshake(let protocolVersion):
            emit(request.id, [
                "type": "handshake_ok",
                "protocol_version": protocolVersion
            ])
        case .health:
            emit(request.id, handler.healthPayload())
        case .createSession(let conversationID, let modelProfile, let instructions):
            emit(request.id, handler.createSession(
                conversationID: conversationID,
                modelProfile: modelProfile,
                instructions: instructions
            ))
        case .generate(let sessionID, let input):
            for payload in await handler.generate(sessionID: sessionID, input: input) {
                emit(request.id, payload)
            }
        case .closeSession(let sessionID):
            emit(request.id, handler.closeSession(sessionID: sessionID))
        case .shutdown:
            emit(request.id, [
                "type": "shutdown_ok"
            ])
            exit(0)
        case .unsupported:
            emit(request.id, errorPayload(
                code: "unsupported_request",
                message: "Unsupported bridge request."
            ))
        }
    }
}

@main
struct NoemaFoundationBridge {
    static func main() async {
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            await runBridge(handler: FoundationModelsHandler())
            return
        }
        #endif

        await runBridge(handler: UnavailableHandler(
            reason: "Foundation Models requires macOS 26 or newer."
        ))
    }
}
