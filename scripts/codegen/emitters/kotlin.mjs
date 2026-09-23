// Compose components (:sinua-view, package dev.sinua.view.generated): per object a props
// class with a pure toOverrides() and a @Composable over SinuaView, plus shared types.
import { HEADER_LINES, sharedGroups, flatProps, docLine, upperSnake } from "./common.mjs";
import { pascal } from "../model.mjs";

const KEYWORDS = new Set(["as", "break", "class", "continue", "do", "else", "false", "for", "fun", "if", "in", "interface", "is", "null", "object", "package", "return", "super", "this", "throw", "true", "try", "typealias", "typeof", "val", "var", "when", "while"]);
const id = (n) => (KEYWORDS.has(n) ? `\`${n}\`` : n);
const PKG = "dev.sinua.view.generated";
const header = () => HEADER_LINES.map((l) => `// ${l}`).join("\n");
const str = (s) => JSON.stringify(s);
const kdoc = (text, indent) => `${indent}/** ${text.replace(/\*\//g, "*&#47;")} */`;

function ktType(prop, naming, owner) {
  if (prop.choices) return `${owner}.${pascal(prop.name)}`;
  switch (prop.type) {
    case "number": return "Double";
    case "integer": return "Int";
    case "boolean": return "Boolean";
    case "number[]": return "List<Double>";
    case "number|number[]": return `${naming.typePrefix}Numbers`;
  }
  throw new Error(`kotlin: unhandled type ${prop.type}`);
}

function choiceEnum(prop, indent) {
  const cases = prop.choices.map((c) => `${upperSnake(c.caseName)}(${c.value})`).join(", ");
  return `${kdoc(prop.label + ".", indent)}\n${indent}enum class ${pascal(prop.name)}(val value: Int) { ${cases} }`;
}

function write(prop, indent, patternExpr) {
  const v = id(prop.name);
  const k = str(prop.key);
  switch (prop.type) {
    case "number": return `${indent}${v}?.let { o[${k}] = it }`;
    case "integer": return `${indent}${v}?.let { o[${k}] = it.toDouble() }`;
    case "boolean": return `${indent}${v}?.let { o[${k}] = if (it) 1.0 else 0.0 }`;
    case "choice": return `${indent}${v}?.let { o[${k}] = it.value.toDouble() }`;
    case "number[]":
      return `${indent}${v}?.let { listOf(${prop.engineKeys.map(str).join(", ")}).zip(it).forEach { (key, x) -> o[key] = x } }`;
    case "number|number[]": {
      const pats = prop.listPatterns.map(str).join(", ");
      return [
        `${indent}${v}?.let {`,
        `${indent}    val xs = when (it) { is ${"__N__"}.Value -> listOf(it.value); is ${"__N__"}.Values -> it.values }`,
        `${indent}    if (${patternExpr} in setOf(${pats})) listOf(${prop.engineKeys.map(str).join(", ")}).zip(xs).forEach { (key, x) -> o[key] = x }`,
        `${indent}    else xs.firstOrNull()?.let { x -> o[${k}] = x }`,
        `${indent}}`,
      ].join("\n");
    }
  }
  throw new Error(`kotlin: unhandled type ${prop.type}`);
}

function emitShared(models, naming) {
  const P = naming.typePrefix;
  const groups = sharedGroups(models);
  const groupClasses = groups.map((g) => {
    const enums = g.props.filter((p) => p.choices).map((p) => choiceEnum(p, "    "));
    const fields = g.props.map((p) => `${kdoc(docLine(p, models[0]), "    ")}\n    val ${id(p.name)}: ${ktType(p, naming, g.typeName)}? = null,`).join("\n");
    const writes = g.props.map((p) => write(p, "        ", "pattern")).join("\n");
    return `${kdoc(`${g.label}${g.description ? `: ${g.description}` : ""} (\`${g.path}.*\`; unset fields keep the pattern's value).`, "")}
data class ${g.typeName}(
${fields}
) {
${enums.length ? enums.join("\n") + "\n\n" : ""}    internal fun writeTo(o: MutableMap<String, Double>) {
${writes}
    }
}`;
  });
  const contents = `${header()}

package ${PKG}

import org.json.JSONObject

/** The sizes the engine resolves (dp). */
enum class ${P}Size(val px: UInt) { S20(20u), S32(32u), S64(64u) }

/** One value or one per item (ring \`progress\`: a number for arc/gauge/segmented, one per ring for tracking). */
sealed class ${P}Numbers {
    data class Value(val value: Double) : ${P}Numbers()
    data class Values(val values: List<Double>) : ${P}Numbers()

    companion object {
        fun of(value: Double): ${P}Numbers = Value(value)
        fun of(first: Double, second: Double, vararg rest: Double): ${P}Numbers = Values(listOf(first, second) + rest.toList())
        fun of(values: List<Double>): ${P}Numbers = Values(values)
    }
}

/** Why a typed component drew nothing. */
data class ${P}SpecError(val expected: String, val found: String) {
    override fun toString() =
        "this FX Spec describes a \\"$found\\" (its \`object\`), not a \\"$expected\\": use the matching component, or SinuaView(spec = …)"
}

/** Non-null when [spec] (JSON) names another object; unreadable specs are left to SinuaView to report. */
fun ${P.toLowerCase()}SpecError(spec: String, expected: String): ${P}SpecError? {
    val found = try { JSONObject(spec).optString("object", "") } catch (_: Exception) { "" }
    return if (found.isEmpty() || found == expected) null else ${P}SpecError(expected, found)
}

${groupClasses.join("\n\n")}
`;
  return { path: `${P}Shared.kt`, contents: contents.replaceAll("__N__", `${P}Numbers`) };
}

function emitComponent(m, naming) {
  const P = naming.typePrefix;
  const flat = flatProps(m);
  const props = `${m.typeName}Props`;
  const patternEnum = `${m.typeName}Pattern`;
  const enums = flat.filter((p) => p.choices).map((p) => choiceEnum(p, "    "));
  const fields = [
    ...flat.map((p) => `${kdoc(docLine(p, m), "    ")}\n    val ${id(p.name)}: ${ktType(p, naming, props)}? = null,`),
    ...m.groups.map((g) => `    val ${id(g.name)}: ${g.typeName}? = null,`),
  ].join("\n");
  const writes = flat.map((p) => write(p, "        ", "pattern.id")).join("\n");
  const groupWrites = m.groups.map((g) => `        ${id(g.name)}?.writeTo(o)`).join("\n");
  const params = [
    ...flat.map((p) => `    ${id(p.name)}: ${ktType(p, naming, props)}? = null,`),
    ...m.groups.map((g) => `    ${id(g.name)}: ${g.typeName}? = null,`),
  ].join("\n");
  const passArgs = ["pattern", "size", ...flat.map((p) => id(p.name)), ...m.groups.map((g) => id(g.name))].map((n) => `${n} = ${n}`).join(", ");
  const contents = `${header()}

package ${PKG}

import android.util.Log
import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import dev.sinua.view.FxFrameStats
import dev.sinua.view.FxLowPower
import dev.sinua.view.FxReducedMotion
import dev.sinua.view.FxTheme
import dev.sinua.view.SinuaView
import dev.sinua.voice.VoiceOverrides
import dev.sinua.voice.VoiceSource

/** ${m.label} patterns. */
enum class ${patternEnum}(val id: String) {
    ${m.patterns.map((p) => `${upperSnake(p.caseName)}(${str(p.id)})`).join(",\n    ")},
}

/** ${m.label} parameters; null keeps the pattern's value. [toOverrides] is what [${m.typeName}] hands to SinuaView. */
data class ${props}(
    val pattern: ${patternEnum},
    val size: ${P}Size = ${P}Size.S64,
${fields}
) {
${enums.length ? enums.join("\n") + "\n\n" : ""}    fun toOverrides(): Map<String, Double> {
        val o = LinkedHashMap<String, Double>()
${writes}
${groupWrites}
        return o
    }
}

/**
 * ${m.label}, typed: \`${m.typeName}(pattern = ${patternEnum}.${upperSnake(m.patterns[0].caseName)})\` plus any of its
 * parameters. A thin wrapper over SinuaView (no painting of its own).
 */
@Composable
fun ${m.typeName}(
    pattern: ${patternEnum},
    modifier: Modifier = Modifier,
    size: ${P}Size = ${P}Size.S64,
${params}
    speed: Double = 1.0,
    /**
     * The agent's lifecycle state ("listening", "speaking", ...): the built-in voice-state
     * behaviour then moves this pattern, under the parameters above. With a [voice] it
     * defaults to that source's state.
     */
    state: String? = null,
    inputs: Map<String, Double> = emptyMap(),
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
) {
    val overrides = ${props}(${passArgs}).toOverrides()
    SinuaView(
        pattern = pattern.id, modifier = modifier, size = size.px, overrides = overrides, speed = speed,
        state = state, inputs = inputs, voice = voice, voiceOverrides = voiceOverrides, theme = theme, paused = paused, reducedMotion = reducedMotion,
        contentDescription = contentDescription, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame,
    )
}

/**
 * Plays an FX Spec (JSON) that must describe a ${m.object} (\`"object": ${str(m.object)}\`); any other object
 * draws nothing and calls [onError] (logged when there is none). [state] picks the spec's lifecycle state.
 */
@Composable
fun ${m.typeName}(
    spec: String,
    modifier: Modifier = Modifier,
    state: String? = null,
    inputs: Map<String, Double> = emptyMap(),
    voiceLevelInput: String? = null,
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onError: ((${P}SpecError) -> Unit)? = null,
    onFrame: ((FxFrameStats) -> Unit)? = null,
) {
    val error = ${P.toLowerCase()}SpecError(spec, ${str(m.object)})
    if (error != null) {
        LaunchedEffect(error) { onError?.invoke(error) ?: Log.e("${m.typeName}", error.toString()) }
        Spacer(modifier)
        return
    }
    SinuaView(
        spec = spec, modifier = modifier, voice = voice, voiceOverrides = voiceOverrides, state = state,
        inputs = inputs, voiceLevelInput = voiceLevelInput, theme = theme, paused = paused, reducedMotion = reducedMotion,
        contentDescription = contentDescription, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame,
    )
}
`;
  return { path: `${m.typeName}.kt`, contents: contents.replaceAll("__N__", `${P}Numbers`) };
}

export function emit(models, naming) {
  return [emitShared(models, naming), ...models.map((m) => emitComponent(m, naming))];
}
