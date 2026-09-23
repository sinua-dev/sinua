package dev.sinua.reactnative

import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.ReadableMap
import com.facebook.react.bridge.WritableArray
import com.facebook.react.bridge.WritableMap
import uniffi.core_engine.ColorMode
import uniffi.core_engine.Dot
import uniffi.core_engine.Line
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.Polyline
import uniffi.core_engine.frame as coreFrame
import uniffi.core_engine.frameFromFxSpecWith as coreFrameFromFxSpecWith
import uniffi.core_engine.frameWithOverrides as coreFrameWithOverrides
import uniffi.core_engine.fxColorToHsl as coreFxColorToHsl
import uniffi.core_engine.resolveFxSpecWith as coreResolveFxSpecWith

/**
 * React Native bridge module. `coreFrame` (aliased above) is the
 * UniFFI-generated function from the vendored `uniffi/core_engine/core_engine.kt`
 * (same file `packages/android` uses) -- this class only converts its
 * `OrbFrame` data class into the `WritableMap` shape the RN bridge sends to JS.
 */
class SinuaCoreModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "SinuaCore"

    @ReactMethod
    fun resolveFrame(state: String, size: Double, t: Double, promise: Promise) {
        val result: OrbFrame? = coreFrame(state, size.toUInt(), t)
        promise.resolve(result?.let { toWritableMap(it) })
    }

    @ReactMethod
    fun resolveFrameWithOverrides(state: String, size: Double, t: Double, overrides: ReadableMap, promise: Promise) {
        val overridesMap = mutableMapOf<String, Double>()
        val iterator = overrides.keySetIterator()
        while (iterator.hasNextKey()) {
            val key = iterator.nextKey()
            overridesMap[key] = overrides.getDouble(key)
        }
        val result: OrbFrame? = coreFrameWithOverrides(state, size.toUInt(), t, overridesMap)
        promise.resolve(result?.let { toWritableMap(it) })
    }

    // FX Spec (docs/fx-spec.md): parse/validate/resolve all happen in Rust;
    // these only carry the JSON text in and the records out.
    @ReactMethod
    fun resolveFxSpec(json: String, state: String?, inputs: ReadableMap?, lowPower: Boolean, promise: Promise) {
        val r = coreResolveFxSpecWith(json, state, numbers(inputs), lowPower)
        val map = Arguments.createMap()
        map.putBoolean("ok", r.ok)
        map.putString("state", r.state)
        map.putInt("size", r.size.toInt())
        map.putDouble("speed", r.speed)
        val overrides = Arguments.createMap()
        r.overrides.forEach { (k, v) -> overrides.putDouble(k, v) }
        map.putMap("overrides", overrides)
        val diagnostics = Arguments.createArray()
        r.diagnostics.forEach { d ->
            val m = Arguments.createMap()
            m.putString("severity", d.severity)
            m.putString("path", d.path)
            m.putString("message", d.message)
            diagnostics.pushMap(m)
        }
        map.putArray("diagnostics", diagnostics)
        map.putString("stateKey", r.stateKey)
        map.putArray("stateKeys", Arguments.fromList(r.stateKeys))
        map.putArray("inactiveBindings", Arguments.fromList(r.inactiveBindings))
        val fps = r.maxFps
        if (fps == null) map.putNull("maxFps") else map.putDouble("maxFps", fps)
        map.putArray("disabledMaterials", Arguments.fromList(r.disabledMaterials))
        promise.resolve(map)
    }

    @ReactMethod
    fun frameFromFxSpec(json: String, elapsed: Double, state: String?, inputs: ReadableMap?, lowPower: Boolean, promise: Promise) {
        promise.resolve(coreFrameFromFxSpecWith(json, elapsed, state, numbers(inputs), lowPower)?.let { toWritableMap(it) })
    }

    /** JS `{ name: number }` -> `Map<String, Double>` (non-numbers skipped). */
    private fun numbers(map: ReadableMap?): Map<String, Double> {
        val out = mutableMapOf<String, Double>()
        val it = map?.keySetIterator() ?: return out
        while (it.hasNextKey()) {
            val key = it.nextKey()
            if (map.getType(key) == com.facebook.react.bridge.ReadableType.Number) out[key] = map.getDouble(key)
        }
        return out
    }

    @ReactMethod
    fun fxColorToHsl(color: String, promise: Promise) {
        val c = coreFxColorToHsl(color)
        if (c == null) {
            promise.resolve(null)
            return
        }
        val map = Arguments.createMap()
        map.putDouble("h", c.h)
        map.putDouble("s", c.s)
        map.putDouble("l", c.l)
        map.putBoolean("achromatic", c.achromatic)
        map.putString("hex", c.hex)
        promise.resolve(map)
    }

    /**
     * Field-for-field mirror of `packages/core`'s TS `OrbFrame` (the wasm
     * JSON shape), so a consumer's paint code is identical on Web and RN.
     */
    private fun toWritableMap(frame: OrbFrame): WritableMap {
        val map = Arguments.createMap()
        map.putArray("dots", dotsArray(frame.dots))
        map.putArray("lines", linesArray(frame.lines))
        map.putArray("polylines", polylinesArray(frame.polylines))
        map.putString("colorMode", if (frame.colorMode == ColorMode.FIXED) "fixed" else "ink")
        // Materials phase 1: present only when non-empty, like the wasm JSON.
        if (frame.fills.isNotEmpty()) {
            val fills = Arguments.createArray()
            frame.fills.forEach { f ->
                val m = Arguments.createMap()
                val pts = Arguments.createArray()
                f.points.forEach { p ->
                    val q = Arguments.createMap()
                    q.putDouble("x", p.x)
                    q.putDouble("y", p.y)
                    pts.pushMap(q)
                }
                m.putArray("points", pts)
                if (f.holes.isNotEmpty()) {
                    val holes = Arguments.createArray()
                    f.holes.forEach { ring ->
                        val r = Arguments.createArray()
                        ring.forEach { p ->
                            val q = Arguments.createMap()
                            q.putDouble("x", p.x)
                            q.putDouble("y", p.y)
                            r.pushMap(q)
                        }
                        holes.pushArray(r)
                    }
                    m.putArray("holes", holes)
                }
                m.putDouble("white", f.white)
                m.putDouble("a", f.a)
                m.putDouble("saturation", f.saturation)
                m.putDouble("hue", f.hue)
                m.putDouble("blur", f.blur)
                m.putInt("blend", f.blend.toInt())
                val g = f.gradient
                if (g == null) {
                    m.putNull("gradient")
                } else {
                    val gm = Arguments.createMap()
                    gm.putInt("kind", g.kind.toInt())
                    gm.putDouble("x0", g.x0)
                    gm.putDouble("y0", g.y0)
                    gm.putDouble("x1", g.x1)
                    gm.putDouble("y1", g.y1)
                    gm.putDouble("r", g.r)
                    val stops = Arguments.createArray()
                    g.stops.forEach { st ->
                        val sm = Arguments.createMap()
                        sm.putDouble("offset", st.offset)
                        sm.putDouble("white", st.white)
                        sm.putDouble("a", st.a)
                        sm.putDouble("saturation", st.saturation)
                        sm.putDouble("hue", st.hue)
                        stops.pushMap(sm)
                    }
                    gm.putArray("stops", stops)
                    m.putMap("gradient", gm)
                }
                fills.pushMap(m)
            }
            map.putArray("fills", fills)
        }
        if (frame.effects.isNotEmpty()) {
            val effects = Arguments.createArray()
            frame.effects.forEach { e ->
                val m = Arguments.createMap()
                m.putInt("target", e.target.toInt())
                m.putInt("start", e.start.toInt())
                m.putInt("count", e.count.toInt())
                m.putDouble("blur", e.blur)
                m.putInt("blend", e.blend.toInt())
                effects.pushMap(m)
            }
            map.putArray("effects", effects)
        }
        return map
    }

    private fun dotsArray(dots: List<Dot>): WritableArray {
        val array = Arguments.createArray()
        for (dot in dots) {
            val m = Arguments.createMap()
            m.putDouble("x", dot.x)
            m.putDouble("y", dot.y)
            m.putDouble("z", dot.z)
            m.putDouble("r", dot.r)
            m.putDouble("white", dot.white)
            m.putDouble("a", dot.a)
            m.putDouble("saturation", dot.saturation)
            m.putDouble("hue", dot.hue)
            array.pushMap(m)
        }
        return array
    }

    private fun polylinesArray(polylines: List<Polyline>): WritableArray {
        val array = Arguments.createArray()
        for (polyline in polylines) {
            val points = Arguments.createArray()
            for (point in polyline.points) {
                val p = Arguments.createMap()
                p.putDouble("x", point.x)
                p.putDouble("y", point.y)
                points.pushMap(p)
            }
            val m = Arguments.createMap()
            m.putArray("points", points)
            m.putDouble("white", polyline.white)
            m.putDouble("a", polyline.a)
            m.putDouble("w", polyline.w)
            m.putDouble("saturation", polyline.saturation)
            m.putDouble("hue", polyline.hue)
            array.pushMap(m)
        }
        return array
    }

    private fun linesArray(lines: List<Line>): WritableArray {
        val array = Arguments.createArray()
        for (line in lines) {
            val m = Arguments.createMap()
            m.putDouble("x1", line.x1)
            m.putDouble("y1", line.y1)
            m.putDouble("x2", line.x2)
            m.putDouble("y2", line.y2)
            m.putDouble("white", line.white)
            m.putDouble("a", line.a)
            m.putDouble("w", line.w)
            m.putDouble("saturation", line.saturation)
            m.putDouble("hue", line.hue)
            array.pushMap(m)
        }
        return array
    }
}
