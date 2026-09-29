import XCTest

@testable import SinuaVoice

/// The raw-API-key rule (`InsecureCredential`): always refused, no opt-in. The
/// same rule and the same message text exist on Web
/// (`packages/voice/src/insecureCredential.ts`) and Android
/// (`InsecureCredentialTest.kt`); this pins the exact text so the three can't drift.
final class InsecureCredentialTests: XCTestCase {
    func testEphemeralCredentialPasses() throws {
        XCTAssertNil(
            InsecureCredential.refusal(
                vendor: "GeminiLiveVoiceSource", isEphemeral: true, ephemeralShape: InsecureCredential.geminiShape))
        try InsecureCredential.check(
            vendor: "OpenAIRealtimeVoiceSource", isEphemeral: true, ephemeralShape: InsecureCredential.openAIShape)
    }

    func testRawKeyIsAlwaysRefusedWithTheWebMessage() {
        XCTAssertEqual(
            InsecureCredential.refusal(
                vendor: "GeminiLiveVoiceSource", isEphemeral: false, ephemeralShape: InsecureCredential.geminiShape),
            "GeminiLiveVoiceSource: expected a short-lived credential (auth_tokens/…); refusing what looks like a raw, "
                + "long-lived API key. Mint one in your backend with @sinua/voice/server, or run "
                + "`npx @sinua/voice dev-proxy` and pass `credentialUrl`.")
        XCTAssertThrowsError(
            try InsecureCredential.check(
                vendor: "OpenAIRealtimeVoiceSource", isEphemeral: false, ephemeralShape: InsecureCredential.openAIShape)
        ) { XCTAssertEqual(($0 as? CredentialError)?.isFatal, true, "a refusal is never retried") }
    }

    func testCredentialShapes() {
        XCTAssertTrue(InsecureCredential.isGeminiEphemeral("auth_tokens/abc"))
        XCTAssertTrue(InsecureCredential.isGeminiEphemeral("  auth_tokens/abc  "), "trims, as the sources do")
        XCTAssertFalse(InsecureCredential.isGeminiEphemeral("AIzaKEY"))
        XCTAssertTrue(InsecureCredential.isOpenAIEphemeral("ek_abc"))
        XCTAssertFalse(InsecureCredential.isOpenAIEphemeral("sk-abc"))
    }
}
