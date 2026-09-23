import Foundation
import React

/// React Native bridge module. `frame(state:size:t:)` below is the
/// UniFFI-generated free function (vendored `core_engine.swift`, same file
/// `packages/ios` uses) -- this class only converts its `OrbFrame` struct
/// into the `[String: Any]` shape the RN bridge can serialize to JS.
@objc(SinuaCore)
class SinuaCore: NSObject {

  @objc(resolveFrame:size:t:resolver:rejecter:)
  func resolveFrame(
    _ state: String,
    size: NSNumber,
    t: NSNumber,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    guard let sizeUInt = UInt32(exactly: size) else {
      reject("invalid_size", "size must be a non-negative integer", nil)
      return
    }
    guard let result = frame(state: state, size: sizeUInt, t: t.doubleValue) else {
      resolve(NSNull())
      return
    }
    resolve(SinuaCore.dictionary(from: result))
  }

  @objc(resolveFrameWithOverrides:size:t:overrides:resolver:rejecter:)
  func resolveFrameWithOverrides(
    _ state: String,
    size: NSNumber,
    t: NSNumber,
    overrides: NSDictionary,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    guard let sizeUInt = UInt32(exactly: size) else {
      reject("invalid_size", "size must be a non-negative integer", nil)
      return
    }
    // `NSDictionary` isn't directly iterable as (key, value) pairs the way
    // a bridged Swift `[String: Any]` is -- cast first, or this silently
    // produces zero entries instead of an error (caught by
    // packages/react-native/example's smoke test, not a type error here).
    var overridesMap: [String: Double] = [:]
    if let overridesDict = overrides as? [String: Any] {
      for (key, value) in overridesDict {
        guard let numberValue = value as? NSNumber else { continue }
        overridesMap[key] = numberValue.doubleValue
      }
    }
    guard
      let result = frameWithOverrides(
        state: state, size: sizeUInt, t: t.doubleValue, overrides: overridesMap)
    else {
      resolve(NSNull())
      return
    }
    resolve(SinuaCore.dictionary(from: result))
  }

  // FX Spec (docs/fx-spec.md): parse/validate/resolve all happen in Rust;
  // these only carry the JSON text in and the records out.
  @objc(resolveFxSpec:state:inputs:lowPower:resolver:rejecter:)
  func resolveFxSpecBridge(
    _ json: String,
    state: String?,
    inputs: NSDictionary?,
    lowPower: Bool,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    let r = resolveFxSpecWith(
      json: json, state: state, inputs: SinuaCore.numbers(inputs), lowPower: lowPower)
    resolve([
      "ok": r.ok, "state": r.state, "size": r.size, "speed": r.speed,
      "overrides": r.overrides,
      "diagnostics": r.diagnostics.map {
        ["severity": $0.severity, "path": $0.path, "message": $0.message]
      },
      "stateKey": r.stateKey, "stateKeys": r.stateKeys, "inactiveBindings": r.inactiveBindings,
      "maxFps": r.maxFps.map { $0 as Any } ?? NSNull(), "disabledMaterials": r.disabledMaterials,
    ] as [String: Any])
  }

  @objc(frameFromFxSpec:elapsed:state:inputs:lowPower:resolver:rejecter:)
  func frameFromFxSpecBridge(
    _ json: String,
    elapsed: NSNumber,
    state: String?,
    inputs: NSDictionary?,
    lowPower: Bool,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    guard
      let result = frameFromFxSpecWith(
        json: json, elapsed: elapsed.doubleValue, state: state, inputs: SinuaCore.numbers(inputs),
        lowPower: lowPower)
    else {
      resolve(NSNull())
      return
    }
    resolve(SinuaCore.dictionary(from: result))
  }

  /// JS `{ name: number }` -> `[String: Double]` (non-numbers skipped; see
  /// the NSDictionary note in `resolveFrameWithOverrides`).
  private static func numbers(_ dict: NSDictionary?) -> [String: Double] {
    var out: [String: Double] = [:]
    for (key, value) in (dict as? [String: Any]) ?? [:] {
      if let n = value as? NSNumber { out[key] = n.doubleValue }
    }
    return out
  }

  @objc(fxColorToHsl:resolver:rejecter:)
  func fxColorToHslBridge(
    _ color: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    guard let c = fxColorToHsl(color: color) else {
      resolve(NSNull())
      return
    }
    resolve(["h": c.h, "s": c.s, "l": c.l, "achromatic": c.achromatic, "hex": c.hex] as [String: Any])
  }

  /// Field-for-field mirror of `packages/core`'s TS `OrbFrame` (the wasm
  /// JSON shape), so a consumer's paint code is identical on Web and RN.
  private static func dictionary(from result: OrbFrame) -> [String: Any] {
    var out = base(from: result)
    // Materials phase 1: present only when non-empty, like the wasm JSON.
    if !result.fills.isEmpty {
      out["fills"] = result.fills.map { f -> [String: Any] in
        var d: [String: Any] = [
          "points": f.points.map { ["x": $0.x, "y": $0.y] },
          "white": f.white, "a": f.a, "saturation": f.saturation, "hue": f.hue,
          "blur": f.blur, "blend": Int(f.blend), "gradient": NSNull(),
        ]
        if !f.holes.isEmpty {
          d["holes"] = f.holes.map { ring in ring.map { ["x": $0.x, "y": $0.y] } }
        }
        if let g = f.gradient {
          d["gradient"] = [
            "kind": Int(g.kind), "x0": g.x0, "y0": g.y0, "x1": g.x1, "y1": g.y1, "r": g.r,
            "stops": g.stops.map {
              ["offset": $0.offset, "white": $0.white, "a": $0.a, "saturation": $0.saturation, "hue": $0.hue]
            },
          ] as [String: Any]
        }
        return d
      }
    }
    if !result.effects.isEmpty {
      out["effects"] = result.effects.map {
        ["target": Int($0.target), "start": Int($0.start), "count": Int($0.count), "blur": $0.blur, "blend": Int($0.blend)]
          as [String: Any]
      }
    }
    return out
  }

  private static func base(from result: OrbFrame) -> [String: Any] {
    [
      "dots": result.dots.map { dot in
        [
          "x": dot.x, "y": dot.y, "z": dot.z,
          "r": dot.r, "white": dot.white, "a": dot.a,
          "saturation": dot.saturation, "hue": dot.hue,
        ]
      },
      "lines": result.lines.map { line in
        [
          "x1": line.x1, "y1": line.y1, "x2": line.x2, "y2": line.y2,
          "white": line.white, "a": line.a, "w": line.w,
          "saturation": line.saturation, "hue": line.hue,
        ]
      },
      "polylines": result.polylines.map { polyline in
        [
          "points": polyline.points.map { ["x": $0.x, "y": $0.y] },
          "white": polyline.white, "a": polyline.a, "w": polyline.w,
          "saturation": polyline.saturation, "hue": polyline.hue,
        ]
      },
      "colorMode": result.colorMode == .fixed ? "fixed" : "ink",
    ]
  }

  @objc
  static func requiresMainQueueSetup() -> Bool {
    false
  }
}
