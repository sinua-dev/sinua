package dev.sinua.view

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.TransitionSide
import uniffi.core_engine.applyLoadout
import uniffi.core_engine.characterSlots
import uniffi.core_engine.frameStill
import uniffi.core_engine.resolveFxSpec

/**
 * Loadouts (FX Spec 1.13, design note 25): the shared vectors (spec/loadout-vectors.json, also
 * checked by Rust, wasm and iOS), what fits, thumbnails, and the wear clock.
 */
@RunWith(AndroidJUnit4::class)
class LoadoutTest {
    private val assets = InstrumentationRegistry.getInstrumentation().context.assets

    private fun read(name: String) = assets.open(name).bufferedReader().use { it.readText() }

    private fun ids(a: JSONArray?) = if (a == null) emptyList() else List(a.length()) { a.getJSONObject(it).getString("id") }

    private fun strings(a: JSONArray) = List(a.length()) { a.getString(it) }

    @Test fun theSharedLoadoutVectorsHold() {
        val v = JSONObject(read("loadout-vectors.json"))
        val spec = read(v.getString("spec").removePrefix("examples/"))
        val cases = v.getJSONArray("cases")
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i)
            val name = c.getString("name")
            val r = applyLoadout(spec, c.get("loadout").toString())
            val out = JSONObject(r.spec)
            assertEquals("$name: worn", strings(c.getJSONArray("wear")), ids(out.optJSONArray("cosmetics")))
            assertEquals("$name: warnings", strings(c.getJSONArray("warnings")), r.diagnostics.map { it.path }.sorted())
            assertTrue(name, r.diagnostics.all { it.severity == "warning" })
            val eye = out.optJSONObject("params")?.optString("eyeStyle")?.ifEmpty { null }
            assertEquals("$name: eyeStyle", if (c.isNull("eyeStyle")) null else c.getString("eyeStyle"), eye)
            // An eye colour by name joins the palette (design note 27).
            val iris = { p: JSONObject? -> p?.optString("iris")?.ifEmpty { null } }
            assertEquals("$name: iris", iris(c.optJSONObject("palette")), iris(out.optJSONObject("palette")))
            assertTrue("$name: still draws", resolveFxSpec(r.spec).ok)
        }
        val fits = v.getJSONArray("fits")
        for (i in 0 until fits.length()) {
            val f = fits.getJSONObject(i)
            val ch = f.getString("character")
            val want = f.getJSONObject("expect").let { o -> o.keys().asSequence().associateWith { o.getString(it) } }
            assertEquals(ch, want, SinuaCosmeticFit.list(f.optJSONObject("spec")?.toString() ?: spec, ch).associate { it.id to it.reason })
        }
    }

    /** Design note 29: where a dragged item snaps, from the engine (UniFFI `characterSlots`). */
    @Test fun characterSlotsGiveEachSlotWhereItIsDrawn() {
        val slots = characterSlots("buzzy", 64u, 1.0, mapOf("still" to 1.0))
        assertEquals(listOf("chest", "face", "headTop", "neck"), slots.map { it.name }.sorted())
        val top = slots.first { it.name == "headTop" }
        assertTrue(top.y > 0 && top.y < 32)
        assertTrue(characterSlots("working", 64u, 1.0, emptyMap()).isEmpty())
    }

    @Test fun aThumbnailIsAStillImage() {
        val spec = read("wardrobe-bean.fxspec.json")
        val lo = SinuaLoadout(wear = listOf("round-glasses"))
        assertEquals("catalog:eyes-hazel", JSONObject(SinuaLoadout(iris = "catalog:eyes-hazel").toJson()).getString("iris"))
        val bmp = SinuaThumbnail.bitmap(spec, lo, px = 128)
        assertNotNull(bmp)
        assertEquals(128, bmp!!.width)
        assertEquals(frameStill(spec, lo.toJson(), 64u, 0.0), frameStill(spec, lo.toJson(), 64u, 0.0))
        assertNull(SinuaThumbnail.bitmap("{"))
    }

    @Test fun theWearClockRunsBesideAStateChange() {
        val tr = StateTransition()
        tr.frames(TransitionSide("bean", 1.0, emptyMap()), 64u, 1.0, emptyMap())
        tr.start(0.6, "easeInOut")
        tr.advance(0.1)
        tr.wear()
        assertTrue(tr.wearing && tr.active)
        tr.advance(WEAR_SECONDS)
        assertTrue("the loadout change ends first, the state change runs on", !tr.wearing && tr.active)
        tr.wear()
        tr.cancel()
        assertFalse(tr.wearing || tr.active)
    }

    @Test fun aLoadoutChangeOnTheModelEasesAndEnds() {
        val m = FxModel(FxInput.Spec(read("wardrobe-bean.fxspec.json")), null, null)
        m.setLoadout(SinuaLoadout(wear = emptyList()))
        val bare = m.frame(1_000_000L, running = true, reduced = false)!!.frame.fills.size
        m.setLoadout(SinuaLoadout(wear = listOf("party-hat")))
        var t = 1_000_000L
        repeat(30) {
            t += 20_000_000L
            m.frame(t, running = true, reduced = false)
        }
        assertTrue("the hat is worn", m.frame(t + 20_000_000L, running = true, reduced = false)!!.frame.fills.size > bare)
    }

    /** Sinua's catalog pack (design note 26): the asset is spec/catalog/catalog-1.json, it loads, and a spec wears from it. */
    @Test fun theBundledCatalogLoadsAndIsTheSpecFile() {
        val ctx = InstrumentationRegistry.getInstrumentation().context
        val bundled = assertNotNullAnd(SinuaCatalog.json(ctx))
        assertEquals(JSONObject(read("catalog/catalog-1.json")).toString(), JSONObject(bundled).toString())
        assertTrue(SinuaCatalog.load(ctx).isEmpty())
        val r = resolveFxSpec(
            """{"fxSpec":"1.13","object":"character","pattern":"bean","cosmetics":["catalog:crown"],"palette":"catalog:berry"}""",
        )
        assertTrue(r.diagnostics.toString(), r.ok && r.diagnostics.isEmpty())
        assertTrue(r.state.startsWith("recipe:bean:"))
        assertTrue(uniffi.core_engine.unloadCatalog("catalog"))
    }

    private fun assertNotNullAnd(s: String?): String {
        assertNotNull("the catalog asset is bundled", s)
        return s!!
    }
}
