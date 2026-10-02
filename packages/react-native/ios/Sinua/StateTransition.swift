
/// How long a loadout change takes (design note 25), seconds.
let wearSeconds = 0.35

/// State transitions, caller side -- the Swift mirror of `@sinua/core`'s
/// `StateTransition` (packages/core/src/transition.ts; docs/fx-spec.md,
/// *Transitions*). The engine says what to draw at an instant (`transitionMix`);
/// this keeps the clock and what was on screen, so a change mid-transition starts
/// from there.
struct StateTransition {
    private var from: TransitionSide?
    /// What was on screen when the loadout last changed, while its change runs.
    private var wearFrom: TransitionSide?
    private var wearAge = Double.infinity
    private var shown: TransitionSide?
    private var age = Double.infinity
    /// Seconds since the last state change (cut or not); infinite before the first.
    private var since = Double.infinity
    private var duration = 0.0
    private var curve = "easeInOut"

    /// A state change happened: animate from what is on screen now. `duration` 0 = a cut.
    mutating func start(duration: Double, curve: String) {
        from = shown
        // A change of something already on screen; the first state isn't a change.
        if shown != nil { since = 0 }
        self.duration = max(0, duration)
        self.curve = curve
        age = from != nil && self.duration > 0 ? 0 : .infinity
    }

    /// The loadout changed (design note 25): the character eases from what is on screen now
    /// (a hat pops in, colours blend, a new eye style swaps in a blink), on its own clock, so a
    /// state change in the middle of it doesn't cut it, and the reverse.
    mutating func wear() {
        guard let shown else { return }
        wearFrom = shown
        wearAge = 0
    }

    /// A loadout change is easing in.
    var wearing: Bool { wearFrom != nil }

    /// Stop any transition now (reduced motion, a new design).
    mutating func cancel() {
        age = .infinity
        from = nil
        wearAge = .infinity
        wearFrom = nil
    }

    /// No transition running: remember `to` as what is on screen.
    mutating func settle(_ to: TransitionSide) {
        if !active { shown = to }
    }

    mutating func advance(_ dt: Double) {
        age += max(0, dt)
        since += max(0, dt)
        wearAge += max(0, dt)
        if wearAge >= wearSeconds { wearFrom = nil }
    }

    /// Seconds since the lifecycle state last changed, or nil before the first change.
    /// The frames carry it as the `stateAge` runtime key (a character blinks at the end
    /// of the user's turn); other patterns ignore it.
    var stateAge: Double? { since.isFinite ? since : nil }

    var active: Bool { from != nil && age < duration }

    private func mix(_ to: TransitionSide, size: UInt32) -> TransitionMix? {
        guard let from, active else { return nil }
        return transitionMix(from: from, to: to, size: size, progress: age / duration, curve: curve)
    }

    /// The speed multiplier to run the phase at now, for `to`.
    func speed(_ to: TransitionSide, size: UInt32) -> Double { mix(to, size: size)?.speed ?? to.speed }

    /// The frames for `to` at engine time `t`: `previous` dissolved into `frame` at `blend`.
    /// `extra` is the live runtime keys (audio, pointer), spread over both sides.
    mutating func frames(_ to: TransitionSide, size: UInt32, t: Double, extra live: [String: Double])
        -> (frame: OrbFrame?, previous: OrbFrame?, blend: Double)
    {
        var extra = live
        if since.isFinite { extra["stateAge"] = since }
        let wearFrom = wearFrom
        let wearW = wearAge / wearSeconds
        func draw(_ s: TransitionSide) -> OrbFrame? {
            let o = s.overrides.merging(extra) { $1 }
            // A loadout change in progress: the new side easing from the old one (the engine
            // answers only for two loadouts of one character; anything else draws plain).
            if let w = wearFrom,
                let f = frameTransitionWithOverrides(
                    from: TransitionSide(state: w.state, speed: w.speed, overrides: w.overrides.merging(extra) { $1 }),
                    to: TransitionSide(state: s.state, speed: s.speed, overrides: o), size: size, t: t, blend: wearW)
            {
                return f
            }
            return frameWithOverrides(state: s.state, size: size, t: t, overrides: o)
        }
        guard let from, let m = mix(to, size: size) else {
            shown = to
            return (draw(to), nil, 1)
        }
        switch m.technique {
        case "params":
            let base = TransitionSide(state: to.state, speed: m.speed, overrides: m.overrides)
            let swapped = TransitionSide(
                state: to.state, speed: m.speed, overrides: m.overrides.merging(m.structuralTo) { $1 })
            let structural = !m.structuralTo.isEmpty
            shown = structural && m.swap >= 0.5 ? swapped : base
            if !structural || m.swap <= 0 { return (draw(base), nil, 1) }
            if m.swap >= 1 { return (draw(swapped), nil, 1) }
            return (draw(swapped), draw(base), m.swap)
        case "morph":
            shown = to
            func withExtra(_ s: TransitionSide) -> TransitionSide {
                TransitionSide(state: s.state, speed: s.speed, overrides: s.overrides.merging(extra) { $1 })
            }
            if let f = frameTransitionWithOverrides(
                from: withExtra(from), to: withExtra(to), size: size, t: t, blend: m.weight)
            {
                return (f, nil, 1)
            }
            return (draw(to), draw(from), m.weight)
        default:
            shown = to
            return (draw(to), draw(from), m.weight)
        }
    }
}
