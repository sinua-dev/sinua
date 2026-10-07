import Foundation

/// The vendor-free half of the transcript API (docs/audio-pipeline.md, *Transcripts*): a
/// port of packages/voice/src/transcript.ts, held to the same table
/// (spec/transcript-cases.json). A source feeds it each vendor fragment and its 30 Hz level
/// tick; it answers with cumulative `TranscriptUpdate`s. Main thread; the rules are in the
/// Web file's header.
public final class TranscriptAssembler {
    public static let userSilenceMs = 4000.0
    public static let userCloseChars = 12
    public static let revealCharsPerSecond = 14.0
    public static let cutGraceMs = 150.0
    /// `segments`: GPT-Live's text timeline runs ahead of the played audio; the reveal waits this
    /// long behind it (measured live, design note 39).
    public static let segmentDelayMs = 300.0
    /// A barge-in cut: left-over text starting at least this long after the fragment before it (on
    /// the vendor's timeline) is the model's next utterance; it opens the next assistant turn.
    public static let newUtteranceGapMs = 600.0
    /// The level above which the assistant's audio counts as audible (the sessions' speaking level).
    public static let audibleLevel = 0.05
    /// A tick gap longer than this doesn't count as speaking time.
    static let maxTickMs = 100.0

    private struct Fragment {
        var text: String
        var length: Int
        var startMs: Double?
        var endMs: Double?
    }

    private final class Turn {
        let id: String
        var fragments: [Fragment] = []
        var length = 0
        var shown = 0
        var lastAt: Double
        var onset: Double?
        var anchor: Double?
        var spoken = 0.0
        let assistantAtOpen: Int

        init(id: String, lastAt: Double, assistantAtOpen: Int) {
            self.id = id
            self.lastAt = lastAt
            self.assistantAtOpen = assistantAtOpen
        }
    }

    public var onUpdate: ((TranscriptUpdate) -> Void)?
    public let timing: TranscriptTiming
    /// Reveal assistant text with the audio (`syncToAudio`); else raw.
    public let sync: Bool

    private var user: Turn?
    private var assistant: Turn?
    private var userCount = 0
    private var assistantCount = 0
    private var pendingOnset: Double?
    private var lastAudible: Double?
    private var lastTick: Double?
    private var held = false

    public init(timing: TranscriptTiming, sync: Bool) {
        self.timing = timing
        self.sync = sync
    }

    /// A user fragment (ASR delta), at `now` ms.
    public func userDelta(_ text: String, now: Double, startMs: Double? = nil, endMs: Double? = nil) {
        guard !text.isEmpty else { return }
        let u = user ?? open(.user, now)
        user = u
        append(u, text, now, startMs, endMs)
        u.shown = u.length
        emit(.user, u, final: false)
    }

    /// An assistant fragment, at `now` ms.
    public func assistantDelta(_ text: String, now: Double, startMs: Double? = nil, endMs: Double? = nil) {
        guard !text.isEmpty else { return }
        let a: Turn
        if let existing = assistant {
            a = existing
        } else {
            a = open(.assistant, now)
            a.onset = pendingOnset
            assistant = a
        }
        append(a, text, now, startMs, endMs)
        if a.anchor == nil, let s = startMs { a.anchor = s }
        if let u = user, assistantCount > u.assistantAtOpen, a.length >= Self.userCloseChars { finish(.user) }
        reveal(a, now)
    }

    /// The user started talking over the assistant's audio: no reveal past the last audible
    /// moment until the audio is heard again.
    public func hold() {
        if assistant != nil { held = true }
    }

    /// 30 Hz: the assistant audio's level and whether the source is in `speaking`, at `now` ms.
    public func tick(now: Double, level: Double, speaking: Bool) {
        let dt = lastTick.map { min(Self.maxTickMs, max(0, now - $0)) } ?? 0
        lastTick = now
        let audible = speaking && level > Self.audibleLevel
        if audible {
            lastAudible = now
            held = false
            if let a = assistant {
                if a.onset == nil { a.onset = now }
                a.spoken += dt / 1000
            } else if pendingOnset == nil {
                pendingOnset = now
            }
        } else if !speaking {
            pendingOnset = nil
        }
        if let u = user, now - u.lastAt >= Self.userSilenceMs { finish(.user) }
        if let a = assistant {
            if a.onset == nil, !speaking, now - a.lastAt >= Self.userSilenceMs {
                finish(.assistant)
            } else {
                reveal(a, now)
            }
        }
    }

    /// The source left `speaking` on its own: the assistant turn ends.
    public func speakingEnded() {
        guard let a = assistant, a.onset != nil else { return }
        finish(.assistant)
    }

    /// A barge-in cut the assistant: its turn ends with what was audible by the last audible moment.
    public func cut() {
        guard let a = assistant else { return }
        var n = 0
        if let at = lastAudible, let onset = a.onset, at >= onset {
            if !sync, let anchor = a.anchor {
                let vendorAt = anchor + (at - onset - Self.segmentDelayMs)
                for f in a.fragments {
                    guard let end = f.endMs, end <= vendorAt else { break }
                    n += f.length
                }
            } else {
                n = min(a.shown, visible(a, now: at, audibleAt: at))
            }
        }
        a.shown = n
        assistant = nil
        pendingOnset = nil
        held = false
        emitText(.assistant, a, Self.trim(Self.prefix(a.fragments, n)), final: true, truncated: true)
        // What follows the kept text: the unspoken tail (dropped), or, from the first fragment that
        // starts a gap after the one before it, the next utterance.
        var kept = 0
        var i = 0
        while i < a.fragments.count, kept < n {
            kept += a.fragments[i].length
            i += 1
        }
        let fs = a.fragments
        guard
            let next = fs.indices.first(where: { k in
                guard k >= max(i, 1), let s = fs[k].startMs, let e = fs[k - 1].endMs else { return false }
                return s - e >= Self.newUtteranceGapMs
            })
        else { return }
        let t = open(.assistant, a.lastAt)
        for f in fs[next...] { append(t, f.text, a.lastAt, f.startMs, f.endMs) }
        t.anchor = t.fragments.first?.startMs
        assistant = t
        if let u = user, assistantCount > u.assistantAtOpen, t.length >= Self.userCloseChars { finish(.user) }
    }

    /// The session ended: open turns end as they are.
    public func stop() {
        if user != nil { finish(.user) }
        if let a = assistant {
            assistant = nil
            emitText(.assistant, a, Self.trim(Self.prefix(a.fragments, a.shown)), final: true, truncated: false)
        }
        pendingOnset = nil
        lastAudible = nil
        lastTick = nil
        held = false
    }

    private func open(_ role: TranscriptUpdate.Role, _ now: Double) -> Turn {
        let id: String
        if role == .user {
            userCount += 1
            id = "u\(userCount)"
        } else {
            assistantCount += 1
            id = "a\(assistantCount)"
        }
        return Turn(id: id, lastAt: now, assistantAtOpen: assistantCount)
    }

    private func append(_ t: Turn, _ text: String, _ now: Double, _ startMs: Double?, _ endMs: Double?) {
        let norm = text.precomposedStringWithCanonicalMapping
        let length = norm.unicodeScalars.count
        t.fragments.append(Fragment(text: norm, length: length, startMs: startMs, endMs: endMs))
        t.length += length
        t.lastAt = now
    }

    private func visible(_ a: Turn, now: Double, audibleAt: Double?) -> Int {
        if !sync || timing == .synced || timing == .chars { return a.length }
        guard let onset = a.onset else { return 0 }
        if timing == .none { return min(a.length, Int((a.spoken * Self.revealCharsPerSecond).rounded(.down))) }
        guard let anchor = a.anchor else { return a.length }
        let heldAt = audibleAt.map { min(now, $0 + (held ? 0 : Self.cutGraceMs)) } ?? now
        let head = anchor + (heldAt - onset - Self.segmentDelayMs)
        var n = 0
        for f in a.fragments {
            guard let start = f.startMs, let end = f.endMs else {
                n += f.length
                continue
            }
            if head >= end {
                n += f.length
            } else {
                if head > start { n += Int((Double(f.length) * (head - start) / (end - start)).rounded(.down)) }
                break
            }
        }
        return n
    }

    private func reveal(_ a: Turn, _ now: Double) {
        let n = max(a.shown, min(a.length, visible(a, now: now, audibleAt: lastAudible)))
        guard n != a.shown else { return }
        a.shown = n
        emit(.assistant, a, final: false)
    }

    private func finish(_ role: TranscriptUpdate.Role) {
        guard let t = role == .user ? user : assistant else { return }
        if role == .user {
            user = nil
        } else {
            assistant = nil
            pendingOnset = nil
            held = false
        }
        t.shown = t.length
        emitText(role, t, Self.trim(Self.prefix(t.fragments, t.length)), final: true, truncated: false)
    }

    private func emit(_ role: TranscriptUpdate.Role, _ t: Turn, final: Bool) {
        emitText(role, t, Self.prefix(t.fragments, t.shown), final: final, truncated: false)
    }

    private func emitText(_ role: TranscriptUpdate.Role, _ t: Turn, _ text: String, final: Bool, truncated: Bool) {
        if !final, text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return }
        var seen = 0
        var end: Double?
        for f in t.fragments {
            if seen >= t.shown { break }
            seen += f.length
            if let e = f.endMs { end = e }
        }
        let start = t.shown > 0 ? t.fragments.first?.startMs : nil
        onUpdate?(
            TranscriptUpdate(
                role: role, text: text, final: final, turnId: t.id, truncated: truncated, startMs: start, endMs: end))
    }

    private static func prefix(_ fragments: [Fragment], _ n: Int) -> String {
        var out = String.UnicodeScalarView()
        var left = n
        for f in fragments {
            if left <= 0 { break }
            if f.length <= left {
                out.append(contentsOf: f.text.unicodeScalars)
                left -= f.length
            } else {
                out.append(contentsOf: f.text.unicodeScalars.prefix(left))
                left = 0
            }
        }
        return String(out)
    }

    private static func trim(_ s: String) -> String {
        s.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
