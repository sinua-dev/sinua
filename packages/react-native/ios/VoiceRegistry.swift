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
                return
                    "the \(v) voice source isn't in this build: add `pod 'SinuaCore/\(s)'` (and LiveKit's podspecs source) to your Podfile, then pod install"
            case .missingField(let v, let f): return "\(v) needs \(f)"
            case .unknownSource(let id): return "no voice source \(id) (already released?)"
            }
        }
    }

    /// Each source behind its fan-out: the JS events and every view bound by id listen side
    /// by side (a source holds one callback of each kind).
    private var sources: [String: SharedVoiceSource] = [:]
    /// Waiting credential round trips (a JS provider or `credentialUrl`): requestId -> continuation.
    private var pending: [String: CheckedContinuation<SinuaCredential, Error>] = [:]

    /// Emitted state / error / interrupt / credentialRequest; the module forwards them to JS.
    var onEvent: ((_ id: String, _ event: String, _ payload: [String: Any]) -> Void)?

    func source(id: String) -> VoiceSource? { sources[id] }

    func create(config: [String: Any]) throws {
        guard let id = config["id"] as? String, let vendor = config["vendor"] as? String else {
            throw RegistryError.missingField(vendor: "voice", field: "id and vendor")
        }
        let source = SharedVoiceSource.of(try make(vendor: vendor, id: id, config: config))
        source.listenState { [weak self] state in
            self?.onEvent?(id, "state", ["state": state.rawValue])
        }
        source.listenInterrupt { [weak self] in self?.onEvent?(id, "interrupt", [:]) }
        source.listenConnection { [weak self] up in self?.onEvent?(id, "connection", ["connected": up]) }
        source.listenMute { [weak self] muted in self?.onEvent?(id, "mute", ["muted": muted]) }
        // Always forwarded (JS keeps the listeners), so a JS listener added before connect() sees the first turn.
        source.listenTranscript { [weak self] u in
            var payload: [String: Any] = [
                "role": u.role.rawValue, "text": u.text, "final": u.final, "turnId": u.turnId, "truncated": u.truncated,
            ]
            if let s = u.startMs { payload["startMs"] = s }
            if let e = u.endMs { payload["endMs"] = e }
            self?.onEvent?(id, "transcript", payload)
        }
        sources[id] = source
    }

    /// Mutes the microphone (silence goes out, the session stays up); views bound by id show the cue.
    func setMuted(id: String, muted: Bool) {
        sources[id]?.setMuted(muted)
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

    // MARK: - Credential round trip (JS resolves `credential` providers and `credentialUrl`)

    func requestCredential(id: String) async throws -> SinuaCredential {
        let requestId = UUID().uuidString
        return try await withCheckedThrowingContinuation { cont in
            pending[requestId] = cont
            onEvent?(id, "credentialRequest", ["requestId": requestId])
        }
    }

    func provideCredential(requestId: String, credential: String?, url: String?, error: String?, fatal: Bool) {
        guard let cont = pending.removeValue(forKey: requestId) else { return }
        if let credential, !credential.isEmpty {
            cont.resume(returning: SinuaCredential(credential: credential, url: url.flatMap { $0.isEmpty ? nil : $0 }))
        } else {
            let message = error ?? "the credential provider returned nothing"
            cont.resume(throwing: fatal ? CredentialError.fatal(message) : CredentialError.retryable(message))
        }
    }

    /// The config's credential as the shared `CredentialSource`: a JS provider or
    /// `credentialUrl` (`hasCredentialProvider`) is a round trip per (re)connect;
    /// otherwise the fixed `credential` string.
    private func credentialSource(id: String, config: [String: Any], missing: (vendor: String, field: String)) throws
        -> CredentialSource
    {
        if config["hasCredentialProvider"] as? Bool == true {
            return .provider { [weak self] in
                guard let self else { throw RegistryError.unknownSource(id) }
                return try await self.requestCredential(id: id)
            }
        }
        guard let credential = (config["credential"] as? String).flatMap({ $0.isEmpty ? nil : $0 }) else {
            throw RegistryError.missingField(vendor: missing.vendor, field: missing.field)
        }
        return .value(credential)
    }

    // MARK: - Vendors

    private func make(vendor: String, id: String, config: [String: Any]) throws -> VoiceSource {
        let string = { (key: String) -> String? in (config[key] as? String).flatMap { $0.isEmpty ? nil : $0 } }
        switch vendor {
        case "test": return TestToneVoiceSource()
        case "simulated":
            let loop = config["loop"] as? Bool
            if let script = string("script") { return try SimulatedVoiceSource(script: script, loop: loop) }
            return try SimulatedVoiceSource(sample: string("sample") ?? "calendar", loop: loop)
        case "mic": return LocalMicVoiceSource()
        case "gemini":
            return GeminiLiveVoiceSource(
                credential: try credentialSource(
                    id: id, config: config, missing: ("gemini", "a credential or credentialUrl")),
                model: string("model") ?? GeminiLiveSession.defaultModel,
                instructions: string("instructions"),
                endpoint: string("endpoint").flatMap(URL.init(string:)).map {
                    GeminiLiveSession.Endpoint(url: $0, headers: [:])
                })
        case "elevenlabs":
            return ElevenLabsVoiceSource(
                credential: try credentialSource(
                    id: id, config: config, missing: ("elevenlabs", "an agent id, a signed URL, or credentialUrl")),
                endpoint: string("endpoint").flatMap { URL(string: $0) })
        case "livekit":
            #if SINUA_LIVEKIT
                let publish = config["publishMicrophone"] as? Bool ?? true
                if let url = string("url"), let token = string("token") {
                    return LiveKitVoiceSource(url: url, token: token, publishMicrophone: publish)
                }
                guard config["hasCredentialProvider"] as? Bool == true else {
                    throw RegistryError.missingField(vendor: "livekit", field: "a url and token, or credentialUrl")
                }
                return LiveKitVoiceSource(
                    credential: try credentialSource(id: id, config: config, missing: ("livekit", "credentialUrl")),
                    publishMicrophone: publish)
            #else
                throw RegistryError.vendorNotInstalled(vendor: "LiveKit", subspec: "LiveKit")
            #endif
        case "openai":
            #if SINUA_OPENAI
                return OpenAIRealtimeVoiceSource(
                    credential: try credentialSource(
                        id: id, config: config, missing: ("openai", "a credential or credentialUrl")))
            #else
                throw RegistryError.vendorNotInstalled(vendor: "OpenAI Realtime", subspec: "OpenAI")
            #endif
        default:
            throw RegistryError.unknownVendor(vendor)
        }
    }
}
