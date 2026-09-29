import XCTest

@testable import SinuaVoice

/// The shared credential contract (`CredentialSource`), the Swift port of
/// `packages/voice/src/credential.ts` and the same rules as its tests: one JSON
/// shape, a provider or URL asked on every resolve, 4xx/bad shape fatal, 5xx/429
/// retryable, and the credential never in a message. No network: a fake HTTP.
final class CredentialTests: XCTestCase {
    private final class FakeHTTP: @unchecked Sendable {
        var requests: [URLRequest] = []
        var status = 200
        var body = Data()
        var error: Error?
        lazy var http: CredentialHTTP = { [unowned self] req in
            self.requests.append(req)
            if let error = self.error { throw error }
            return (self.status, self.body)
        }
    }

    func testValueProviderAndURLResolveToTheSharedShape() async throws {
        let v = try await CredentialSource.value(" ek_a ").resolve(vendor: "V")
        XCTAssertEqual(v, SinuaCredential(credential: "ek_a"))
        XCTAssertFalse(CredentialSource.value("x").canRefresh)

        var calls = 0
        let p = CredentialSource.provider {
            calls += 1
            return SinuaCredential(credential: "ek_\(calls)", expiresAt: 5)
        }
        XCTAssertTrue(p.canRefresh)
        _ = try await p.resolve(vendor: "V")
        let second = try await p.resolve(vendor: "V")
        XCTAssertEqual(second, SinuaCredential(credential: "ek_2", expiresAt: 5), "asked again on every resolve")

        let fake = FakeHTTP()
        fake.body = Data(#"{"credential":"jwt","url":"wss://lk.example","expiresAt":9}"#.utf8)
        let u = try await CredentialSource.url(URL(string: "https://app.example/api/voice/livekit")!, http: fake.http)
            .resolve(vendor: "V", needsURL: true)
        XCTAssertEqual(u, SinuaCredential(credential: "jwt", expiresAt: 9, url: "wss://lk.example"))
        let req = try XCTUnwrap(fake.requests.first)
        XCTAssertEqual(req.httpMethod, "POST")
        XCTAssertEqual(req.cachePolicy, .reloadIgnoringLocalCacheData)
        XCTAssertEqual(req.value(forHTTPHeaderField: "Accept"), "application/json")
    }

    func testBadShapesAreFatalAndNeverEchoTheValue() {
        for body in [
            "", "<html>", "[]", "{}", #"{"key":"ek_SECRET"}"#, #"{"credential":1}"#, #"{"credential":"  "}"#,
            #"{"credential":"x","expiresAt":"soon"}"#, #"{"credential":"x","expiresAt":true}"#,
            #"{"credential":"x","url":2}"#,
        ] {
            XCTAssertThrowsError(try CredentialSource.decode(vendor: "V", Data(body.utf8)), body) {
                XCTAssertEqual(($0 as? CredentialError)?.isFatal, true, body)
                XCTAssertFalse($0.localizedDescription.contains("SECRET"), "names keys, never values")
            }
        }
    }

    func testHTTPStatusesSplitIntoFatalAndRetryable() async {
        let fake = FakeHTTP()
        let source = CredentialSource.url(URL(string: "https://a.example/c")!, http: fake.http)
        for (status, fatal) in [(401, true), (404, true), (408, false), (429, false), (500, false), (503, false)] {
            fake.status = status
            do {
                _ = try await source.resolve(vendor: "V")
                XCTFail("\(status) should throw")
            } catch let e as CredentialError {
                XCTAssertEqual(e.isFatal, fatal, "\(status)")
            } catch { XCTFail("\(error)") }
        }
        fake.error = URLError(.notConnectedToInternet)
        do {
            _ = try await source.resolve(vendor: "V")
            XCTFail("a transport error should throw")
        } catch let e as CredentialError { XCTAssertFalse(e.isFatal, "network errors are retried") } catch {
            XCTFail("\(error)")
        }
    }

    func testLiveKitNeedsAURL() async {
        do {
            _ = try await CredentialSource.value("jwt").resolve(vendor: "V", needsURL: true)
            XCTFail("expected a missing-url error")
        } catch let e as CredentialError {
            XCTAssertTrue(e.isFatal)
            XCTAssertTrue(e.localizedDescription.contains("no `url`"))
        } catch { XCTFail("\(error)") }
    }
}
