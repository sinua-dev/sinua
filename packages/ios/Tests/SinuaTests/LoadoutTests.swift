import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// Loadouts (FX Spec 1.13, design note 25): the shared vectors (spec/loadout-vectors.json,
/// also checked by Rust, wasm and Android), what fits, thumbnails, and the wear clock.
final class LoadoutTests: XCTestCase {
    private func specURL(_ path: String) -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/\(path)")
    }

    private func json(_ path: String) throws -> Any {
        try JSONSerialization.jsonObject(with: Data(contentsOf: specURL(path)), options: [.fragmentsAllowed])
    }

    func testTheSharedLoadoutVectorsHold() throws {
        let v = try XCTUnwrap(json("loadout-vectors.json") as? [String: Any])
        let spec = try String(contentsOf: specURL(try XCTUnwrap(v["spec"] as? String)), encoding: .utf8)
        // A `fits` entry may carry its own spec (the capability-tag cases).
        func own(_ f: [String: Any]) throws -> String {
            guard let o = f["spec"] else { return spec }
            return String(decoding: try JSONSerialization.data(withJSONObject: o), as: UTF8.self)
        }
        for c in try XCTUnwrap(v["cases"] as? [[String: Any]]) {
            let name = c["name"] as? String ?? "?"
            let lo = try JSONSerialization.data(withJSONObject: c["loadout"] as Any, options: [.fragmentsAllowed])
            let r = applyLoadout(spec: spec, loadout: String(decoding: lo, as: UTF8.self))
            let out = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(r.spec.utf8)) as? [String: Any])
            let worn = (out["cosmetics"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String }
            XCTAssertEqual(worn, c["wear"] as? [String], "\(name): worn")
            XCTAssertEqual(r.diagnostics.map(\.path).sorted(), c["warnings"] as? [String], "\(name): warnings")
            XCTAssertTrue(r.diagnostics.allSatisfy { $0.severity == "warning" }, name)
            let eye = (out["params"] as? [String: Any])?["eyeStyle"] as? String
            XCTAssertEqual(eye, c["eyeStyle"] as? String, "\(name): eyeStyle")
            XCTAssertTrue(resolveFxSpec(json: r.spec).ok, "\(name): still draws")
        }
        for f in try XCTUnwrap(v["fits"] as? [[String: Any]]) {
            let ch = try XCTUnwrap(f["character"] as? String)
            let got = Dictionary(
                uniqueKeysWithValues: SinuaCosmeticFit.list(spec: try own(f), character: ch).map { ($0.id, $0.reason) })
            XCTAssertEqual(got, f["expect"] as? [String: String], ch)
        }
    }

    func testAStoredLoadoutReadsBackEvenWithMissingFields() throws {
        let l = try JSONDecoder().decode(SinuaLoadout.self, from: Data(#"{"wear":["party-hat"]}"#.utf8))
        XCTAssertEqual(l, SinuaLoadout(wear: ["party-hat"]))
        XCTAssertEqual(try JSONDecoder().decode(SinuaLoadout.self, from: Data(l.json.utf8)), l)
    }

    @available(iOS 16.0, *)
    @MainActor
    func testAThumbnailIsAStillImage() throws {
        let spec = try String(contentsOf: specURL("examples/wardrobe-bean.fxspec.json"), encoding: .utf8)
        let a = try XCTUnwrap(
            SinuaThumbnail.image(spec: spec, loadout: SinuaLoadout(wear: ["round-glasses"]), size: 64))
        XCTAssertEqual(a.width, 128)
        let f1 = frameStill(spec: spec, loadout: SinuaLoadout(wear: ["round-glasses"]).json, size: 64, turnYaw: 0)
        let f2 = frameStill(spec: spec, loadout: SinuaLoadout(wear: ["round-glasses"]).json, size: 64, turnYaw: 0)
        XCTAssertEqual(f1, f2)
        XCTAssertNil(SinuaThumbnail.image(spec: "{", size: 64))
    }

    func testTheWearClockRunsBesideAStateChange() {
        var tr = StateTransition()
        let bean = TransitionSide(state: "bean", speed: 1, overrides: [:])
        _ = tr.frames(bean, size: 64, t: 1, extra: [:])
        tr.start(duration: 0.6, curve: "easeInOut")
        tr.advance(0.1)
        tr.wear()
        XCTAssertTrue(tr.wearing && tr.active)
        tr.advance(wearSeconds)
        XCTAssertTrue(!tr.wearing && tr.active, "the loadout change ends first, the state change runs on")
        tr.wear()
        tr.cancel()
        XCTAssertFalse(tr.wearing || tr.active)
    }

    /// Sinua's catalog pack (design note 26): the bundled resource is spec/catalog/catalog-1.json,
    /// it loads, and a spec wears and paints from it.
    func testTheBundledCatalogLoadsAndIsTheSpecFile() throws {
        let bundled = try XCTUnwrap(SinuaCatalog.json)
        let a = try JSONSerialization.jsonObject(with: Data(bundled.utf8)) as? NSDictionary
        let b = try XCTUnwrap(json("catalog/catalog-1.json") as? NSDictionary)
        XCTAssertEqual(a, b)
        XCTAssertEqual(SinuaCatalog.load(), [])
        let r = resolveFxSpec(
            json:
                #"{"fxSpec":"1.13","object":"character","pattern":"bean","cosmetics":["catalog:crown"],"palette":"catalog:berry"}"#
        )
        XCTAssertTrue(r.ok && r.diagnostics.isEmpty, "\(r.diagnostics)")
        XCTAssertTrue(r.state.hasPrefix("recipe:bean:"))
        XCTAssertTrue(unloadCatalog(namespace: "catalog"))
    }
}
