package dev.sinua.view

import androidx.activity.ComponentActivity
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.atomic.AtomicInteger
import dev.sinua.view.test.R as TestR

/**
 * SinuaViewLayout inflated from XML in a ComponentActivity (the lifecycle owner a
 * Compose-in-View needs): attributes parsed, frames render while running and stop
 * when paused, and the spec/state paths switch from Kotlin. Real-time polling, not
 * the compose test rule: its idle sync never settles on a continuously animating
 * view (ComposeNotIdleException). Render-only; no audio.
 */
@RunWith(AndroidJUnit4::class)
class SinuaViewLayoutTest {
    private lateinit var scenario: ActivityScenario<ComponentActivity>
    private lateinit var stateView: SinuaViewLayout
    private lateinit var specView: SinuaViewLayout

    @Before
    fun setUp() {
        scenario = ActivityScenario.launch(ComponentActivity::class.java)
        scenario.onActivity {
            it.setContentView(TestR.layout.fx_view_layout_test)
            stateView = it.findViewById(TestR.id.fx_state)
            specView = it.findViewById(TestR.id.fx_spec)
        }
    }

    @After
    fun tearDown() = scenario.close()

    private fun until(timeoutMs: Long = 5_000, cond: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (!cond()) {
            check(System.currentTimeMillis() < end) { "timed out" }
            Thread.sleep(20)
        }
    }

    @Test
    fun xmlAttributesAreParsed() {
        scenario.onActivity {
            assertNull(stateView.spec)
            assertEquals("speaking", stateView.pattern)
            assertEquals(20u, stateView.size)
            assertEquals(1.5, stateView.speed, 1e-6)
            assertEquals(FxTheme.DARK, stateView.theme)
            assertEquals(FxLowPower.ON, stateView.lowPower)
            assertEquals(30.0, stateView.maxFps!!, 1e-6)
            assertEquals("Assistant", stateView.contentDescription)

            assertNotNull("fxSpecAsset loaded from assets", specView.spec)
            assertTrue(specView.spec!!.trimStart().startsWith("{"))
            assertTrue(specView.paused)
            assertEquals("listening", specView.state)
            assertEquals(FxReducedMotion.NEVER, specView.reducedMotion)
            assertNull(specView.maxFps)
        }
    }

    @Test
    fun rendersWhileRunningAndStopsWhenPaused() {
        val frames = AtomicInteger()
        scenario.onActivity {
            stateView.reducedMotion = FxReducedMotion.NEVER
            stateView.onFrame = { frames.incrementAndGet() }
        }
        until { frames.get() > 10 } // ~30 fps (fxLowPower="on")
        scenario.onActivity { stateView.paused = true }
        Thread.sleep(200) // let the pause land
        val atPause = frames.get()
        Thread.sleep(600)
        assertTrue("paused stops the loop (${frames.get()} vs $atPause)", frames.get() - atPause <= 1)
        scenario.onActivity { stateView.paused = false }
        until { frames.get() > atPause + 5 }
    }

    @Test
    fun switchesBetweenStateAndSpecFromKotlin() {
        val frames = AtomicInteger()
        scenario.onActivity {
            stateView.reducedMotion = FxReducedMotion.NEVER
            stateView.onFrame = { frames.incrementAndGet() }
            stateView.spec = specView.spec // state path -> spec path
        }
        until { frames.get() > 10 }
        scenario.onActivity {
            stateView.spec = null
            stateView.pattern = "tracking" // back to the pattern path
        }
        val before = frames.get()
        until { frames.get() > before + 10 }
    }
}
