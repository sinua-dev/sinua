package dev.sinua.core

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.checkOverrides
import uniffi.core_engine.estimateCost
import uniffi.core_engine.frameFromFxSpec
import uniffi.core_engine.frameFromFxSpecWith
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.fxColorToHsl
import uniffi.core_engine.fxSpecCost
import uniffi.core_engine.parameterCatalogJson
import uniffi.core_engine.particleDefaults
import uniffi.core_engine.resolveFxSpec
import uniffi.core_engine.resolveFxSpecWith
import uniffi.core_engine.resolvedOpts

/**
 * FX Spec through the UniFFI boundary on an Android runtime: every
 * `spec/examples/` spec resolves with no errors, and `frameFromFxSpec`
 * renders exactly what `frameWithOverrides(resolved)` at
 * `elapsed * presetSpeed * spec.speed` does -- the property
 * `crates/core_engine/src/fx_spec.rs` checks natively. The examples ride in
 * the test APK via the same `androidTest` assets dir as the golden file
 * (`spec/`, so they sit under `examples/`).
 */
@RunWith(AndroidJUnit4::class)
class FxSpecTests {
    @Test
    fun examplesRenderLikeOverrides() {
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val files = assets.list("examples")!!.filter { it.endsWith(".fxspec.json") }.sorted()
        assertTrue("found ${files.size} examples", files.size >= 8)
        for (file in files) {
            val json = assets.open("examples/$file").bufferedReader().use { it.readText() }
            // v1.1: every `states` key too, with the fixed inputs every
            // platform's parity test passes (fx-spec.test.mjs's TEST_INPUTS).
            val keys: List<String?> = listOf(null) + resolveFxSpec(json).stateKeys
            for (key in keys) {
                for (low in listOf(false, true)) {
                    val label = "$file ${key ?: "<base>"} lowPower=$low"
                    val r = resolveFxSpecWith(json, key, INPUTS, low)
                    assertTrue("$label: ${r.diagnostics}", r.ok)
                    assertEquals(label, emptyList<String>(), r.inactiveBindings)
                    val preset = resolvedOpts(r.state, r.size)!!.speed
                    val got = frameFromFxSpecWith(json, 1.3, key, INPUTS, low)
                    assertNotNull(label, got)
                    assertEquals(label, frameWithOverrides(r.state, r.size, 1.3 * preset * r.speed, r.overrides), got)
                    assertNotNull(label, fxSpecCost(json, key, INPUTS, low))
                }
            }
        }
        // FX Spec 1.2: low power caps fps and sheds glow + noise.
        val power = assets.open("examples/status-beacon-power.fxspec.json").bufferedReader().use { it.readText() }
        assertEquals(30.0, resolveFxSpecWith(power, null, emptyMap()).maxFps)
        val low = resolveFxSpecWith(power, null, emptyMap(), true)
        assertEquals(15.0, low.maxFps)
        assertEquals(listOf("glow", "noise"), low.disabledMaterials)
        assertEquals("heavy", estimateCost("working", 64u, mapOf("glowStrength" to 1.0))?.`class`)
        // Per-state particle defaults (the Studio's knob defaults).
        assertEquals(3.0, particleDefaults("tracking")?.get("particleStyle"))
        assertEquals(1.0, particleDefaults("notifying")?.get("particleSync"))
        assertEquals(4.5, particleDefaults("working")?.get("particleLife"))
        assertNull(particleDefaults("nope"))
    }

    private companion object {
        val INPUTS = mapOf(
            "micMuted" to 0.0,
            "micLevel" to 0.6,
            "agentVolume" to 0.5,
            "steps" to 6200.0,
            "waterMl" to 1800.0,
            "activeMinutes" to 12.0,
            "heartRate" to 128.0,
        )
    }

    @Test
    fun diagnosticsAndColor() {
        val r = resolveFxSpec("""{"fxSpec":"1.8","object":"orb","pattern":"working","params":{"orbitn":3}}""")
        assertFalse(r.ok)
        assertTrue(r.diagnostics.any { it.path == "/params/orbitn" && it.message.contains("`orbitN`") })
        assertNull(frameFromFxSpec("""{"fxSpec":"2.0","object":"orb","pattern":"working"}""", 0.0))
        val c = fxColorToHsl("#ff00ff")!!
        assertEquals("#ff00ff", c.hex)
        assertEquals(300.0, c.h, 1e-9)
    }

    /** The parameter catalog (docs/parameters.md) crosses UniFFI as JSON text; checkOverrides warns only. */
    @Test
    fun parameterCatalogAndCheckOverrides() {
        val cat = org.json.JSONObject(parameterCatalogJson())
        val objects = cat.getJSONArray("objects")
        assertEquals(
            listOf("SinuaOrb", "SinuaSignal", "SinuaRing", "SinuaCore", "SinuaBeacon"),
            (0 until objects.length()).map { objects.getJSONObject(it).getString("component") },
        )
        assertEquals(
            "glow.strength",
            cat.getJSONObject("definitions").getJSONObject("glowStrength@shared").getString("path"),
        )
        val w = checkOverrides("breathing", 64u, mapOf("lanse" to 6.0))
        assertEquals(1, w.size)
        assertEquals("warning", w[0].severity)
        assertTrue(w[0].message.contains("did you mean `lanes`"))
        assertTrue(checkOverrides("tracking", 64u, mapOf("progress1" to 2.0)).isEmpty())
    }
}
