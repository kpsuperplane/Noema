import Foundation

struct BridgeRequest: Decodable {
    let id: String
    let payload: Payload

    enum Payload: Decodable {
        case handshake(protocolVersion: Int)
        case health
        case shutdown
        case unsupported

        enum CodingKeys: String, CodingKey {
            case type
            case protocolVersion = "protocol_version"
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
            case "shutdown":
                self = .shutdown
            default:
                self = .unsupported
            }
        }
    }
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

while let line = readLine() {
    guard let data = line.data(using: .utf8) else {
        emit("unknown", [
            "type": "error",
            "code": "malformed_request",
            "message": "Malformed bridge request."
        ])
        continue
    }

    guard let request = try? JSONDecoder().decode(BridgeRequest.self, from: data) else {
        emit("unknown", [
            "type": "error",
            "code": "malformed_request",
            "message": "Malformed bridge request."
        ])
        continue
    }

    switch request.payload {
    case .handshake(let protocolVersion):
        emit(request.id, [
            "type": "handshake_ok",
            "protocol_version": protocolVersion
        ])
    case .health:
        emit(request.id, [
            "type": "health",
            "available": false,
            "profiles": [
                [
                    "id": "default",
                    "label": "Default on-device"
                ]
            ],
            "unavailable_reason": "Foundation Models runtime is not available in this bridge skeleton."
        ])
    case .shutdown:
        emit(request.id, [
            "type": "shutdown_ok"
        ])
        exit(0)
    case .unsupported:
        emit(request.id, [
            "type": "error",
            "code": "unsupported_request",
            "message": "Unsupported bridge request."
        ])
    }
}
