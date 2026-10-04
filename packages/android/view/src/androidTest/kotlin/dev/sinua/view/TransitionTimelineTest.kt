package dev.sinua.view

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.TransitionSide
import kotlin.math.min

/**
 * The transition clock (design note 31) against spec/transition-timeline.json, the steps
 * Web (packages/core/test/transition-timeline.test.mjs) and iOS replay too.
 */
@RunWith(AndroidJUnit4::class)
class TransitionTimelineTest {
    private val assets = InstrumentationRegistry.getInstrumentation().context.assets

    private fun side(o: JSONObject): TransitionSide {
        val ov = o.getJSONObject("overrides")
        return TransitionSide(o.getString("state"), o.getDouble("speed"), ov.keys().asSequence().associateWith { ov.getDouble(it) })
    }

    @Test fun theClockReplaysTheSharedVectors() {
        val cases = JSONObject(assets.open("transition-timeline.json").bufferedReader().use { it.readText() }).getJSONArray("cases")
        for (n in 0 until cases.length()) {
            val c = cases.getJSONObject(n)
            val name = c.getString("name")
            val sidesJ = c.getJSONObject("sides")
            val sides = sidesJ.keys().asSequence().associateWith { side(sidesJ.getJSONObject(it)) }
            val events = c.getJSONArray("events")
            val dts = c.getJSONArray("dts")
            val want = c.getJSONArray("frames")
            val tr = StateTransition()
            var cur = events.getJSONArray(0).getString(1)
            var phase = c.getDouble("t0") * sides.getValue(cur).speed
            var e = 1
            for (i in 0 until dts.length()) {
                while (e < events.length() && events.getJSONArray(e).getInt(0) == i) {
                    val ev = events.getJSONArray(e)
                    tr.start(ev.getDouble(2), ev.getString(3), ev.length() > 4 && ev.getBoolean(4))
                    cur = ev.getString(1)
                    e++
                }
                val dt = dts.getDouble(i)
                tr.advance(dt)
                val to = sides.getValue(cur)
                phase += min(dt, 0.1) * tr.speed(to, 64u)
                val out = tr.frames(to, 64u, phase, emptyMap())
                val w = want.getJSONObject(i)
                val at = "$name frame $i"
                val ws = w.getJSONArray("weights")
                assertEquals(at, ws.length(), tr.weights.size)
                for (k in 0 until ws.length()) assertEquals(at, ws.getDouble(k), tr.weights[k], 1e-9)
                assertEquals(at, w.getDouble("speed"), tr.speed(to, 64u), 1e-9)
                assertEquals(at, w.getDouble("phase"), phase, 1e-9)
                assertEquals(at, w.getBoolean("two"), out?.previous != null)
                assertEquals(at, w.getDouble("blend"), out?.blend ?: 1.0, 1e-9)
                val sums = w.getJSONObject("sums")
                val mine = tr.rateSums(to.state)
                assertEquals(at, sums.keys().asSequence().toSet(), mine.keys)
                for (k in sums.keys()) assertEquals("$at: $k", sums.getDouble(k), mine.getValue(k), 1e-9)
            }
        }
    }
}
