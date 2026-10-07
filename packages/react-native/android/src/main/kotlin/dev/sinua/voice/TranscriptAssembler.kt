package dev.sinua.voice

import java.text.Normalizer
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min

/**
 * The vendor-free half of the transcript API (docs/audio-pipeline.md, *Transcripts*): a port of
 * packages/voice/src/transcript.ts, held to the same table (spec/transcript-cases.json). A
 * source feeds it each vendor fragment and its 30 Hz level tick; it answers with cumulative
 * [TranscriptUpdate]s. Main thread; the rules are in the Web file's header.
 */
class TranscriptAssembler(
    val timing: TranscriptTiming,
    /** Reveal assistant text with the audio (`syncToAudio`); else raw. */
    val sync: Boolean,
) {
    companion object {
        const val USER_SILENCE_MS = 4000.0
        const val USER_CLOSE_CHARS = 12
        const val REVEAL_CHARS_PER_SECOND = 14.0
        const val CUT_GRACE_MS = 150.0

        /**
         * `segments`: GPT-Live's text timeline runs ahead of the played audio; the reveal waits
         * this long behind it (measured live, design note 39).
         */
        const val SEGMENT_DELAY_MS = 300.0

        /**
         * A barge-in cut: left-over text starting at least this long after the fragment before it
         * (on the vendor's timeline) is the model's next utterance; it opens the next assistant turn.
         */
        const val NEW_UTTERANCE_GAP_MS = 600.0

        /** The level above which the assistant's audio counts as audible (the sessions' speaking level). */
        const val AUDIBLE_LEVEL = 0.05

        /** A tick gap longer than this doesn't count as speaking time. */
        private const val MAX_TICK_MS = 100.0

        private fun prefix(fragments: List<Fragment>, n: Int): String {
            val out = StringBuilder()
            var left = n
            for (f in fragments) {
                if (left <= 0) break
                if (f.length <= left) {
                    out.append(f.text)
                    left -= f.length
                } else {
                    out.append(f.text, 0, f.text.offsetByCodePoints(0, left))
                    left = 0
                }
            }
            return out.toString()
        }
    }

    private class Fragment(val text: String, val length: Int, val startMs: Double?, val endMs: Double?)

    private class Turn(val id: String, var lastAt: Double, val assistantAtOpen: Int) {
        val fragments = ArrayList<Fragment>()
        var length = 0
        var shown = 0
        var onset: Double? = null
        var anchor: Double? = null
        var spoken = 0.0
    }

    var onUpdate: ((TranscriptUpdate) -> Unit)? = null

    private var user: Turn? = null
    private var assistant: Turn? = null
    private var userCount = 0
    private var assistantCount = 0
    private var pendingOnset: Double? = null
    private var lastAudible: Double? = null
    private var lastTick: Double? = null
    private var held = false

    /** A user fragment (ASR delta), at [now] ms. */
    fun userDelta(text: String, now: Double, startMs: Double? = null, endMs: Double? = null) {
        if (text.isEmpty()) return
        val u = user ?: open(TranscriptRole.USER, now).also { user = it }
        append(u, text, now, startMs, endMs)
        u.shown = u.length
        emit(TranscriptRole.USER, u, false)
    }

    /** An assistant fragment, at [now] ms. */
    fun assistantDelta(text: String, now: Double, startMs: Double? = null, endMs: Double? = null) {
        if (text.isEmpty()) return
        val a = assistant ?: open(TranscriptRole.ASSISTANT, now).also {
            it.onset = pendingOnset
            assistant = it
        }
        append(a, text, now, startMs, endMs)
        if (a.anchor == null && startMs != null) a.anchor = startMs
        val u = user
        if (u != null && assistantCount > u.assistantAtOpen && a.length >= USER_CLOSE_CHARS) finish(TranscriptRole.USER)
        reveal(a, now)
    }

    /**
     * The user started talking over the assistant's audio: no reveal past the last audible
     * moment until the audio is heard again.
     */
    fun hold() {
        if (assistant != null) held = true
    }

    /** 30 Hz: the assistant audio's level and whether the source is in `speaking`, at [now] ms. */
    fun tick(now: Double, level: Double, speaking: Boolean) {
        val dt = lastTick?.let { min(MAX_TICK_MS, max(0.0, now - it)) } ?: 0.0
        lastTick = now
        val audible = speaking && level > AUDIBLE_LEVEL
        if (audible) {
            lastAudible = now
            held = false
            val a = assistant
            if (a != null) {
                if (a.onset == null) a.onset = now
                a.spoken += dt / 1000
            } else if (pendingOnset == null) {
                pendingOnset = now
            }
        } else if (!speaking) {
            pendingOnset = null
        }
        val u = user
        if (u != null && now - u.lastAt >= USER_SILENCE_MS) finish(TranscriptRole.USER)
        val a = assistant
        if (a != null) {
            if (a.onset == null && !speaking && now - a.lastAt >= USER_SILENCE_MS) {
                finish(TranscriptRole.ASSISTANT)
            } else {
                reveal(a, now)
            }
        }
    }

    /** The source left `speaking` on its own: the assistant turn ends. */
    fun speakingEnded() {
        val a = assistant ?: return
        if (a.onset == null) return
        finish(TranscriptRole.ASSISTANT)
    }

    /** A barge-in cut the assistant: its turn ends with what was audible by the last audible moment. */
    fun cut() {
        val a = assistant ?: return
        val at = lastAudible
        val onset = a.onset
        var n = 0
        if (at != null && onset != null && at >= onset) {
            val anchor = a.anchor
            if (!sync && anchor != null) {
                val vendorAt = anchor + (at - onset - SEGMENT_DELAY_MS)
                for (f in a.fragments) {
                    val end = f.endMs ?: break
                    if (end > vendorAt) break
                    n += f.length
                }
            } else {
                n = min(a.shown, visible(a, at, at))
            }
        }
        a.shown = n
        assistant = null
        pendingOnset = null
        held = false
        emitText(TranscriptRole.ASSISTANT, a, prefix(a.fragments, n).trim(), final = true, truncated = true)
        // What follows the kept text: the unspoken tail (dropped), or, from the first fragment that
        // starts a gap after the one before it, the next utterance.
        var kept = 0
        var i = 0
        while (i < a.fragments.size && kept < n) kept += a.fragments[i++].length
        val fs = a.fragments
        val next = fs.indices.firstOrNull { k ->
            val s = fs[k].startMs
            val e = if (k >= 1) fs[k - 1].endMs else null
            k >= maxOf(i, 1) && s != null && e != null && s - e >= NEW_UTTERANCE_GAP_MS
        } ?: return
        val t = open(TranscriptRole.ASSISTANT, a.lastAt)
        for (f in fs.subList(next, fs.size)) append(t, f.text, a.lastAt, f.startMs, f.endMs)
        t.anchor = t.fragments.first().startMs
        assistant = t
        val u = user
        if (u != null && assistantCount > u.assistantAtOpen && t.length >= USER_CLOSE_CHARS) finish(TranscriptRole.USER)
    }

    /** The session ended: open turns end as they are. */
    fun stop() {
        if (user != null) finish(TranscriptRole.USER)
        val a = assistant
        if (a != null) {
            assistant = null
            emitText(TranscriptRole.ASSISTANT, a, prefix(a.fragments, a.shown).trim(), final = true, truncated = false)
        }
        pendingOnset = null
        lastAudible = null
        lastTick = null
        held = false
    }

    private fun open(role: TranscriptRole, now: Double): Turn {
        val id = if (role == TranscriptRole.USER) "u${++userCount}" else "a${++assistantCount}"
        return Turn(id, now, assistantCount)
    }

    private fun append(t: Turn, text: String, now: Double, startMs: Double?, endMs: Double?) {
        val norm = Normalizer.normalize(text, Normalizer.Form.NFC)
        val length = norm.codePointCount(0, norm.length)
        t.fragments.add(Fragment(norm, length, startMs, endMs))
        t.length += length
        t.lastAt = now
    }

    private fun visible(a: Turn, now: Double, audibleAt: Double?): Int {
        if (!sync || timing == TranscriptTiming.SYNCED || timing == TranscriptTiming.CHARS) return a.length
        val onset = a.onset ?: return 0
        if (timing == TranscriptTiming.NONE) return min(a.length, floor(a.spoken * REVEAL_CHARS_PER_SECOND).toInt())
        val anchor = a.anchor ?: return a.length
        val heldAt = audibleAt?.let { min(now, it + (if (held) 0.0 else CUT_GRACE_MS)) } ?: now
        val head = anchor + (heldAt - onset - SEGMENT_DELAY_MS)
        var n = 0
        for (f in a.fragments) {
            val start = f.startMs
            val end = f.endMs
            if (start == null || end == null) {
                n += f.length
                continue
            }
            if (head >= end) {
                n += f.length
            } else {
                if (head > start) n += floor(f.length * (head - start) / (end - start)).toInt()
                break
            }
        }
        return n
    }

    private fun reveal(a: Turn, now: Double) {
        val n = max(a.shown, min(a.length, visible(a, now, lastAudible)))
        if (n == a.shown) return
        a.shown = n
        emit(TranscriptRole.ASSISTANT, a, false)
    }

    private fun finish(role: TranscriptRole) {
        val t = (if (role == TranscriptRole.USER) user else assistant) ?: return
        if (role == TranscriptRole.USER) {
            user = null
        } else {
            assistant = null
            pendingOnset = null
            held = false
        }
        t.shown = t.length
        emitText(role, t, prefix(t.fragments, t.length).trim(), final = true, truncated = false)
    }

    private fun emit(role: TranscriptRole, t: Turn, final: Boolean) {
        emitText(role, t, prefix(t.fragments, t.shown), final, false)
    }

    private fun emitText(role: TranscriptRole, t: Turn, text: String, final: Boolean, truncated: Boolean) {
        if (!final && text.isBlank()) return
        var seen = 0
        var end: Double? = null
        for (f in t.fragments) {
            if (seen >= t.shown) break
            seen += f.length
            if (f.endMs != null) end = f.endMs
        }
        val start = if (t.shown > 0) t.fragments.firstOrNull()?.startMs else null
        onUpdate?.invoke(TranscriptUpdate(role, text, final, t.id, truncated, start, end))
    }
}
