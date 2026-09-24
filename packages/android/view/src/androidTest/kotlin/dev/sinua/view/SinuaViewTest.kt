package dev.sinua.view

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.frame
import uniffi.core_engine.frameFromFxSpec
import java.io.File

/**
 * On a device/emulator (the engine is native): FxPaint draws exactly what
 * the Studio's painter drew before the move, the spec path equals
 * frameFromFxSpec, and SinuaView actually renders and animates.
 */
@RunWith(AndroidJUnit4::class)
class SinuaViewTest {
    @get:Rule val compose = createComposeRule()

    /**
     * Every pattern the engine publishes, from `spec/parameters.json` (staged as a
     * test asset by build.gradle.kts) -- the catalog `catalog.rs` pins to the
     * engine's presets. This was a hand-written 34-name string, byte-identical to
     * one in SinuaViewTests.swift, so a 35th pattern went unrendered while the
     * suite still passed.
     */
    private val states: List<String> by lazy {
        val objects = JSONObject(spec("parameters.json")).getJSONArray("objects")
        (0 until objects.length()).flatMap { i ->
            val patterns = objects.getJSONObject(i).getJSONArray("patterns")
            (0 until patterns.length()).map { patterns.getJSONObject(it).getString("id") }
        }
    }

    private fun raster(block: CanvasDrawScope.() -> Unit): IntArray {
        val img = ImageBitmap(256, 256)
        val scope = CanvasDrawScope()
        scope.draw(Density(1f), LayoutDirection.Ltr, Canvas(img), Size(256f, 256f)) { scope.block() }
        val px = img.toPixelMap()
        return IntArray(256 * 256) { px[it % 256, it / 256].hashCode() }
    }

    @Test
    fun fxPaintIsPixelIdenticalToTheStudioPainter() {
        // A floor, not a count: it only guards a loader that read too little.
        assertTrue("only ${states.size} patterns read -- the asset or the loader moved", states.size >= 34)
        var compared = 0
        for (s in states) {
            val f: OrbFrame = frame(s, 64u, 1.7)
                ?: throw AssertionError("$s is in spec/parameters.json but does not render through core_engine")
            for (dark in listOf(false, true)) {
                for (alpha in listOf(1f, 0.37f)) {
                    val old = raster { oldPaintFrame(f, 64, dark, alpha) }
                    val new = raster { paintFxFrame(f, 64, dark, alpha) }
                    assertTrue("$s dark=$dark alpha=$alpha", old.contentEquals(new))
                    compared++
                }
            }
        }
        // The denominator, exactly: two themes x two alphas for every pattern.
        assertEquals("four rasters per pattern", states.size * 4, compared)
        println("FxPaint parity: $compared rasters identical across ${states.size} patterns")
    }

    private fun spec(name: String) = InstrumentationRegistry.getInstrumentation().context.assets.open(name).bufferedReader().use {
        it.readText()
    }

    @Test
    fun specPathEqualsFrameFromFxSpec() {
        val names = InstrumentationRegistry.getInstrumentation().context.assets.list("")!!.filter {
            it.endsWith(".fxspec.json")
        }
        var checked = 0
        for (n in names) {
            val json = spec(n)
            val want = frameFromFxSpec(json, 1.3) ?: continue
            assertEquals(n, want, FxStatePlayer.render(json, null, 1.3, emptyMap(), emptyMap()))
            checked++
        }
        assertTrue("checked $checked", checked > 3)
    }

    @Test
    fun statePlayerCrossFadesThenSettles() {
        val p = FxStatePlayer().apply { crossFade = 0.25 }
        val json = """{"fxSpec":"1.8","object":"orb","pattern":"listening","states":{"speaking":{"pattern":"speaking"}}}"""
        p.setState(null)
        p.frame(json, 1.0, 0.016, emptyMap(), emptyMap())
        p.setState("speaking")
        val mid = p.frame(json, 1.0, 0.1, emptyMap(), emptyMap())!!
        assertTrue(mid.previous != null)
        assertEquals(1 - Math.pow(1 - 0.4, 3.0), mid.blend, 1e-12)
        val end = p.frame(json, 1.0, 0.2, emptyMap(), emptyMap())!!
        assertNull(end.previous)
    }

    /**
     * An app's own Compose UI tests must be able to go idle while a SinuaView animates on
     * screen. The frame loop is an infinite animation, so it has to use
     * `withInfiniteAnimationFrameNanos` (it honours `InfiniteAnimationPolicy`); a plain
     * `withFrameNanos` loop kept the Recomposer busy and `waitForIdle` threw
     * `ComposeNotIdleException` (found by a consuming app's chat-screen tests, 2026-09-24).
     */
    @Test
    fun anAnimatingViewLetsTheHostTestGoIdle() {
        compose.setContent {
            SinuaView(
                pattern = "glowing",
                modifier = Modifier.size(64.dp),
                reducedMotion = FxReducedMotion.NEVER,
            )
        }
        compose.waitForIdle() // autoAdvance on, as in an app's tests
        compose.onRoot().assertExists()
    }

    @Test
    fun fxViewRendersAndAnimates() {
        val json = spec("voice-assistant.fxspec.json")
        // A continuously animating view is never "idle": drive the test clock by hand.
        compose.mainClock.autoAdvance = false
        compose.setContent {
            SinuaView(
                spec = json,
                modifier = Modifier.size(160.dp),
                theme = FxTheme.LIGHT,
                reducedMotion = FxReducedMotion.NEVER,
            )
        }
        compose.mainClock.advanceTimeBy(500)
        val a = compose.onRoot().captureToImage()
        compose.mainClock.advanceTimeBy(700)
        val b = compose.onRoot().captureToImage()
        val pa = a.toPixelMap()
        val pb = b.toPixelMap()
        var inked = 0
        var changed = 0
        for (y in 0 until pa.height) {
            for (x in 0 until pa.width) {
                if (pa[x, y].alpha > 0.05f && pa[x, y].red < 0.9f) inked++
                if (pa[x, y] != pb[x, y]) changed++
            }
        }
        assertTrue("inked $inked", inked > 100)
        assertTrue("changed $changed", changed > 50)
        // Keep the render for the log (pulled with adb).
        val out =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null),
                "SinuaView-spec.png",
            )
        out.outputStream().use {
            android.graphics.Bitmap.createBitmap(
                pa.width,
                pa.height,
                android.graphics.Bitmap.Config.ARGB_8888,
            ).also { bm ->
                for (y in 0 until pa.height) {
                    for (x in 0 until pa.width) {
                        bm.setPixel(
                            x,
                            y,
                            android.graphics.Color.argb(
                                (
                                    pa[x, y].alpha *
                                        255
                                    ).toInt(),
                                (pa[x, y].red * 255).toInt(),
                                (pa[x, y].green * 255).toInt(),
                                (pa[x, y].blue * 255).toInt(),
                            ),
                        )
                    }
                }
            }.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
        }
        println("SinuaView render: inked=$inked changed=$changed -> ${out.absolutePath}")
        assertNotEquals(0, inked)
    }

    @Test
    fun spec12LowPowerCapsAndSheds() {
        val spec = """{"fxSpec":"1.8","object":"orb","pattern":"composing","materials":{"glow":{"strength":1}},"performance":{"maxFps":50,"lowPower":{"maxFps":20,"disable":["glow"]}}}"""
        val m = FxModel(FxInput.Spec(spec), null, null)
        assertEquals(50.0, m.performance(false, null).maxFps!!, 0.0)
        assertEquals(FxPerformance(20.0, emptyMap()), m.performance(true, null))
        val shed = FxStatePlayer.render(spec, null, 1.0, emptyMap(), emptyMap(), lowPower = true)
        val full = FxStatePlayer.render(spec, null, 1.0, emptyMap(), emptyMap(), lowPower = false)
        assertTrue(shed != null && shed != full)
        assertEquals(
            listOf("glow"),
            uniffi.core_engine.resolveFxSpecWith(spec, null, emptyMap(), true).disabledMaterials,
        )
    }

    @Test
    fun maxFpsCapsTheFrameLoop() {
        // Counts the frames the pacer lets into the Canvas (the frame-state writes that
        // invalidate it) on the test's frame clock -- deterministic, unlike on-screen draws.
        var model: FxModel? = null
        compose.mainClock.autoAdvance = false
        compose.setContent {
            val m = remember { FxModel(FxInput.State("listening", 64u, emptyMap(), 1.0), null, null) }
            model = m
            FxCanvasForTest(m, maxFps = 30.0)
        }
        compose.mainClock.advanceTimeBy(200)
        val start = model!!.pacedFrames
        compose.mainClock.advanceTimeBy(2000)
        val capped = model!!.pacedFrames - start
        // The test clock's frames are 16 ms apart: a 30 fps cap passes every other one.
        assertTrue("capped $capped", capped in 58..64)
    }

    /**
     * Renders families' materials golden cases at 256 px, light and dark, and
     * prints each PNG base64 to logcat (tag FxMaterials) -- the test APK's own
     * files are removed with it; the cross-platform comparison runs outside.
     */
    @Test
    fun renderMaterialsGoldenCases() {
        val cases = listOf(
            Triple("generating-64-0.6-highlight-fill", "generating", mapOf("highlightFill" to 1.0)),
            Triple("scanning-64-0.6-trail-fill", "scanning", mapOf("trailFill" to 1.0)),
            Triple("glowing-64-0.6-glow-blur", "glowing", mapOf("glowMode" to 1.0, "glowStrength" to 1.0)),
            Triple(
                "tracking-64-0.6-glow-blur-additive",
                "tracking",
                mapOf(
                    "glowBlend" to 1.0,
                    "glowMode" to 1.0,
                    "glowStrength" to 0.8,
                ),
            ),
            Triple("speaking-64-0.6-liquid-fill", "speaking", mapOf("liquidStrength" to 1.0, "liquidStyle" to 0.0)),
            Triple("metering-64-0.6-liquid-outline", "metering", mapOf("liquidStrength" to 1.0)),
            Triple("glowing-64-0.6-liquid-dots", "glowing", mapOf("liquidStrength" to 1.0, "liquidStyle" to 2.0)),
            Triple(
                "glowing-64-0.6-particles-drift",
                "glowing",
                mapOf("particleStrength" to 1.0, "particleStyle" to 0.0),
            ),
            Triple(
                "speaking-64-0.6-particles-attract",
                "speaking",
                mapOf(
                    "particleStrength" to 1.0,
                    "particleStyle" to 1.0,
                ),
            ),
            Triple(
                "completing-64-0.6-particles-orbit",
                "completing",
                mapOf(
                    "particleStrength" to 1.0,
                    "particleStyle" to 2.0,
                ),
            ),
            Triple(
                "drifting-64-0.6-particles-liquid",
                "drifting",
                mapOf(
                    "liquidStrength" to 1.0,
                    "particleStrength" to 1.0,
                ),
            ),
            Triple("glowing-64-0.6-holo", "glowing", mapOf("holoStrength" to 1.0)),
            Triple(
                "speaking-64-0.6-holo-fill",
                "speaking",
                mapOf(
                    "holoStrength" to 1.0,
                    "liquidStrength" to 1.0,
                    "liquidStyle" to 0.0,
                ),
            ),
            Triple("completing-64-0.6-holo-glow", "completing", mapOf("holoStrength" to 1.0, "glowStrength" to 0.8)),
            Triple(
                "drifting-64-0.6-holo-gradient",
                "drifting",
                mapOf("holoStrength" to 0.5, "gradientStrength" to 1.0),
            ),
            // Per-vertex stroke colour (golden 1.6.0, Polyline.hues): the one-layer paint rule.
            Triple("tracking-64-0.6-holo", "tracking", mapOf("holoStrength" to 1.0)),
            Triple(
                "tracking-64-0.6-gradient3",
                "tracking",
                mapOf(
                    "gradientStrength" to 1.0,
                    "gradientHue" to 200.0,
                    "gradientHue2" to 300.0,
                    "gradientHue3" to 40.0,
                ),
            ),
            Triple("locating-64-0.6-holo", "locating", mapOf("holoStrength" to 1.0)),
            Triple(
                "completing-64-0.6-holo-interrupt",
                "completing",
                mapOf("holoStrength" to 1.0, "interruptAge" to 0.15),
            ),
            // Synthetic, information only (packages/web/scripts/materials/frames.mjs SYNTHETIC).
            Triple(
                "x-completing-64-0.6-holo-glowblur",
                "completing",
                mapOf(
                    "holoStrength" to 1.0,
                    "glowStrength" to 0.8,
                    "glowMode" to 1.0,
                ),
            ),
            Triple(
                "x-completing-64-0.6-holo-glowblur-additive",
                "completing",
                mapOf(
                    "holoStrength" to 1.0,
                    "glowStrength" to 0.8,
                    "glowMode" to 1.0,
                    "glowBlend" to 1.0,
                ),
            ),
        )
        for ((key, state, overrides) in cases) {
            val f = uniffi.core_engine.frameWithOverrides(state, 64u, 0.6, overrides)!!
            if (!key.contains("liquid-outline") && !key.contains("liquid-dots") && !key.contains("particles") &&
                !key.contains("holo") &&
                !key.contains("gradient3")
            ) {
                assertTrue(
                    "$key has materials",
                    f.fills.isNotEmpty() || f.effects.isNotEmpty(),
                )
            }
            if (key.contains("liquid-fill")) {
                assertTrue(
                    "the liquid fill has a hole",
                    f.fills.any {
                        it.holes.isNotEmpty()
                    },
                )
            }
            for (dark in listOf(false, true)) {
                val img = ImageBitmap(256, 256)
                val scope = CanvasDrawScope()
                scope.draw(Density(1f), LayoutDirection.Ltr, Canvas(img), Size(256f, 256f)) {
                    paintFxFrame(f, 64, dark)
                }
                val bos = java.io.ByteArrayOutputStream()
                img.asAndroidBitmap().compress(android.graphics.Bitmap.CompressFormat.PNG, 100, bos)
                val b64 = android.util.Base64.encodeToString(bos.toByteArray(), android.util.Base64.NO_WRAP)
                val name = "android-$key-${if (dark) "dark" else "light"}.png"
                b64.chunked(3000).forEachIndexed { i, chunk -> android.util.Log.i("FxMaterials", "$name|$i|$chunk") }
                android.util.Log.i("FxMaterials", "$name|END|${b64.length}")
            }
        }
    }
}
