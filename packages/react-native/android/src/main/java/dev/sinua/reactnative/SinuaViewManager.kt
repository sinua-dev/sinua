package dev.sinua.reactnative

import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactContext
import com.facebook.react.bridge.WritableMap
import com.facebook.react.module.annotations.ReactModule
import com.facebook.react.uimanager.SimpleViewManager
import com.facebook.react.uimanager.ThemedReactContext
import com.facebook.react.uimanager.UIManagerHelper
import com.facebook.react.uimanager.ViewManagerDelegate
import com.facebook.react.uimanager.events.Event
import com.facebook.react.viewmanagers.SinuaViewManagerDelegate
import com.facebook.react.viewmanagers.SinuaViewManagerInterface
import dev.sinua.view.FxLowPower
import dev.sinua.view.FxReducedMotion
import dev.sinua.view.FxTheme

/** The Fabric view manager behind `<SinuaView>` (src/specs/SinuaViewNativeComponent.ts). */
@ReactModule(name = SinuaViewManager.NAME)
class SinuaViewManager : SimpleViewManager<FxHostView>(), SinuaViewManagerInterface<FxHostView> {
    private val delegate = SinuaViewManagerDelegate(this)

    override fun getDelegate(): ViewManagerDelegate<FxHostView> = delegate
    override fun getName(): String = NAME

    override fun createViewInstance(context: ThemedReactContext): FxHostView =
        FxHostView(context).also { view ->
            view.onFrameStats = { s ->
                val rc = view.context as ReactContext
                val payload = Arguments.createMap().apply {
                    putDouble("dtMs", s.dtMs)
                    putDouble("computeMs", s.computeMs)
                    putDouble("paintMs", s.paintMs)
                }
                UIManagerHelper.getEventDispatcherForReactTag(rc, view.id)
                    ?.dispatchEvent(FrameEvent(UIManagerHelper.getSurfaceId(rc), view.id, payload))
            }
        }

    override fun getExportedCustomDirectEventTypeConstants(): Map<String, Any> =
        mapOf(FrameEvent.NAME to mapOf("registrationName" to "onFrame"))

    override fun setSpec(view: FxHostView, value: String?) { view.spec = value }
    override fun setState(view: FxHostView, value: String?) { view.state = if (value.isNullOrEmpty()) "working" else value }
    override fun setSize(view: FxHostView, value: Int) { view.size = if (value in listOf(20, 32, 64)) value else 64 }
    override fun setOverridesJson(view: FxHostView, value: String?) { view.overrides = FxHostView.map(value) }
    override fun setSpeed(view: FxHostView, value: Double) { view.speed = value }
    override fun setSpecState(view: FxHostView, value: String?) { view.specState = value?.ifEmpty { null } }
    override fun setInputsJson(view: FxHostView, value: String?) { view.inputs = FxHostView.map(value) }
    override fun setVoiceLevelInput(view: FxHostView, value: String?) { view.voiceLevelInput = value?.ifEmpty { null } }
    override fun setCrossFade(view: FxHostView, value: Double) { view.crossFade = value.takeIf { it >= 0 } }
    override fun setAudioStrength(view: FxHostView, value: Double) { view.audioStrength = value.takeIf { it >= 0 } }
    override fun setVoice(view: FxHostView, value: String?) { view.setVoiceMode(value ?: "none") }
    override fun setVoiceSourceId(view: FxHostView, value: String?) { view.bindVoiceSource(value?.ifEmpty { null }) }
    override fun setTheme(view: FxHostView, value: String?) {
        view.theme = when (value) { "light" -> FxTheme.LIGHT; "dark" -> FxTheme.DARK; else -> FxTheme.AUTO }
    }
    override fun setPaused(view: FxHostView, value: Boolean) { view.paused = value }
    override fun setReducedMotion(view: FxHostView, value: String?) {
        view.reducedMotion = when (value) { "always" -> FxReducedMotion.ALWAYS; "never" -> FxReducedMotion.NEVER; else -> FxReducedMotion.AUTO }
    }
    override fun setMaxFps(view: FxHostView, value: Double) { view.maxFps = if (value > 0) value else null }
    override fun setLowPower(view: FxHostView, value: String?) {
        view.lowPower = when (value) { "on" -> FxLowPower.ON; "off" -> FxLowPower.OFF; else -> FxLowPower.AUTO }
    }
    override fun setLabel(view: FxHostView, value: String?) { view.label = value?.ifEmpty { null } }
    override fun setReportFrames(view: FxHostView, value: Boolean) { view.reportFrames = value }
    override fun setLabelsJson(view: FxHostView, value: String?) {
        view.labels = FxHostView.words(value)
    }
    override fun setAnnounce(view: FxHostView, value: String?) {
        view.announce = when (value) { "on" -> true; "off" -> false; else -> null }
    }
    override fun setHaptics(view: FxHostView, value: Boolean) { view.haptics = value }
    override fun setRules(view: FxHostView, value: Boolean) { view.rules = value }
    override fun setTap(view: FxHostView, value: Boolean) { view.tap = value }
    override fun setExpression(view: FxHostView, value: String?) { view.expression = value?.ifEmpty { null } }
    override fun setPaletteJson(view: FxHostView, value: String?) {
        val o = value?.ifEmpty { null }?.let { runCatching { org.json.JSONObject(it) }.getOrNull() }
        view.palette = o?.keys()?.asSequence()?.associateWith { o.optString(it) } ?: emptyMap()
    }
    override fun setLoadoutJson(view: FxHostView, value: String?) {
        val o = value?.ifEmpty { null }?.let { runCatching { org.json.JSONObject(it) }.getOrNull() }
        view.loadout = o?.let {
            val wear = it.optJSONArray("wear")?.let { a -> List(a.length()) { i -> a.optString(i) } }
            dev.sinua.view.SinuaLoadout(
                wear = wear,
                palette = it.optString("palette").ifEmpty { null },
                eyeStyle = it.optString("eyeStyle").ifEmpty { null },
            )
        }
    }
    // The name is stored; a changed key plays it (setters run in the spec's order, name first).
    override fun setEffectName(view: FxHostView, value: String?) { view.setEffectName(value?.ifEmpty { null }) }
    override fun setEffectKey(view: FxHostView, value: Int) { view.setEffect(null, value) }

    private class FrameEvent(surfaceId: Int, viewId: Int, private val payload: WritableMap) : Event<FrameEvent>(surfaceId, viewId) {
        override fun getEventName() = NAME
        override fun getEventData(): WritableMap = payload
        companion object { const val NAME = "topFrame" }
    }

    companion object {
        const val NAME = "SinuaView"
    }
}
