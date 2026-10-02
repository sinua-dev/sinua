package dev.sinua.view

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import org.json.JSONArray
import org.json.JSONObject
import uniffi.core_engine.FxDiagnostic
import uniffi.core_engine.applyLoadout
import uniffi.core_engine.cosmeticsFor
import uniffi.core_engine.frameStill

/**
 * An end user's choice for a character (FX Spec 1.13, design note 25): small, so the app
 * stores it in its own account and passes it back next launch. [wear]: ids from the spec's
 * `wardrobe` (or its `cosmetics`), one per slot; [palette]: a `wardrobe.palettes` name or a
 * built-in palette (`sunset`, `ocean`, ...); [eyeStyle]: `auto`, `shape`, `glossy`, `pixel`, `dot`.
 */
data class SinuaLoadout(val wear: List<String>? = null, val palette: String? = null, val eyeStyle: String? = null) {
    /** The loadout as JSON (what the engine reads, and what to store). */
    fun toJson(): String {
        val o = JSONObject().put("loadout", 1)
        wear?.let { o.put("wear", JSONArray(it)) }
        palette?.let { o.put("palette", it) }
        eyeStyle?.let { o.put("eyeStyle", it) }
        return o.toString()
    }

    /** [spec] with this loadout applied: what the spec no longer offers is skipped and warns. */
    fun applyTo(spec: String): Pair<String, List<FxDiagnostic>> {
        val r = applyLoadout(spec, toJson())
        return r.spec to r.diagnostics
    }
}

/** One wardrobe item for a picker: whether it fits a character, and why not. */
data class SinuaCosmeticFit(
    val id: String,
    val fits: Boolean,
    /** A key to translate: `fits`, `no-slot` or `not-made-for`. */
    val reason: String,
    /** The reason in English ("" when it fits). */
    val why: String,
) {
    companion object {
        /** What [spec]'s wardrobe offers [character] (a built-in id, or the spec's own recipe's id). */
        fun list(spec: String, character: String): List<SinuaCosmeticFit> =
            // The engine carries each row in the diagnostic record: path = id, severity = reason.
            cosmeticsFor(spec, character).map {
                SinuaCosmeticFit(it.path, it.severity == "fits", it.severity, it.message)
            }
    }
}

/** Thumbnails for a picker screen (design note 25). */
object SinuaThumbnail {
    /**
     * [spec] wearing [loadout], still (no blink, no glance), [px] pixels across, turned
     * [turnYaw] radians (0 = facing). Transparent around the character; null if the spec
     * doesn't resolve.
     */
    fun bitmap(
        spec: String,
        loadout: SinuaLoadout? = null,
        px: Int = 256,
        dark: Boolean = false,
        turnYaw: Double = 0.0,
        engineSize: Int = 128,
    ): android.graphics.Bitmap? {
        val frame = frameStill(spec, loadout?.toJson() ?: "", engineSize.toUInt(), turnYaw) ?: return null
        val img = ImageBitmap(px, px)
        val scope = CanvasDrawScope()
        scope.draw(Density(1f), LayoutDirection.Ltr, Canvas(img), Size(px.toFloat(), px.toFloat())) {
            paintFxFrame(frame, engineSize, dark)
        }
        return img.asAndroidBitmap()
    }
}
