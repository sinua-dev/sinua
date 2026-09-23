import XCTest

@testable import SinuaVoice

/// The raw-API-key gate (`InsecureCredential`). The same rule and the same
/// message text exist on Web (`packages/voice/test/sources.test.mjs`) and
/// Android (`InsecureCredentialTest.kt`); these assert the shared shape so the
/// three can't drift apart silently.
final class InsecureCredentialTests: XCTestCase {
    private func refuse(_ body: () throws -> Void) -> String? {
        do {
            try body()
            return nil
        } catch let e as InsecureCredential.Refused { return e.message } catch { return "\(error)" }
    }

    func testEphemeralCredentialPassesAndWarnsAboutNothing() {
        var warnings: [String] = []
        XCTAssertNil(
            refuse {
                try InsecureCredential.check(
                    vendor: "GeminiLiveVoiceSource", isEphemeral: true,
                    allowInsecureApiKey: false,
                    ephemeralShape: InsecureCredential.geminiShape,
                    mintHint: InsecureCredential.geminiMintHint,
                    warn: { warnings.append($0) })
            })
        XCTAssertTrue(warnings.isEmpty)
    }

    func testRawKeyIsRefusedWithoutTheOptIn() {
        let message = refuse {
            try InsecureCredential.check(
                vendor: "GeminiLiveVoiceSource", isEphemeral: false,
                allowInsecureApiKey: false,
                ephemeralShape: InsecureCredential.geminiShape,
                mintHint: InsecureCredential.geminiMintHint,
                warn: { _ in XCTFail("a refusal must not also warn") })
        }
        let text = try! XCTUnwrap(message)
        XCTAssertTrue(text.contains("refusing a raw, long-lived API key"), text)
        XCTAssertTrue(text.contains("auth_tokens/…"), "says what to pass instead: \(text)")
        XCTAssertTrue(text.contains("v1beta/auth_tokens"), "says how to mint one: \(text)")
        XCTAssertTrue(text.contains("allowInsecureApiKey"), "says how to opt in: \(text)")
    }

    func testTheOptInAllowsItAndWarnsExactlyOnce() {
        var warnings: [String] = []
        XCTAssertNil(
            refuse {
                try InsecureCredential.check(
                    vendor: "OpenAIRealtimeVoiceSource", isEphemeral: false,
                    allowInsecureApiKey: true,
                    ephemeralShape: InsecureCredential.openAIShape,
                    mintHint: InsecureCredential.openAIMintHint,
                    warn: { warnings.append($0) })
            })
        XCTAssertEqual(warnings.count, 1)
        XCTAssertTrue(warnings[0].contains("local demos, not for shipping"), warnings[0])
    }

    func testCredentialShapes() {
        XCTAssertTrue(InsecureCredential.isGeminiEphemeral("auth_tokens/abc"))
        XCTAssertTrue(InsecureCredential.isGeminiEphemeral("  auth_tokens/abc  "), "trims, as the sources do")
        XCTAssertFalse(InsecureCredential.isGeminiEphemeral("AIzaKEY"))
        XCTAssertTrue(InsecureCredential.isOpenAIEphemeral("ek_abc"))
        XCTAssertFalse(InsecureCredential.isOpenAIEphemeral("sk-abc"))
    }
}
