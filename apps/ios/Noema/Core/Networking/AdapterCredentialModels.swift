import Foundation
import NoemaAPI

enum AdapterCredentialError: LocalizedError {
  case invalidDocument

  var errorDescription: String? {
    "Choose a non-empty credential document no larger than 128 KB."
  }
}

struct AdapterCredentialSetupModel: Hashable {
  let credentialType: String
  let setupURL: URL?
  let instructions: [String]
  let inputKind: String
  let fields: [AdapterCredentialFieldModel]
  let documentMediaType: String?
  let redirectURI: String?
  let normalizationTransform: AdapterCredentialTransformModel?
  let requestAuthTransform: AdapterCredentialTransformModel?

  init(_ source: NoemaAPI.AdapterCredentialSetupFields) {
    credentialType = source.credentialType
    setupURL = URL(string: source.setupUrl)
    instructions = source.instructions
    inputKind = source.inputKind
    fields = source.fields.map { AdapterCredentialFieldModel(id: $0.fieldId, label: $0.label) }
    documentMediaType = source.documentMediaType
    redirectURI = source.redirectUri
    normalizationTransform = source.normalizationTransform.map {
      AdapterCredentialTransformModel(language: $0.language, sourceDigest: $0.sourceDigest, source: $0.source)
    }
    requestAuthTransform = source.requestAuthTransform.map {
      AdapterCredentialTransformModel(language: $0.language, sourceDigest: $0.sourceDigest, source: $0.source)
    }
  }
}

struct AdapterCredentialFieldModel: Identifiable, Hashable {
  let id: String
  let label: String
}

struct AdapterCredentialTransformModel: Hashable {
  let language: String
  let sourceDigest: String
  let source: String
}

struct AdapterCredentialSubmission: Hashable {
  let fieldValues: [AdapterCredentialValue]
  let document: Data?
}

struct AdapterCredentialValue: Hashable {
  let fieldID: String
  let value: String
}
