import SwiftUI
import UIKit

// The RN SinuaView's native half on iOS (docs/fx-view.md, *React Native*): a
// UIView that hosts packages/ios's SwiftUI `SinuaView` unchanged in a
// UIHostingController -- Callstack's "Exposing SwiftUI Views to React Native"
// pattern. SinuaViewComponentView.mm (the Fabric view) owns one and pushes
// every prop change through `apply(...)`; the props live in an
// ObservableObject, so SwiftUI re-renders in place (no remount, the clock
// keeps running). SinuaView and SinuaVoice are compiled into this pod by
// build.sh (copied from packages/ios), like CoreEngine.

final class FxHostModel: ObservableObject {
    @Published var spec: String?
    @Published var state = "working"
    @Published var size: UInt32 = 64
    @Published var overrides: [String: Double] = [:]
    @Published var speed = 1.0
    @Published var specState: String?
    @Published var inputs: [String: Double] = [:]
    @Published var voiceLevelInput: String?
    /// nil = the spec's `transitions` (default 0.6 s).
    @Published var crossFade: Double?
    @Published var theme: FxTheme = .auto
    @Published var paused = false
    @Published var reducedMotion: FxReducedMotion = .auto
    @Published var maxFps: Double?
    @Published var lowPower: FxLowPower = .auto
    @Published var label: String?
    @Published var voice: VoiceSource?
    /// Set when `audioStrength` asks for a pulse other than the view's default: this view's
    /// own tracker on the source's fan-out, used instead of `voice`.
    @Published var voiceOverrides: VoiceOverrides?
    @Published var labels: [String: String] = [:]
    @Published var announce: Bool?
    @Published var haptics = false
    @Published var rules = true
    @Published var tap = false
    @Published var expression: String?
    @Published var palette: [String: String] = [:]
    @Published var loadout: SinuaLoadout?
    @Published var effect: SinuaEffectTrigger?
    /// The app's real activity. SinuaView runs only while `scenePhase == .active`,
    /// and a UIHostingController inside a scene-less UIKit app (React Native's
    /// AppDelegate + window) never gets an active `scenePhase` from SwiftUI --
    /// so the host feeds it from UIApplication's notifications instead.
    @Published var phase: ScenePhase = UIApplication.shared.applicationState == .background ? .background : .active
    var onFrame: ((FxFrameStats) -> Void)?
}

struct FxHostContent: View {
    @ObservedObject var model: FxHostModel

    var body: some View {
        content.environment(\.scenePhase, model.phase)
    }

    @ViewBuilder private var content: some View {
        let frame: ((FxFrameStats) -> Void)? = model.onFrame
        if let spec = model.spec, !spec.isEmpty {
            SinuaView(spec: spec, voice: model.voice, voiceOverrides: model.voiceOverrides, state: model.specState,
                   inputs: model.inputs,
                   voiceLevelInput: model.voiceLevelInput, crossFade: model.crossFade, theme: model.theme,
                   paused: model.paused, reducedMotion: model.reducedMotion, accessibilityLabel: model.label,
                   maxFps: model.maxFps, lowPower: model.lowPower, onFrame: frame,
                   labels: model.labels, announce: model.announce, haptics: model.haptics, rules: model.rules,
                   effect: model.effect, tap: model.tap, expression: model.expression,
                   palette: model.palette, loadout: model.loadout)
        } else {
            SinuaView(pattern: model.state, size: model.size, overrides: model.overrides, speed: model.speed,
                   state: model.specState, voice: model.voice, voiceOverrides: model.voiceOverrides,
                   theme: model.theme, paused: model.paused, reducedMotion: model.reducedMotion,
                   accessibilityLabel: model.label, maxFps: model.maxFps, lowPower: model.lowPower, onFrame: frame,
                   labels: model.labels, announce: model.announce, haptics: model.haptics, effect: model.effect,
                   tap: model.tap, expression: model.expression,
                   palette: model.palette)
        }
    }
}

@objc(SinuaHostView)
public final class FxHostView: UIView {
    private let model = FxHostModel()
    private var host: UIHostingController<FxHostContent>?
    private var voiceMode = "none"
    /// A source the app created and owns: bound, never connected or disconnected here.
    private var boundSourceId: String?
    private var lastEffectKey = 0
    private var lastFrameEvent: CFTimeInterval = 0

    /// dtMs, computeMs, paintMs -- at most 4 per second, only while `reportFrames`.
    @objc public var onFrameStats: ((Double, Double, Double) -> Void)?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        let h = UIHostingController(rootView: FxHostContent(model: model))
        h.view.backgroundColor = .clear
        h.view.translatesAutoresizingMaskIntoConstraints = false
        addSubview(h.view)
        NSLayoutConstraint.activate([
            h.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            h.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            h.view.topAnchor.constraint(equalTo: topAnchor),
            h.view.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        host = h
        let nc = NotificationCenter.default
        let m = model
        observers = [
            nc.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { _ in m.phase = .active },
            nc.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { _ in m.phase = .inactive },
            nc.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in m.phase = .background },
        ]
    }

    private var observers: [NSObjectProtocol] = []

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
        // Only a source this view created (the `voice` shorthands) is disconnected here.
        if boundSourceId == nil { model.voice?.disconnect() }
    }

    private static func map(_ json: String?) -> [String: Double] {
        guard let json, let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return obj.compactMapValues { ($0 as? NSNumber)?.doubleValue }
    }

    /// Every Fabric prop at once (one SwiftUI update per React commit).
    @objc public func apply(spec: String?, state: String?, size: Int, overridesJson: String?, speed: Double,
                            specState: String?, inputsJson: String?, voiceLevelInput: String?, crossFade: Double,
                            audioStrength: Double, voice: String, voiceSourceId: String?, theme: String, paused: Bool, reducedMotion: String, maxFps: Double,
                            lowPower: String, label: String?, reportFrames: Bool, labelsJson: String?, announce: String,
                            haptics: Bool, rules: Bool, effectName: String?, effectKey: Int, tap: Bool,
                            expression: String?, paletteJson: String?, loadoutJson: String?) {
        let m = model
        m.spec = (spec?.isEmpty ?? true) ? nil : spec
        m.state = (state?.isEmpty ?? true) ? "working" : state!
        m.size = UInt32([20, 32, 64].contains(size) ? size : 64)
        m.overrides = Self.map(overridesJson)
        m.speed = speed
        m.specState = (specState?.isEmpty ?? true) ? nil : specState
        m.inputs = Self.map(inputsJson)
        m.voiceLevelInput = (voiceLevelInput?.isEmpty ?? true) ? nil : voiceLevelInput
        m.crossFade = crossFade >= 0 ? crossFade : nil
        m.theme = theme == "light" ? .light : theme == "dark" ? .dark : .auto
        m.paused = paused
        m.reducedMotion = reducedMotion == "always" ? .always : reducedMotion == "never" ? .never : .auto
        m.maxFps = maxFps > 0 ? maxFps : nil
        m.lowPower = lowPower == "on" ? .on : lowPower == "off" ? .off : .auto
        m.label = (label?.isEmpty ?? true) ? nil : label
        let words = (labelsJson?.data(using: .utf8)).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        m.labels = words?.compactMapValues { $0 as? String } ?? [:]
        m.announce = announce == "on" ? true : announce == "off" ? false : nil
        m.haptics = haptics
        m.rules = rules
        m.tap = tap
        m.expression = (expression?.isEmpty ?? true) ? nil : expression
        let pal = (paletteJson?.data(using: .utf8)).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        m.palette = pal?.compactMapValues { $0 as? String } ?? [:]
        let lo = (loadoutJson?.isEmpty ?? true) ? nil : loadoutJson?.data(using: .utf8)
        let loadout = lo.flatMap { try? JSONDecoder().decode(SinuaLoadout.self, from: $0) }
        if m.loadout != loadout { m.loadout = loadout }
        // A new key plays the effect once (a fresh trigger value).
        if effectKey != lastEffectKey {
            lastEffectKey = effectKey
            if effectKey != 0, let name = effectName, let kind = SinuaEffectTrigger.Kind(rawValue: name) {
                m.effect = SinuaEffectTrigger(kind)
            }
        }
        m.onFrame = reportFrames ? { [weak self] s in self?.frame(s) } : nil
        // A source created in JS (src/voice.ts) is bound by id and owned by the app;
        // the `voice` shorthands stay owned by this view.
        let boundId = (voiceSourceId?.isEmpty ?? true) ? nil : voiceSourceId
        if boundId != self.boundSourceId {
            self.boundSourceId = boundId
            if voiceMode != "none" { setVoice("none") }
            model.voice = boundId.flatMap { VoiceRegistry.shared.source(id: $0) }
        }
        if boundId == nil, voice != voiceMode { setVoice(voice) }
        retrack(audioStrength: audioStrength)
    }

    private var tracked: SharedVoiceSource.Tracked?
    private var trackedFor: (ObjectIdentifier, Double)?

    /// `audioStrength` >= 0 (the voice button's ring): this view's own tracker with that pulse.
    private func retrack(audioStrength: Double) {
        guard audioStrength >= 0, let v = model.voice else {
            tracked = nil
            trackedFor = nil
            if model.voiceOverrides != nil { model.voiceOverrides = nil }
            return
        }
        if let t = trackedFor, t.0 == ObjectIdentifier(v), t.1 == audioStrength { return }
        var options = VoiceOverridesOptions()
        options.audioStrength = audioStrength
        let t = SharedVoiceSource.of(v).track(options: options)
        tracked = t
        trackedFor = (ObjectIdentifier(v), audioStrength)
        model.voiceOverrides = t.overrides
    }

    private func frame(_ s: FxFrameStats) {
        let now = CACurrentMediaTime()
        guard now - lastFrameEvent >= 0.25 else { return }
        lastFrameEvent = now
        onFrameStats?(s.dtMs, s.computeMs, s.paintMs)
    }

    /// SinuaView draws a voice but doesn't own it: the host connects and disconnects.
    private func setVoice(_ mode: String) {
        voiceMode = mode
        model.voice?.disconnect()
        model.voice = nil
        let source: VoiceSource? = mode == "test" ? TestToneVoiceSource() : mode == "mic" ? LocalMicVoiceSource() : nil
        guard let source else { return }
        model.voice = source
        Task { @MainActor [weak self] in
            do { try await source.connect() } catch {
                if self?.model.voice === source { self?.model.voice = nil }
            }
        }
    }
}
