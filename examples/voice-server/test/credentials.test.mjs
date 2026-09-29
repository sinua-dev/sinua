// The templates' shared handler against mocked vendor HTTP: auth first, the
// canonical shape out, vendor details never echoed to the browser.
import { test } from "node:test";
import assert from "node:assert/strict";
import { handleCredentialRequest } from "../shared/credentials.ts";
import worker from "../cloudflare-worker/src/worker.ts";

const env = {
  OPENAI_API_KEY: "sk-SECRET",
  GEMINI_API_KEY: "g-SECRET",
  ELEVENLABS_API_KEY: "x-SECRET",
  ELEVENLABS_AGENT_ID: "agent",
  LIVEKIT_API_KEY: "k",
  LIVEKIT_API_SECRET: "s",
  LIVEKIT_URL: "wss://lk.example",
  VOICE_INSTRUCTIONS: "Be brief.",
};
const post = () => new Request("https://app.example/api/voice/x", { method: "POST" });
const vendorReplies = {
  "api.openai.com": { value: "ek_1", expires_at: 100 },
  "generativelanguage.googleapis.com": { name: "auth_tokens/1" },
  "api.elevenlabs.io": { signed_url: "wss://api.elevenlabs.io/v1/convai/conversation?x=1" },
};
const sent = [];
const fakeFetch = async (url, init) => {
  const host = new URL(String(url)).host;
  sent.push({ host, body: init?.body ? JSON.parse(String(init.body)) : undefined });
  return Response.json(vendorReplies[host]);
};

test("every vendor answers { credential, expiresAt?, url? } with no-store", async () => {
  for (const [vendor, check] of [
    ["openai", (c) => assert.deepEqual(c, { credential: "ek_1", expiresAt: 100 })],
    ["gemini", (c) => assert.equal(c.credential, "auth_tokens/1")],
    ["elevenlabs", (c) => assert.match(c.credential, /^wss:\/\//)],
    ["livekit", (c) => assert.equal(c.url, "wss://lk.example")],
  ]) {
    const res = await handleCredentialRequest(post(), vendor, env, "user-1", fakeFetch);
    assert.equal(res.status, 200, vendor);
    assert.equal(res.headers.get("cache-control"), "no-store");
    check(await res.json());
  }
  const openai = sent.find((s) => s.host === "api.openai.com").body;
  assert.equal(openai.session.instructions, "Be brief.", "instructions come from the server's env");
});

test("no user → 401 before any vendor call; bad vendor → 404; GET → 405", async () => {
  const before = sent.length;
  assert.equal((await handleCredentialRequest(post(), "openai", env, null, fakeFetch)).status, 401);
  assert.equal(sent.length, before);
  assert.equal((await handleCredentialRequest(post(), "nope", env, "u", fakeFetch)).status, 404);
  assert.equal((await handleCredentialRequest(new Request("https://a.example/"), "openai", env, "u", fakeFetch)).status, 405);
});

test("a vendor failure is a generic 502: no vendor text, no key", async () => {
  const quiet = console.error;
  console.error = () => {};
  try {
    const failing = async () => new Response("invalid key sk-SECRET", { status: 401 });
    const res = await handleCredentialRequest(post(), "openai", env, "u", failing);
    assert.equal(res.status, 502);
    assert.doesNotMatch(await res.text(), /SECRET|invalid/);
  } finally {
    console.error = quiet;
  }
});

test("worker template: routes /api/voice/<vendor> and refuses anonymous callers by default", async () => {
  const res = await worker.fetch(new Request("https://w.example/api/voice/livekit", { method: "POST" }), env);
  assert.equal(res.status, 401);
  const open = await worker.fetch(new Request("https://w.example/api/voice/livekit", { method: "POST" }), { ...env, VOICE_ALLOW_ANONYMOUS: "1" });
  assert.equal(open.status, 200);
  assert.equal((await worker.fetch(new Request("https://w.example/other"), env)).status, 404);
});
