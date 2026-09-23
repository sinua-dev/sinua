import Foundation

/// The native voice sources a React Native app can create (src/voice.ts), kept by
/// id so a view can bind one with the `voiceSourceId` prop and several views can
/// share it. The app owns the lifecycle: nothing here connects on its own.
///
/// Vendors are opt-in per build. The Gemini and ElevenLabs glue is Foundation-only
/// and always present; LiveKit and OpenAI Realtime come from the `SinuaCore/LiveKit`
/// and `SinuaCore/OpenAI` subspecs, which also define SINUA_LIVEKIT / SINUA_OPENAI.
/// Without the subspec, `create` fails with what to add -- nothing is linked silently.
///
/// Credentials live only in the source they are handed to: never stored, never logged.
@MainActor
final class VoiceRegistry {
    static let shared = VoiceRegistry()

    enum RegistryError: LocalizedError {
        case unknownVendor(String)
        case vendorNotInstalled(vendor: String, subspec: String)
        case missingField(vendor: String, field: String)
        case unknownSource(String)

        var errorDescription: String? {
            switch self {
            case .unknownVendor(let v): return "unknown voice vendor \"\(v)\""
            case .vendorNotInstalled(let v, let s):
                return "the \(v) voice source isn't in this build: add `pod 'SinuaCore/\(s)'` (and LiveKit's podspecs source) to your Podfile, then pod install"
            case .missingField(let v, let f): return "\(v) needs \(f)"
            case .unknownSource(let id): return "no voice source \(id) (already released?)"
            }
        }
    }

    private var sources: [String: VoiceSource] = [:]
    /// Waiting `getCredential` round trips (OpenAI): requestId -> continuation.
    private var pending: [String: CheckedContinuation<String, Error>] = [:]

    /// Emitted state / error / interrupt / credentialRequest; the module forwards them to JS.
    var onEvent: ((_ id: String, _ event: String, _ payload: [String: Any]) -> Void)?

    func source(id: String) -> VoiceSource? { sources[id] }

    func create(config: [String: Any]) throws {
        guard let id = config["id"] as? String, let vendor = config["vendor"] as? String else {
            throw RegistryError.missingField(vendor: "voice", field: "id and vendor")
        }
        let source = try make(vendor: vendor, id: id, config: config)
        source.onStateChange { [weak self] state in
            self?.onEvent?(id, "state", ["state": state.rawValue])
        }
        source.onInterrupt { [weak self] in self?.onEvent?(id, "interrupt", [:]) }
        sources[id] = source
    }

    func connect(id: String) async throws {
        guard let source = sources[id] else { throw RegistryError.unknownSource(id) }
        try await source.connect()
    }

    func disconnect(id: String) {
        sources[id]?.disconnect()
    }

    func release(id: String) {
        sources[id]?.disconnect()
        sources[id] = nil
    }

    // MARK: - getCredential round trip (OpenAI: an `ek_` is single-use, so every reconnect asks again)

    func requestCredential(id: String) async throws -> String {
        let requestId = UUID().uuidString
        return try await withCheckedThrowingContinuation { cont in
            pending[requestId] = cont
            onEvent?(id, "credentialRequest", ["requestId": requestId])
        }
    }

    func provideCredential(requestId: String, credential: String?, error: String?) {
        guard let cont = pending.removeValue(forKey: requestId) else { return }
        if let credential, !credential.isEmpty {
            cont.resume(returning: credential)
        } else {
            cont.resume(throwing: RegistryError.missingField(vendor: "openai", field: error ?? "a credential"))
        }
    }

    // MARK: - Vendors

    private func make(vendor: String, id: String, config: [String: Any]) throws -> VoiceSource {
        let string = { (key: String) -> String? in (config[key] as? String).flatMap { $0.isEmpty ? nil : $0 } }
        let allowInsecureApiKey = config["allowInsecureApiKey"] as? Bool ?? false
        switch vendor {
        case "test": return TestToneVoiceSource()
        case "mic": return LocalMicVoiceSource()
        case "gemini":
            guard let credential = string("credential") else { throw RegistryError.missingField(vendor: "gemini", field: "a credential") }
            return GeminiLiveVoiceSource(credential: credential,
                                         model: string("model") ?? GeminiLiveSession.defaultModel,
                                         instructions: string("instructions"),
                                         allowInsecureApiKey: allowInsecureApiKey,
                                         endpoint: string("endpoint").flatMap(URL.init(string:)).map { GeminiLiveSession.Endpoint(url: $0, headers: [:]) })
        case "elevenlabs":
            guard let credential = string("credential") else { throw RegistryError.missingField(vendor: "elevenlabs", field: "an agent id or signed URL") }
            return ElevenLabsVoiceSource(credential: credential, endpoint: string("endpoint").flatMap { URL(string: $0) })
        case "livekit":
            #if SINUA_LIVEKIT
            guard let url = string("url"), let token = string("token") else {
                throw RegistryError.missingField(vendor: "livekit", field: "a url and token")
            }
            return LiveKitVoiceSource(url: url, token: token, publishMicrophone: config["publishMicrophone"] as? Bool ?? true)
            #else
            throw RegistryError.vendorNotInstalled(vendor: "LiveKit", subspec: "LiveKit")
            #endif
        case "openai":
            #if SINUA_OPENAI
            if config["hasCredentialProvider"] as? Bool == true {
                return OpenAIRealtimeVoiceSource(credentialProvider: { [weak self] in
                    guard let self else { throw RegistryError.unknownSource(id) }
                    return try await self.requestCredential(id: id)
                })
            }
            guard let credential = string("credential") else {
                throw RegistryError.missingField(vendor: "openai", field: "a credential or getCredential")
            }
            return OpenAIRealtimeVoiceSource(credential: credential,
                                             model: string("model") ?? OpenAIRealtimeSignaling.defaultModel,
                                             voice: string("voice") ?? OpenAIRealtimeSignaling.defaultVoice,
                                             instructions: string("instructions"),
                                             allowInsecureApiKey: allowInsecureApiKey)
            #else
            throw RegistryError.vendorNotInstalled(vendor: "OpenAI Realtime", subspec: "OpenAI")
            #endif
        default:
            throw RegistryError.unknownVendor(vendor)
        }
    }
}
