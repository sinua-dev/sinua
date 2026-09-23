// SwiftUI components (SinuaView module): one struct per object + shared types.
import { HEADER_LINES, sharedGroups, flatProps, docLine } from "./common.mjs";
import { pascal } from "../model.mjs";

const KEYWORDS = new Set(["as", "break", "case", "catch", "class", "continue", "default", "defer", "do", "else", "enum", "extension", "fallthrough", "false", "for", "func", "guard", "if", "import", "in", "init", "inout", "internal", "is", "let", "nil", "operator", "private", "protocol", "public", "repeat", "return", "self", "static", "struct", "subscript", "super", "switch", "throw", "throws", "true", "try", "var", "where", "while"]);
const id = (n) => (KEYWORDS.has(n) ? `\`${n}\`` : n);
const header = () => HEADER_LINES.map((l) => `// ${l}`).join("\n");
const str = (s) => JSON.stringify(s);

function swiftType(prop, naming) {
  if (prop.choices) return pascal(prop.name);
  switch (prop.type) {
    case "number": return "Double";
    case "integer": return "Int";
    case "boolean": return "Bool";
    case "number[]": return "[Double]";
    case "number|number[]": return `${naming.typePrefix}Numbers`;
  }
  throw new Error(`swift: unhandled type ${prop.type}`);
}

function choiceEnum(prop, indent) {
  const cases = prop.choices.map((c) => `${indent}    case ${id(c.caseName)} = ${c.value}`).join("\n");
  return `${indent}/// ${prop.label}.\n${indent}public enum ${pascal(prop.name)}: Int, CaseIterable, Sendable {\n${cases}\n${indent}}`;
}

/** Statement(s) writing one prop into `o` (a `[String: Double]`). `patternExpr` is the current pattern id. */
function write(prop, indent) {
  const v = id(prop.name);
  const k = str(prop.key);
  switch (prop.type) {
    case "number": return `${indent}if let v = ${v} { o[${k}] = v }`;
    case "integer": return `${indent}if let v = ${v} { o[${k}] = Double(v) }`;
    case "boolean": return `${indent}if let v = ${v} { o[${k}] = v ? 1 : 0 }`;
    case "choice": return `${indent}if let v = ${v} { o[${k}] = Double(v.rawValue) }`;
    case "number[]":
      return `${indent}if let v = ${v} { for (key, x) in zip([${prop.engineKeys.map(str).join(", ")}], v) { o[key] = x } }`;
    case "number|number[]": {
      const keys = `[${prop.engineKeys.map(str).join(", ")}]`;
      const isList = prop.listPatterns.map((p) => `pattern.rawValue == ${str(p)}`).join(" || ");
      return [
        `${indent}if let v = ${v} {`,
        `${indent}    let xs: [Double] = { switch v { case .value(let x): return [x]; case .list(let l): return l } }()`,
        `${indent}    if ${isList} { for (key, x) in zip(${keys}, xs) { o[key] = x } } else if let x = xs.first { o[${k}] = x }`,
        `${indent}}`,
      ].join("\n");
    }
  }
  throw new Error(`swift: unhandled type ${prop.type}`);
}

function emitShared(models, naming) {
  const P = naming.typePrefix;
  const groups = sharedGroups(models);
  const groupStructs = groups.map((g) => {
    const props = g.props;
    const enums = props.filter((p) => p.choices).map((p) => choiceEnum(p, "    ")).join("\n\n");
    const fields = props.map((p) => `    /// ${docLine(p, models[0])}\n    public var ${id(p.name)}: ${swiftType(p, naming)}?`).join("\n");
    const params = props.map((p) => `${id(p.name)}: ${swiftType(p, naming)}? = nil`).join(", ");
    const assigns = props.map((p) => `        self.${p.name} = ${id(p.name)}`).join("\n");
    const writes = props.map((p) => write(p, "        ")).join("\n");
    return `/// ${g.label}${g.description ? `: ${g.description}` : ""} (\`${g.path}.*\`; unset fields keep the pattern's value).
public struct ${g.typeName}: Equatable, Sendable {
${enums ? enums + "\n\n" : ""}${fields}

    public init(${params}) {
${assigns}
    }

    func write(into o: inout [String: Double]) {
${writes}
    }
}`;
  });
  const contents = `${header()}

import Foundation

/// The sizes the engine resolves (points).
public enum ${P}Size: UInt32, CaseIterable, Sendable {
    case s20 = 20, s32 = 32, s64 = 64
}

/// One value or one per item (ring \`progress\`: a number for arc/gauge/segmented, one per ring for tracking).
public enum ${P}Numbers: Equatable, Sendable, ExpressibleByFloatLiteral, ExpressibleByIntegerLiteral, ExpressibleByArrayLiteral {
    case value(Double)
    case list([Double])
    public init(floatLiteral v: Double) { self = .value(v) }
    public init(integerLiteral v: Int) { self = .value(Double(v)) }
    public init(arrayLiteral xs: Double...) { self = .list(xs) }
}

/// Why a typed component drew nothing.
public struct ${P}SpecError: Error, Equatable, CustomStringConvertible {
    public let expected: String
    public let found: String
    public var description: String {
        "this FX Spec describes a \\"\\(found)\\" (its \`object\`), not a \\"\\(expected)\\": use the matching component, or SinuaView(spec:)"
    }
}

/// The spec's top-level \`object\`, or nil when it can't be read (SinuaView then reports the spec itself).
func ${P.toLowerCase()}SpecObject(_ spec: String) -> String? {
    guard let data = spec.data(using: .utf8),
          let doc = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
    return doc["object"] as? String
}

${groupStructs.join("\n\n")}
`;
  return { path: `${P}Shared.swift`, contents };
}

function emitComponent(m, models, naming) {
  const P = naming.typePrefix;
  const flat = flatProps(m).slice().sort((a, b) => a.name.localeCompare(b.name));
  const groups = m.groups.slice().sort((a, b) => a.name.localeCompare(b.name));
  const enums = flat.filter((p) => p.choices).map((p) => choiceEnum(p, "    "));
  const patternCases = m.patterns.map((p) => `        case ${id(p.caseName)} = ${str(p.id)}`).join("\n");
  const fields = [
    ...flat.map((p) => `    /// ${docLine(p, m)}\n    public var ${id(p.name)}: ${swiftType(p, naming)}?`),
    ...groups.map((g) => `    public var ${id(g.name)}: ${g.typeName}?`),
  ].join("\n");
  const propParams = [...flat.map((p) => `${id(p.name)}: ${swiftType(p, naming)}? = nil`), ...groups.map((g) => `${id(g.name)}: ${g.typeName}? = nil`)];
  const propAssigns = [...flat, ...groups].map((p) => `        self.${p.name} = ${id(p.name)}`).join("\n");
  const writes = flat.map((p) => write(p, "        ")).join("\n");
  const groupWrites = groups.map((g) => `        ${id(g.name)}?.write(into: &o)`).join("\n");
  const firstPattern = m.patterns[0].caseName;
  const contents = `${header()}

import SwiftUI
import SinuaVoiceTypes

/// ${m.label}, typed: \`${m.typeName}(pattern: .${firstPattern})\` plus any of its parameters.
/// A thin wrapper over \`SinuaView\` (no painting of its own): typed props become engine overrides,
/// or \`init(spec:)\` plays an FX Spec that must describe a ${m.object}.
public struct ${m.typeName}: View {
    public enum Pattern: String, CaseIterable, Sendable {
${patternCases}
    }
${enums.length ? "\n" + enums.join("\n\n") + "\n" : ""}
    public var pattern: Pattern
    public var size: ${P}Size
    /// The agent's lifecycle state ("listening", "speaking", ...). Without a spec it picks the
    /// built-in voice-state behaviour for this pattern, under your own parameters; with a
    /// \`spec\` it picks that file's \`states\` entry. With a \`voice\`, it defaults to the source's state.
    public var state: String?
${fields}
    public var speed: Double
    public var voice: VoiceSource?
    public var voiceOverrides: VoiceOverrides?
    public var theme: FxTheme
    public var paused: Bool
    public var reducedMotion: FxReducedMotion
    public var accessibilityLabel: String?
    public var maxFps: Double?
    public var lowPower: FxLowPower
    public var onFrame: ((FxFrameStats) -> Void)?
    private var spec: String?
    public var inputs: [String: Double] = [:]
    private var voiceLevelInput: String?
    private var onError: ((${P}SpecError) -> Void)?

    public init(
        pattern: Pattern,
        size: ${P}Size = .s64,
        state: String? = nil,
        inputs: [String: Double] = [:],
        ${propParams.join(",\n        ")},
        speed: Double = 1,
        voice: VoiceSource? = nil,
        voiceOverrides: VoiceOverrides? = nil,
        theme: FxTheme = .auto,
        paused: Bool = false,
        reducedMotion: FxReducedMotion = .auto,
        accessibilityLabel: String? = nil,
        maxFps: Double? = nil,
        lowPower: FxLowPower = .auto,
        onFrame: ((FxFrameStats) -> Void)? = nil
    ) {
        self.pattern = pattern
        self.size = size
        self.state = state
        self.inputs = inputs
${propAssigns}
        self.speed = speed
        self.voice = voice
        self.voiceOverrides = voiceOverrides
        self.theme = theme
        self.paused = paused
        self.reducedMotion = reducedMotion
        self.accessibilityLabel = accessibilityLabel
        self.maxFps = maxFps
        self.lowPower = lowPower
        self.onFrame = onFrame
    }

    /// Plays an FX Spec (JSON). It must describe a ${m.object} (\`"object": ${str(m.object)}\`): any other
    /// object draws nothing and calls \`onError\` (without one, a debug build stops at an assertion).
    /// \`state\` picks the spec's lifecycle state.
    public init(
        spec: String,
        state: String? = nil,
        inputs: [String: Double] = [:],
        voiceLevelInput: String? = nil,
        voice: VoiceSource? = nil,
        voiceOverrides: VoiceOverrides? = nil,
        theme: FxTheme = .auto,
        paused: Bool = false,
        reducedMotion: FxReducedMotion = .auto,
        accessibilityLabel: String? = nil,
        maxFps: Double? = nil,
        lowPower: FxLowPower = .auto,
        onError: ((${P}SpecError) -> Void)? = nil,
        onFrame: ((FxFrameStats) -> Void)? = nil
    ) {
        self.init(pattern: .${firstPattern}, voice: voice, voiceOverrides: voiceOverrides, theme: theme, paused: paused,
                  reducedMotion: reducedMotion, accessibilityLabel: accessibilityLabel, maxFps: maxFps, lowPower: lowPower, onFrame: onFrame)
        self.spec = spec
        self.state = state
        self.inputs = inputs
        self.voiceLevelInput = voiceLevelInput
        self.onError = onError
    }

    /// The engine overrides these props produce (unset props keep the pattern's values).
    public func overrides() -> [String: Double] {
        var o: [String: Double] = [:]
${writes}
${groupWrites}
        return o
    }

    /// Non-nil when \`spec\` describes another object.
    public var specError: ${P}SpecError? {
        guard let spec, let found = ${P.toLowerCase()}SpecObject(spec), found != ${str(m.object)} else { return nil }
        return ${P}SpecError(expected: ${str(m.object)}, found: found)
    }

    public var body: some View {
        if let spec {
            if let error = specError {
                Color.clear.onAppear {
                    if let onError { onError(error) } else { assertionFailure("${m.typeName}: \\(error)") }
                }
            } else {
                SinuaView(spec: spec, voice: voice, voiceOverrides: voiceOverrides, state: state, inputs: inputs,
                       voiceLevelInput: voiceLevelInput, theme: theme, paused: paused, reducedMotion: reducedMotion,
                       accessibilityLabel: accessibilityLabel, maxFps: maxFps, lowPower: lowPower, onFrame: onFrame)
            }
        } else {
            SinuaView(pattern: pattern.rawValue, size: size.rawValue, overrides: overrides(), speed: speed, state: state, inputs: inputs, voice: voice,
                   voiceOverrides: voiceOverrides, theme: theme, paused: paused, reducedMotion: reducedMotion,
                   accessibilityLabel: accessibilityLabel, maxFps: maxFps, lowPower: lowPower, onFrame: onFrame)
        }
    }
}
`;
  return { path: `${m.typeName}.swift`, contents };
}

export function emit(models, naming) {
  return [emitShared(models, naming), ...models.map((m) => emitComponent(m, models, naming))];
}
