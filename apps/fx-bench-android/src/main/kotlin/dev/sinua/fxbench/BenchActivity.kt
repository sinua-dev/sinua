// sinua bench, Android (docs/bench.md): every case in spec/bench/cases.json
// through :sinua-view's SinuaView twice (lowPower OFF, then ON) for warmup +
// measure seconds; onFrame stats + JankStats -> one result per
// spec/bench/result.schema.json, written to the app's external files dir
// (/sdcard/Android/data/dev.sinua.fxbench/files/bench-<ts>.json) and
// logged as `SINUA_BENCH_DONE <path>` (tag FxBench). Intent extras:
// `benchAuto` (true = run on launch), `benchSeconds`, `benchCases` (a,b).
package dev.sinua.fxbench

import android.os.Build
import android.os.Bundle
import android.os.Process
import android.os.SystemClock
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.metrics.performance.JankStats
import dev.sinua.view.FX_DEFAULT_LOW_POWER
import dev.sinua.view.FxFrameStats
import dev.sinua.view.FxLowPower
import dev.sinua.view.FxReducedMotion
import dev.sinua.view.SinuaView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

data class BenchCase(val id: String, val label: String, val state: String, val overrides: Map<String, Double>)

/** Frame stats land here, off Compose state (no recomposition per frame). */
class Collector {
    @Volatile var measuring = false
    val dts = ArrayList<Double>()
    val computes = ArrayList<Double>()
    val paints = ArrayList<Double>()
    @Volatile var jank = 0
    fun reset() { dts.clear(); computes.clear(); paints.clear(); jank = 0 }
    fun add(s: FxFrameStats) {
        if (!measuring) return
        dts += s.dtMs; computes += s.computeMs; paints += s.paintMs
    }
}

class BenchActivity : ComponentActivity() {
    private val collector = Collector()
    private var jankStats: JankStats? = null

    private var current by mutableStateOf<Pair<BenchCase, Boolean>?>(null)
    private var status by mutableStateOf("idle")
    private val rows = mutableStateListOf<String>()
    private var running = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val file = JSONObject(assets.open("cases.json").bufferedReader().readText())
        val auto = intent.getBooleanExtra("benchAuto", false)
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                Surface(Modifier.fillMaxSize()) {
                    val scope = rememberCoroutineScope()
                    Screen(onRun = { scope.launch { run(file) } })
                    if (auto) LaunchedEffect(Unit) { run(file) }
                }
            }
        }
        jankStats = JankStats.createAndTrack(window) { f -> if (collector.measuring && f.isJank) collector.jank++ }
    }

    override fun onResume() { super.onResume(); jankStats?.isTrackingEnabled = true }
    override fun onPause() { super.onPause(); jankStats?.isTrackingEnabled = false }

    @Composable
    private fun Screen(onRun: () -> Unit) {
        Column(Modifier.padding(16.dp)) {
            Text("sinua bench", style = MaterialTheme.typography.titleMedium)
            Text(
                "Runs every case twice (normal, low power). Keep the app in the foreground and don't touch the screen while it runs. Emulator numbers are not device numbers.",
                fontSize = 12.sp,
            )
            Row { Button(onClick = onRun) { Text("Run") } }
            Text(status, fontSize = 12.sp, fontFamily = FontFamily.Monospace)
            val cur = current
            if (cur != null) {
                key(cur.first.id, cur.second) {
                    SinuaView(
                        state = cur.first.state,
                        modifier = Modifier.size(256.dp),
                        overrides = cur.first.overrides,
                        reducedMotion = FxReducedMotion.NEVER,
                        lowPower = if (cur.second) FxLowPower.ON else FxLowPower.OFF,
                        onFrame = collector::add,
                    )
                }
            } else {
                Spacer(Modifier.size(256.dp))
            }
            LazyColumn { items(rows) { Text(it, fontSize = 11.sp, fontFamily = FontFamily.Monospace) } }
        }
    }

    private fun refreshHz(): Double {
        @Suppress("DEPRECATION")
        val d = if (Build.VERSION.SDK_INT >= 30) display else windowManager.defaultDisplay
        return (d?.refreshRate ?: 60f).toDouble().let { Math.round(it).toDouble() }
    }

    private suspend fun run(file: JSONObject) {
        if (running) return
        running = true
        val size = file.getInt("size").toUInt()
        val warmup = file.getDouble("warmupSeconds")
        val measure = intent.getDoubleExtra("benchSeconds", 0.0).takeIf { it > 0 } ?: file.getDouble("measureSeconds")
        val only = intent.getStringExtra("benchCases")?.split(",")
        val arr = file.getJSONArray("cases")
        val cases = (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            val ov = o.getJSONObject("overrides")
            BenchCase(o.getString("id"), o.getString("label"), o.getString("state"), ov.keys().asSequence().associateWith { ov.getDouble(it) })
        }.filter { only == null || it.id in only }
        val hz = refreshHz()
        val iso = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }
        val started = iso.format(Date())
        val out = JSONArray()
        rows.clear()
        var k = 0
        for (c in cases) for (low in listOf(false, true)) {
            k++
            status = "$k/${cases.size * 2} · ${c.label} · ${if (low) "low" else "normal"}"
            collector.measuring = false
            collector.reset()
            current = c to low
            delay((warmup * 1000).toLong())
            val cpu0 = Process.getElapsedCpuTime()
            val t0 = SystemClock.elapsedRealtimeNanos()
            collector.measuring = true
            delay((measure * 1000).toLong())
            collector.measuring = false
            val wall = (SystemClock.elapsedRealtimeNanos() - t0) / 1e9
            val cpu = (Process.getElapsedCpuTime() - cpu0) / 1000.0 / wall * 100
            val s = summarize(collector.dts.toList(), collector.computes.toList(), collector.paints.toList(), wall, hz, if (low) FX_DEFAULT_LOW_POWER.maxFps else null)
            val eff = if (low) c.overrides + FX_DEFAULT_LOW_POWER.overrides else c.overrides
            val cost = uniffi.core_engine.estimateCost(c.state, size, eff)
            fun p(x: Pct) = JSONObject().put("p50", x.p50).put("p95", x.p95).put("p99", x.p99).put("max", x.max)
            out.put(
                JSONObject()
                    .put("id", c.id).put("power", if (low) "low" else "normal")
                    .put("targetFps", s.targetFps).put("frames", s.frames).put("durationS", s.durationS).put("fps", s.fps)
                    .put("frameMs", p(s.frameMs)).put("computeMs", p(s.computeMs)).put("paintMs", p(s.paintMs))
                    .put("droppedFrames", s.droppedFrames).put("hitchRatioMsPerS", s.hitchRatioMsPerS)
                    .put("cpuPct", r(cpu)).put("platformJankFrames", collector.jank).put("longAnimationFrames", JSONObject.NULL)
                    .put(
                        "cost",
                        if (cost == null) JSONObject.NULL else JSONObject()
                            .put("class", cost.`class`).put("elements", cost.elements.toDouble())
                            .put("coverage", r(cost.coverage)).put("blurLoad", r(cost.blurLoad)),
                    ),
            )
            rows += "%-16s %-6s %4.0f fps p95 %5.1f ms drop %d jank %d cpu %.0f%%".format(
                c.id, if (low) "low" else "normal", s.fps, s.frameMs.p95, s.droppedFrames, collector.jank, cpu,
            )
        }
        current = null
        val emulator = Build.FINGERPRINT.contains("generic") || Build.HARDWARE.contains("ranchu") || Build.PRODUCT.contains("sdk")
        val result = JSONObject()
            .put("schemaVersion", 1).put("platform", "android")
            .put(
                "device",
                JSONObject().put("model", "${Build.MANUFACTURER} ${Build.MODEL}" + if (emulator) " (emulator)" else "")
                    .put("os", "Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})")
                    .put("isSimulator", emulator).put("refreshHz", hz),
            )
            .put("startedAt", started).put("measureSeconds", measure).put("casesVersion", file.getInt("version"))
            .put("cases", out)
        // App-specific external storage: `adb pull` reads it on a release build
        // too (run-as needs a debuggable app). Internal storage as a fallback.
        val dir = getExternalFilesDir(null) ?: filesDir
        val f = File(dir, "bench-${started.replace(":", "-")}.json")
        f.writeText(result.toString(2))
        status = "done · ${hz.toInt()} Hz · ${if (emulator) "emulator: not device numbers" else "device"} · ${f.name}"
        Log.i("FxBench", "SINUA_BENCH_DONE ${f.absolutePath}")
        running = false
    }
}
