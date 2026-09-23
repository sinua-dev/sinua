import Foundation
import React

/// The React Native bridge for native voice sources (src/voice.ts). It only
/// forwards to `VoiceRegistry`: create / connect / disconnect / release, plus the
/// `getCredential` round trip, and streams the registry's events to JS as one
/// `sinua-voice` event carrying the source's id.
///
/// Credentials arrive in `create` (or per request) and go straight into the
/// source; this class keeps none of them and logs none of them.
@objc(SinuaVoice)
class SinuaVoice: RCTEventEmitter {
    private var listening = false

    override static func requiresMainQueueSetup() -> Bool { true }

    override func supportedEvents() -> [String] { ["sinua-voice"] }

    override func startObserving() {
        listening = true
        Task { @MainActor in
            VoiceRegistry.shared.onEvent = { [weak self] id, event, payload in
                guard let self, self.listening else { return }
                var body: [String: Any] = payload
                body["id"] = id
                body["event"] = event
                self.sendEvent(withName: "sinua-voice", body: body)
            }
        }
    }

    override func stopObserving() {
        listening = false
    }

    @objc(create:resolver:rejecter:)
    func create(_ config: NSDictionary, resolver resolve: @escaping RCTPromiseResolveBlock, rejecter reject: @escaping RCTPromiseRejectBlock) {
        let dict = config as? [String: Any] ?? [:]
        Task { @MainActor in
            do {
                try VoiceRegistry.shared.create(config: dict)
                resolve(dict["id"] as? String ?? "")
            } catch {
                reject("sinua_voice_create", error.localizedDescription, nil)
            }
        }
    }

    @objc(connect:resolver:rejecter:)
    func connect(_ id: String, resolver resolve: @escaping RCTPromiseResolveBlock, rejecter reject: @escaping RCTPromiseRejectBlock) {
        Task { @MainActor in
            do {
                try await VoiceRegistry.shared.connect(id: id)
                resolve(NSNull())
            } catch {
                // Failures after a successful connect go to JS as an `error` event instead.
                reject("sinua_voice_connect", error.localizedDescription, nil)
            }
        }
    }

    @objc(disconnect:)
    func disconnect(_ id: String) {
        Task { @MainActor in VoiceRegistry.shared.disconnect(id: id) }
    }

    @objc(release:)
    func release(_ id: String) {
        Task { @MainActor in VoiceRegistry.shared.release(id: id) }
    }

    @objc(provideCredential:credential:error:)
    func provideCredential(_ requestId: String, credential: NSString?, error: NSString?) {
        Task { @MainActor in
            VoiceRegistry.shared.provideCredential(requestId: requestId, credential: credential as String?, error: error as String?)
        }
    }
}
