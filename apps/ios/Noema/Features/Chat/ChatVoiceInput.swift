import AVFoundation
import Foundation
import Observation
import Speech

enum ChatVoiceInputMode: Equatable {
  case toggle
  case hold
}

enum ChatVoiceInputState: Equatable {
  case idle
  case preparing
  case recording(ChatVoiceInputMode)
  case finalizing
  case failed
}

enum ChatVoiceInputCompletion: Equatable {
  case send(String)
  case keepDraft(String, error: String?)
  case restoreDraft(String)
}

struct ChatVoiceInputAlert: Identifiable, Equatable {
  let id = UUID()
  let message: String
  let offersSettings: Bool
}

@MainActor
@Observable
final class ChatVoiceInput {
  private(set) var state: ChatVoiceInputState = .idle
  private(set) var completionGeneration = 0
  private(set) var alert: ChatVoiceInputAlert?
  private(set) var cancelTargeted = false

  @ObservationIgnored private var completion: ChatVoiceInputCompletion?
  @ObservationIgnored private var originalDraft = ""
  @ObservationIgnored private var finalizedText = ""
  @ObservationIgnored private var volatileText = ""
  @ObservationIgnored private var mode: ChatVoiceInputMode = .toggle
  @ObservationIgnored private var pressing = false
  @ObservationIgnored private var sessionGeneration = 0
  @ObservationIgnored private var preparationTask: Task<Void, Never>?
  @ObservationIgnored private var finishingTask: Task<Void, Never>?
  @ObservationIgnored private var resultsTask: Task<Void, Never>?
  @ObservationIgnored private var analyzer: SpeechAnalyzer?
  @ObservationIgnored private var inputContinuation: AsyncStream<AnalyzerInput>.Continuation?
  @ObservationIgnored private var audioEngine: AVAudioEngine?
  @ObservationIgnored private var tapInstalled = false

  var isEngaged: Bool {
    switch state {
    case .preparing, .recording, .finalizing: true
    case .idle, .failed: false
    }
  }

  var showsCancel: Bool {
    switch state {
    case .preparing, .recording: true
    case .idle, .finalizing, .failed: false
    }
  }

  var isRecording: Bool {
    if case .recording = state { return true }
    return false
  }

  var previewText: String {
    Self.join(originalDraft, Self.join(finalizedText, volatileText))
  }

  var currentDraft: String {
    previewText.isEmpty ? originalDraft : previewText
  }

  var previewPlaceholder: String {
    switch state {
    case .preparing: "Preparing voice input…"
    case .recording: "Listening…"
    case .finalizing: "Finishing dictation…"
    case .idle, .failed: "Listening…"
    }
  }

  func pressBegan(originalDraft: String) {
    switch state {
    case .idle, .failed:
      start(originalDraft: originalDraft)
    case .preparing where mode == .toggle && !pressing,
         .recording where mode == .toggle && !pressing:
      pressing = true
      finish(.send)
    case .preparing, .recording, .finalizing:
      break
    }
  }

  func holdThresholdReached() -> Bool {
    guard pressing, showsCancel else { return false }
    mode = .hold
    if case .recording = state { state = .recording(.hold) }
    return true
  }

  func pressEnded(overCancel: Bool) {
    defer {
      pressing = false
      cancelTargeted = false
    }
    guard showsCancel else { return }
    if mode == .hold { finish(overCancel ? .restore : .send) }
  }

  func accessibilityActivate(originalDraft: String) {
    if isEngaged {
      finish(.send)
    } else {
      start(originalDraft: originalDraft)
      mode = .toggle
      pressing = false
    }
  }

  func setCancelTargeted(_ targeted: Bool) {
    cancelTargeted = mode == .hold && targeted
  }

  func cancel() {
    guard showsCancel else { return }
    finish(.restore)
  }

  func forceStop(message: String? = nil) {
    guard showsCancel else { return }
    finish(.keep(error: message))
  }

  func takeCompletion() -> ChatVoiceInputCompletion? {
    defer { completion = nil }
    return completion
  }

  func clearAlert() {
    alert = nil
  }

  private func start(originalDraft: String) {
    sessionGeneration &+= 1
    let generation = sessionGeneration
    self.originalDraft = originalDraft
    finalizedText = ""
    volatileText = ""
    completion = nil
    alert = nil
    mode = .toggle
    pressing = true
    state = .preparing
    preparationTask = Task { [weak self] in
      await self?.prepare(generation: generation)
    }
  }

  private func prepare(generation: Int) async {
    do {
      guard await requestMicrophonePermission() else {
        throw ChatVoiceInputError.microphonePermission
      }
      try Task.checkCancellation()
      guard generation == sessionGeneration else { return }
      guard await requestSpeechPermission() else {
        throw ChatVoiceInputError.speechPermission
      }
      try Task.checkCancellation()
      guard generation == sessionGeneration else { return }

      if SpeechTranscriber.isAvailable,
         let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale.current) {
        let transcriber = SpeechTranscriber(locale: locale, preset: .progressiveTranscription)
        resultsTask = resultTask(
          transcriber.results,
          generation: generation,
          transform: { (result: SpeechTranscriber.Result) in
            (String(result.text.characters), result.isFinal)
          }
        )
        try await startPipeline(module: transcriber, generation: generation)
      } else if let locale = await DictationTranscriber.supportedLocale(equivalentTo: Locale.current) {
        let transcriber = DictationTranscriber(locale: locale, preset: .progressiveShortDictation)
        resultsTask = resultTask(
          transcriber.results,
          generation: generation,
          transform: { (result: DictationTranscriber.Result) in
            (String(result.text.characters), result.isFinal)
          }
        )
        try await startPipeline(module: transcriber, generation: generation)
      } else {
        throw ChatVoiceInputError.unsupportedLocale
      }
    } catch is CancellationError {
      return
    } catch {
      guard generation == sessionGeneration else { return }
      failPreparation(error)
    }
  }

  private func startPipeline(
    module: any SpeechModule,
    generation: Int
  ) async throws {
    let modules: [any SpeechModule] = [module]
    if let request = try await AssetInventory.assetInstallationRequest(supporting: modules) {
      try await request.downloadAndInstall()
    }
    try Task.checkCancellation()
    guard generation == sessionGeneration else { return }

    let audioSession = AVAudioSession.sharedInstance()
    try audioSession.setCategory(.record, mode: .measurement)
    try audioSession.setActive(true)
    guard audioSession.isInputAvailable else { throw ChatVoiceInputError.audioUnavailable }

    let engine = AVAudioEngine()
    let inputNode = engine.inputNode
    let inputFormat = inputNode.outputFormat(forBus: 0)
    guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0,
          let analyzerFormat = await SpeechAnalyzer.bestAvailableAudioFormat(
            compatibleWith: modules,
            considering: inputFormat
          ) else { throw ChatVoiceInputError.audioUnavailable }
    try Task.checkCancellation()
    guard generation == sessionGeneration, state == .preparing else { throw CancellationError() }

    let converter = try ChatVoiceAudioConverter(inputFormat: inputFormat, outputFormat: analyzerFormat)
    let (inputSequence, continuation) = AsyncStream.makeStream(of: AnalyzerInput.self)
    let analyzer = SpeechAnalyzer(modules: modules)
    self.analyzer = analyzer
    inputContinuation = continuation
    audioEngine = engine

    inputNode.installTap(onBus: 0, bufferSize: 1_024, format: inputFormat) { [weak self] buffer, _ in
      do {
        let converted = try converter.convert(buffer)
        continuation.yield(AnalyzerInput(buffer: converted))
      } catch {
        Task { @MainActor [weak self] in
          self?.pipelineFailed(error, generation: generation)
        }
      }
    }
    tapInstalled = true

    try await analyzer.start(inputSequence: inputSequence)
    try Task.checkCancellation()
    guard generation == sessionGeneration, state == .preparing else { throw CancellationError() }
    engine.prepare()
    try engine.start()
    state = .recording(mode)
  }

  private func resultTask<Results: AsyncSequence>(
    _ results: Results,
    generation: Int,
    transform: @escaping @Sendable (Results.Element) -> (String, Bool)
  ) -> Task<Void, Never> where Results: Sendable, Results.Failure == any Error {
    Task { [weak self] in
      do {
        for try await result in results {
          guard !Task.isCancelled else { return }
          let value = transform(result)
          self?.accept(value.0, final: value.1, generation: generation)
        }
      } catch {
        self?.pipelineFailed(error, generation: generation)
      }
    }
  }

  private func accept(_ text: String, final: Bool, generation: Int) {
    guard generation == sessionGeneration else { return }
    if final {
      finalizedText = Self.join(finalizedText, text)
      volatileText = ""
    } else {
      volatileText = text
    }
  }

  private enum FinishKind {
    case send
    case keep(error: String?)
    case restore
  }

  private func finish(_ kind: FinishKind) {
    guard finishingTask == nil else { return }
    let generation = sessionGeneration
    state = .finalizing
    preparationTask?.cancel()
    finishingTask = Task {
      await finishPipeline(kind, generation: generation)
    }
  }

  private func finishPipeline(_ kind: FinishKind, generation: Int) async {
    stopAudioCapture()
    inputContinuation?.finish()

    var finishError: Error?
    if let analyzer {
      switch kind {
      case .restore:
        await analyzer.cancelAndFinishNow()
      case .send, .keep:
        do {
          try await analyzer.finalizeAndFinishThroughEndOfInput()
          await resultsTask?.value
        } catch {
          finishError = error
        }
      }
    }

    guard generation == sessionGeneration else { return }
    let dictated = Self.join(finalizedText, volatileText)
    let combined = Self.join(originalDraft, dictated)
    tearDown()

    switch kind {
    case .restore:
      publish(.restoreDraft(originalDraft))
    case .send where !dictated.isEmpty && finishError == nil:
      publish(.send(combined))
    case .send:
      let message = finishError.map(Self.failureMessage)
      if let message { alert = ChatVoiceInputAlert(message: message, offersSettings: false) }
      publish(.keepDraft(combined, error: message))
    case let .keep(error):
      let message = error ?? finishError.map(Self.failureMessage)
      if let message { alert = ChatVoiceInputAlert(message: message, offersSettings: false) }
      publish(.keepDraft(combined, error: message))
    }
  }

  private func failPreparation(_ error: Error) {
    let message = Self.failureMessage(error)
    let permissionError = (error as? ChatVoiceInputError).map {
      $0 == .microphonePermission || $0 == .speechPermission
    } ?? false
    alert = ChatVoiceInputAlert(
      message: message,
      offersSettings: permissionError
    )
    tearDown()
    publish(.keepDraft(originalDraft, error: message))
    state = .failed
  }

  private func pipelineFailed(_ error: Error, generation: Int) {
    guard generation == sessionGeneration, showsCancel else { return }
    finish(.keep(error: Self.failureMessage(error)))
  }

  private func publish(_ value: ChatVoiceInputCompletion) {
    completion = value
    completionGeneration &+= 1
    state = .idle
  }

  private func stopAudioCapture() {
    if tapInstalled {
      audioEngine?.inputNode.removeTap(onBus: 0)
      tapInstalled = false
    }
    audioEngine?.stop()
  }

  private func tearDown() {
    stopAudioCapture()
    inputContinuation?.finish()
    preparationTask?.cancel()
    resultsTask?.cancel()
    try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    analyzer = nil
    inputContinuation = nil
    audioEngine = nil
    preparationTask = nil
    resultsTask = nil
    finishingTask = nil
    pressing = false
    cancelTargeted = false
    sessionGeneration &+= 1
  }

  private func requestMicrophonePermission() async -> Bool {
    switch AVAudioApplication.shared.recordPermission {
    case .granted: return true
    case .denied: return false
    case .undetermined:
      return await withCheckedContinuation { continuation in
        AVAudioApplication.requestRecordPermission { granted in
          continuation.resume(returning: granted)
        }
      }
    @unknown default: return false
    }
  }

  private func requestSpeechPermission() async -> Bool {
    switch SFSpeechRecognizer.authorizationStatus() {
    case .authorized: return true
    case .denied, .restricted: return false
    case .notDetermined:
      return await withCheckedContinuation { continuation in
        SFSpeechRecognizer.requestAuthorization { status in
          continuation.resume(returning: status == .authorized)
        }
      }
    @unknown default: return false
    }
  }

  private static func join(_ first: String, _ second: String) -> String {
    [first, second]
      .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
      .joined(separator: " ")
  }

  private static func failureMessage(_ error: Error) -> String {
    if let error = error as? ChatVoiceInputError { return error.localizedDescription }
    return "Voice input stopped. Your recognized words remain in the composer."
  }
}

private enum ChatVoiceInputError: LocalizedError, Equatable {
  case microphonePermission
  case speechPermission
  case unsupportedLocale
  case audioUnavailable

  var errorDescription: String? {
    switch self {
    case .microphonePermission:
      "Allow microphone access in Settings to use voice input."
    case .speechPermission:
      "Allow speech recognition in Settings to use voice input."
    case .unsupportedLocale:
      "Voice input is not available for the current device language."
    case .audioUnavailable:
      "No microphone audio format is available for voice input."
    }
  }
}

private final class ChatVoiceAudioConverter: @unchecked Sendable {
  private let converter: AVAudioConverter
  private let outputFormat: AVAudioFormat
  private let lock = NSLock()

  init(inputFormat: AVAudioFormat, outputFormat: AVAudioFormat) throws {
    guard let converter = AVAudioConverter(from: inputFormat, to: outputFormat) else {
      throw ChatVoiceInputError.audioUnavailable
    }
    self.converter = converter
    self.outputFormat = outputFormat
  }

  func convert(_ buffer: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer {
    lock.lock()
    defer { lock.unlock() }

    let ratio = outputFormat.sampleRate / buffer.format.sampleRate
    let capacity = AVAudioFrameCount(ceil(Double(buffer.frameLength) * ratio)) + 1
    guard let output = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity) else {
      throw ChatVoiceInputError.audioUnavailable
    }

    var suppliedInput = false
    var conversionError: NSError?
    let status = converter.convert(to: output, error: &conversionError) { _, inputStatus in
      guard !suppliedInput else {
        inputStatus.pointee = .noDataNow
        return nil
      }
      suppliedInput = true
      inputStatus.pointee = .haveData
      return buffer
    }
    if let conversionError { throw conversionError }
    guard status == .haveData || (status == .inputRanDry && output.frameLength > 0) else {
      throw ChatVoiceInputError.audioUnavailable
    }
    return output
  }
}
