package dev.sinua.core

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.frame
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.resolvedOpts

/**
 * Smoke-level parity check against `spec/orbs-golden.json`, run through the
 * real UniFFI FFI boundary (Kotlin -> JNA -> C -> Rust -> C -> JNA -> Kotlin)
 * on an actual Android runtime (instrumented, not a JVM unit test -- the
 * native .so is a real Android ELF binary, it can't load on the host JVM).
 * The geometry itself is already exhaustively proven in
 * `crates/core_engine/tests/golden.rs` (all 9 modes, both sizes, 4
 * timestamps each); this only needs to prove the binding round-trips
 * values correctly, not re-derive correctness.
 */
@RunWith(AndroidJUnit4::class)
class GoldenTests {
    @Test
    fun workingFrameMatchesGolden() {
        val f = frame("working", 64u, 0.6)
        assertNotNull(f)
        assertEquals(516, f!!.dots.size)
        assertEquals(32.34438, f.dots[0].x, 1e-4)
        assertEquals(30.683937, f.dots[0].y, 1e-4)
    }

    @Test
    fun globeFrameMatchesGolden() {
        val f = frame("searching", 64u, 0.6)
        assertNotNull(f)
        assertEquals(204, f!!.dots.size)
        assertEquals(33.490622, f.dots[0].x, 1e-4)
        assertEquals(32.435651, f.dots[0].y, 1e-4)
    }

    @Test
    fun webFrameHasLinesAndMatchesGolden() {
        val f = frame("connecting", 64u, 0.6)
        assertNotNull(f)
        assertEquals(48, f!!.dots.size)
        assertEquals(81, f.lines.size)
        assertEquals(23.99695, f.dots[0].x, 1e-4)
        assertEquals(34.67833, f.dots[0].y, 1e-4)
    }

    @Test
    fun morphFrameMatchesGolden() {
        val f = frame("shaping", 64u, 0.6)
        assertNotNull(f)
        assertEquals(24, f!!.dots.size)
        assertEquals(32.0, f.dots[0].x, 1e-4)
        assertEquals(9.301059, f.dots[0].y, 1e-4)
    }

    @Test
    fun allNineStatesResolve() {
        val states = listOf(
            "working", "searching", "solving", "listening", "connecting",
            "weaving", "composing", "breathing", "shaping",
        )
        for (state in states) {
            assertNotNull("$state should resolve", frame(state, 64u, 1.0))
        }
    }

    @Test
    fun unknownStateReturnsNull() {
        assertNull(frame("not-a-real-state", 64u, 0.0))
    }

    /**
     * Sanity check for `frameWithOverrides` (the Studio's live parameter
     * sliders, now reachable from Kotlin too -- mirrors
     * crates/core_engine/tests/golden.rs's `frame_with_overrides_sanity`).
     */
    @Test
    fun frameWithOverridesSanity() {
        val stock = frame("searching", 64u, 0.6)
        assertNotNull(stock)
        val empty = frameWithOverrides("searching", 64u, 0.6, emptyMap())
        assertEquals(stock, empty)

        val overridden = frameWithOverrides("searching", 64u, 0.6, mapOf("scanMul" to 8.0))
        assertNotNull(overridden)
        assertEquals(stock!!.dots.size, overridden!!.dots.size)
        assertNotEquals(stock, overridden)
    }

    /**
     * Sanity check for `resolvedOpts` (the Android Studio's live parameter
     * sliders need this to seed a knob's starting position and to compute
     * `t * speed` -- this was native/UniFFI-only-missing until
     * crates/core_engine/src/lib.rs added it; previously only the
     * wasm/JSON path had it). `composing` resolves to the `ribbon` mode
     * (docs/engine.md's state/mode table) with speed 2.34 at size 64
     * (crates/core_engine/src/orbs/presets.rs's `ribbon` table).
     */
    @Test
    fun resolvedOptsMatchesPreset() {
        val resolved = resolvedOpts("composing", 64u)
        assertNotNull(resolved)
        assertEquals("ribbon", resolved!!.mode)
        assertEquals(2.34, resolved.speed, 1e-9)
    }
}
