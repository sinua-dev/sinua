package dev.sinua.core

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.AnnouncerState
import uniffi.core_engine.a11yAccessibleName
import uniffi.core_engine.a11yAnnounceStep

/**
 * State-aware accessibility through the Android bindings: spec/a11y-announce-vectors.json
 * (the file the Web and iOS tests read too), driven with the host loop every platform runs.
 */
@RunWith(AndroidJUnit4::class)
class A11yVectorsTests {
    private val vectors: JSONObject by lazy {
        val ctx = InstrumentationRegistry.getInstrumentation().context
        JSONObject(ctx.assets.open("a11y-announce-vectors.json").bufferedReader().readText())
    }

    private fun run(steps: List<Pair<Double, String?>>, end: Double): List<String> {
        var s = AnnouncerState(false, null, 0.0, null, 0.0)
        val said = mutableListOf<String>()
        var pending: Double? = null
        var i = 0
        var words: String? = null
        while (true) {
            val next = steps.getOrNull(i)?.first
            val t = listOfNotNull(next, pending).minOrNull() ?: break
            if (t > end) break
            if (next == t) {
                words = steps[i].second
                i++
            }
            val out = a11yAnnounceStep(s, words, t)
            s = out.state
            pending = out.recheckAt
            out.announce?.let { said += "$t $it" }
        }
        return said
    }

    @Test
    fun theAnnouncerAndTheNamesMatchTheSharedVectors() {
        val cases = vectors.getJSONArray("cases")
        assertTrue(cases.length() >= 6)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val stepsJson = case.getJSONArray("steps")
            val steps = (0 until stepsJson.length()).map {
                val a = stepsJson.getJSONArray(it)
                a.getDouble(0) to (if (a.isNull(1)) null else a.getString(1))
            }
            val saidJson = case.getJSONArray("said")
            val want = (0 until saidJson.length()).map {
                val a = saidJson.getJSONArray(it)
                "${a.getDouble(0)} ${a.getString(1)}"
            }
            assertEquals(case.getString("name"), want, run(steps, case.getDouble("end")))
        }
        val names = vectors.getJSONArray("names")
        for (n in 0 until names.length()) {
            val o = names.getJSONObject(n)
            fun map(k: String): Map<String, String> = o.getJSONObject(k).let { j -> j.keys().asSequence().associateWith { j.getString(it) } }
            val state = if (o.isNull("state")) null else o.getString("state")
            assertEquals(o.getString("expect"), a11yAccessibleName(o.getString("name"), state, map("spec"), map("app")))
        }
    }
}
