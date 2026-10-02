package dev.sinua.view

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Cross-language parity for the grain tile (design note 22): the same integer hash as the Web's
 * `grainValue` (packages/web/src/paint.ts) and `FxPaint.grainValue` (Swift), checked against the
 * same vectors on every platform.
 */
class GrainVectorsTest {
    @Test
    fun grainHashMatchesTheVectorsEveryPlatformChecks() {
        val want = listOf(
            Triple(0, 0, 0.573750742),
            Triple(1, 0, 0.78714704),
            Triple(0, 1, 0.399833626),
            Triple(63, 63, 0.680704963),
            Triple(17, 42, 0.127631493),
        )
        for ((x, y, v) in want) assertEquals("$x,$y", v, fxGrainValue(x, y), 1e-9)
    }
}
