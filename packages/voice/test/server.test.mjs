// @sinua/voice/server and the dev proxy. Plain node: no DOM (the helper refuses
// to run where there is one), vendor HTTP is a recorded fake, and the proxy
// listens on an ephemeral 127.0.0.1 port. No vendor is ever called.
import { test } from "node:test";
import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import {
  credentialResponse,
  mintGeminiLiveCredential,
  mintLiveKitCredential,
  mintOpenAIRealtimeCredential,
  signElevenLabsUrl,
} from "../dist/server.js";
import { createHandler, loadEnv, parseEnv, startDevProxy } from "../bin/dev-proxy.mjs";

const KEY = "sk-SECRET-123";

/** A fetch that records calls and answers with `body` / `status`. */
function vendor(body, status = 200) {
  const calls = [];
  const fetch = async (url, init = {}) => {
    calls.push({ url: String(url), init, body: init.body ? JSON.parse(init.body) : undefined });
    return new Response(typeof body === "string" ? body : JSON.stringify(body), { status });
  };
  return { fetch, calls };
}

test("openai: client_secrets body carries model/voice/instructions; maps to the shared shape", async () => {
  const v = vendor({ value: "ek_1", expires_at: 1700000600 });
  const c = await mintOpenAIRealtimeCredential({ apiKey: KEY, model: "gpt-realtime", voice: "marin", instructions: "Be brief.", fetch: v.fetch });
  assert.deepEqual(c, { credential: "ek_1", expiresAt: 1700000600 });
  const [call] = v.calls;
  assert.equal(call.url, "https://api.openai.com/v1/realtime/client_secrets");
  assert.equal(call.init.headers.Authorization, `Bearer ${KEY}`);
  assert.deepEqual(call.body.expires_after, { anchor: "created_at", seconds: 600 });
  assert.equal(call.body.session.model, "gpt-realtime");
  assert.equal(call.body.session.audio.output.voice, "marin");
  assert.equal(call.body.session.instructions, "Be brief.");
  assert.equal(call.body.session.audio.input.turn_detection.type, "server_vad");
});

test("gemini: auth_tokens body locks model, voice and instructions -- and nothing the client resumes with", async () => {
  const v = vendor({ name: "auth_tokens/abc" });
  const before = Math.floor(Date.now() / 1000);
  const c = await mintGeminiLiveCredential({ apiKey: KEY, model: "gemini-3.8-live", voice: "Kore", instructions: "Be brief.", fetch: v.fetch });
  assert.equal(c.credential, "auth_tokens/abc");
  assert.ok(c.expiresAt >= before + 1800 && c.expiresAt <= before + 1801);
  const [call] = v.calls;
  assert.equal(call.url, "https://generativelanguage.googleapis.com/v1beta/auth_tokens");
  assert.equal(call.init.headers["x-goog-api-key"], KEY);
  assert.equal(call.body.uses, 1);
  const setup = call.body.bidiGenerateContentSetup;
  assert.equal(setup.model, "models/gemini-3.8-live");
  assert.deepEqual(setup.systemInstruction, { parts: [{ text: "Be brief." }] });
  assert.equal(setup.generationConfig.speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName, "Kore");
  const mask = call.body.fieldMask.split(",");
  assert.deepEqual(mask.sort(), ["generationConfig.responseModalities", "generationConfig.speechConfig", "model", "systemInstruction"]);
  assert.ok(!mask.includes("sessionResumption"), "the client's resumption handle must stay the client's");
  assert.ok(Date.parse(call.body.newSessionExpireTime) - Date.now() <= 61_000);
});

test("elevenlabs: signed URL with the xi-api-key header, 15 minutes", async () => {
  const v = vendor({ signed_url: "wss://api.elevenlabs.io/v1/convai/conversation?agent_id=a&conversation_signature=s" });
  const c = await signElevenLabsUrl({ apiKey: KEY, agentId: "agent 1", fetch: v.fetch });
  assert.match(c.credential, /^wss:\/\//);
  assert.ok(c.expiresAt - Math.floor(Date.now() / 1000) >= 899);
  assert.equal(v.calls[0].url, "https://api.elevenlabs.io/v1/convai/conversation/get-signed-url?agent_id=agent%201");
  assert.equal(v.calls[0].init.headers["xi-api-key"], KEY);
});

test("livekit: an HS256 JWT that verifies, scoped to one room", async () => {
  const c = await mintLiveKitCredential({ apiKey: "APIkey", apiSecret: "s3cret", url: "wss://lk.example", room: "r1", identity: "u1", ttlSeconds: 60 });
  assert.equal(c.url, "wss://lk.example");
  const [h, p, s] = c.credential.split(".");
  const dec = (x) => JSON.parse(Buffer.from(x, "base64url").toString());
  assert.deepEqual(dec(h), { alg: "HS256", typ: "JWT" });
  const claims = dec(p);
  assert.equal(claims.iss, "APIkey");
  assert.equal(claims.sub, "u1");
  assert.equal(claims.exp, c.expiresAt);
  assert.equal(claims.exp - claims.nbf, 60);
  assert.deepEqual(claims.video, { roomJoin: true, room: "r1", canPublish: true, canSubscribe: true, canPublishData: true });
  const key = await webcrypto.subtle.importKey("raw", new TextEncoder().encode("s3cret"), { name: "HMAC", hash: "SHA-256" }, false, ["verify"]);
  assert.ok(await webcrypto.subtle.verify("HMAC", key, Buffer.from(s, "base64url"), new TextEncoder().encode(`${h}.${p}`)));
});

test("a vendor error never carries the API key", async () => {
  const v = vendor(`{"error":"invalid key ${KEY}"}`, 401);
  const err = await mintOpenAIRealtimeCredential({ apiKey: KEY, model: "m", fetch: v.fetch }).catch((e) => e);
  assert.equal(err.status, 401);
  assert.doesNotMatch(err.message, /SECRET/);
  assert.match(err.message, /\[redacted\]/);
  const down = await mintGeminiLiveCredential({ apiKey: KEY, model: "m", fetch: async () => { throw new Error(`ECONNRESET ${KEY}`); } }).catch((e) => e);
  assert.equal(down.status, 0);
  assert.doesNotMatch(down.message, /SECRET/);
});

test("the helper refuses to run where there is a DOM", async () => {
  globalThis.document = {};
  try {
    await assert.rejects(mintOpenAIRealtimeCredential({ apiKey: KEY, model: "m", fetch: vendor({}).fetch }), /ran in a browser/);
    await assert.rejects(mintLiveKitCredential({ apiKey: "a", apiSecret: "b", url: "u", room: "r", identity: "i" }), /ran in a browser/);
  } finally {
    delete globalThis.document;
  }
});

test("credentialResponse: JSON, no-store", async () => {
  const r = credentialResponse({ credential: "ek_1", expiresAt: 5 });
  assert.equal(r.headers.get("Cache-Control"), "no-store");
  assert.equal(r.headers.get("Content-Type"), "application/json");
  assert.deepEqual(await r.json(), { credential: "ek_1", expiresAt: 5 });
});

// ---- dev proxy --------------------------------------------------------------

test("parseEnv / loadEnv: comments, export, quotes; the real environment wins", () => {
  assert.deepEqual(parseEnv('# c\nexport A=1\nB="two words"\nC=\'x\'\nD=val # note\n bad line\n'), { A: "1", B: "two words", C: "x", D: "val" });
  const env = loadEnv("/nonexistent-dir", { OPENAI_API_KEY: "from-env" });
  assert.equal(env.OPENAI_API_KEY, "from-env");
});

async function withProxy(opts, fn) {
  const server = await startDevProxy({ port: 0, ...opts });
  try {
    const { address, port } = server.address();
    assert.equal(address, "127.0.0.1", "bound to loopback only");
    await fn(`http://127.0.0.1:${port}`);
  } finally {
    await new Promise((r) => server.close(r));
  }
}

test("dev proxy: canonical shape per vendor, 501 when unset, 404/405, never the key", async () => {
  const v = vendor({ value: "ek_dev", expires_at: 42 });
  const logs = [];
  await withProxy({ env: { OPENAI_API_KEY: KEY }, fetch: v.fetch, log: (l) => logs.push(l) }, async (base) => {
    const ok = await fetch(`${base}/openai`, { method: "POST" });
    assert.equal(ok.status, 200);
    assert.equal(ok.headers.get("cache-control"), "no-store");
    assert.deepEqual(await ok.json(), { credential: "ek_dev", expiresAt: 42 });

    const unset = await fetch(`${base}/gemini`, { method: "POST" });
    assert.equal(unset.status, 501);
    assert.match((await unset.json()).error, /GEMINI_API_KEY/);

    assert.equal((await fetch(`${base}/nope`, { method: "POST" })).status, 404);
    assert.equal((await fetch(`${base}/openai`)).status, 405);
  });
  assert.ok(logs.every((l) => !l.includes(KEY)));
});

test("dev proxy: a vendor failure is a 502 with the key redacted", async () => {
  const v = vendor(`bad key ${KEY}`, 401);
  await withProxy({ env: { OPENAI_API_KEY: KEY }, fetch: v.fetch }, async (base) => {
    const r = await fetch(`${base}/openai`, { method: "POST" });
    assert.equal(r.status, 502);
    const body = await r.json();
    assert.equal(body.vendorStatus, 401);
    assert.doesNotMatch(JSON.stringify(body), /SECRET/);
  });
});

test("dev proxy: CORS for localhost origins only (plus --allow-origin), and the Private Network Access preflight", async () => {
  await withProxy({ env: { OPENAI_API_KEY: KEY }, fetch: vendor({ value: "ek_x" }).fetch, allowOrigins: ["https://studio.example"] }, async (base) => {
    const post = (origin) => fetch(`${base}/openai`, { method: "POST", headers: { Origin: origin } });
    const local = await post("http://localhost:5173");
    assert.equal(local.headers.get("access-control-allow-origin"), "http://localhost:5173");
    const allowed = await post("https://studio.example");
    assert.equal(allowed.status, 200);
    assert.equal(allowed.headers.get("access-control-allow-origin"), "https://studio.example");
    const evil = await post("https://evil.example");
    assert.equal(evil.status, 403, "another site can't mint on your keys");
    assert.equal(evil.headers.get("access-control-allow-origin"), null);

    const pre = await fetch(`${base}/openai`, {
      method: "OPTIONS",
      headers: { Origin: "https://studio.example", "Access-Control-Request-Method": "POST", "Access-Control-Request-Private-Network": "true" },
    });
    assert.equal(pre.status, 204);
    assert.equal(pre.headers.get("access-control-allow-private-network"), "true");
  });
});

test("dev proxy handler: livekit mints locally, no vendor call", async () => {
  const env = { LIVEKIT_API_KEY: "k", LIVEKIT_API_SECRET: "s", LIVEKIT_URL: "wss://lk.example", LIVEKIT_ROOM: "demo" };
  const handler = createHandler({ env, fetch: async () => assert.fail("no vendor call for LiveKit") });
  const res = { writeHead(status, headers) { this.status = status; this.headers = headers; }, end(b) { this.body = b; } };
  await handler({ method: "POST", url: "/livekit", headers: {} }, res);
  assert.equal(res.status, 200);
  const c = JSON.parse(res.body);
  assert.equal(c.url, "wss://lk.example");
  assert.equal(JSON.parse(Buffer.from(c.credential.split(".")[1], "base64url")).video.room, "demo");
});
