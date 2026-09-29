// The one piece every template shares: pick the vendor, mint with the keys
// from YOUR server's environment, answer with `{ credential, expiresAt?, url? }`.
// Copy this file next to the template you use.
import {
  CredentialMintError,
  credentialResponse,
  mintGeminiLiveCredential,
  mintLiveKitCredential,
  mintOpenAIRealtimeCredential,
  signElevenLabsUrl,
  type SinuaCredential,
} from "@sinua/voice/server";

export type Env = Record<string, string | undefined>;
export const VENDORS = ["openai", "gemini", "elevenlabs", "livekit"] as const;
export type Vendor = (typeof VENDORS)[number];

/**
 * Mints a credential for `vendor`. The model, voice and instructions are
 * decided HERE, on the server; the client can't change them.
 *
 * `user` is whoever your auth says is calling (used as the LiveKit identity).
 * `fetch` is only for tests.
 */
export async function mintCredential(vendor: Vendor, env: Env, user: string, fetch?: typeof globalThis.fetch): Promise<SinuaCredential> {
  const instructions = env.VOICE_INSTRUCTIONS;
  switch (vendor) {
    case "openai":
      return mintOpenAIRealtimeCredential({ apiKey: need(env, "OPENAI_API_KEY"), model: "gpt-realtime", voice: "marin", instructions, fetch });
    case "gemini":
      return mintGeminiLiveCredential({ apiKey: need(env, "GEMINI_API_KEY"), model: "gemini-3.8-live", instructions, fetch });
    case "elevenlabs":
      return signElevenLabsUrl({ apiKey: need(env, "ELEVENLABS_API_KEY"), agentId: need(env, "ELEVENLABS_AGENT_ID"), fetch });
    case "livekit":
      return mintLiveKitCredential({
        apiKey: need(env, "LIVEKIT_API_KEY"),
        apiSecret: need(env, "LIVEKIT_API_SECRET"),
        url: need(env, "LIVEKIT_URL"),
        room: `voice-${user}`,
        identity: user,
      });
  }
}

/**
 * A whole fetch-style endpoint: `POST /…/<vendor>` → the credential as JSON.
 * `user` is `null` when the caller isn't signed in -- then it answers 401, so
 * strangers can't spend your quota.
 */
export async function handleCredentialRequest(req: Request, vendor: string, env: Env, user: string | null, fetch?: typeof globalThis.fetch): Promise<Response> {
  if (req.method !== "POST") return Response.json({ error: "use POST" }, { status: 405 });
  if (!user) return Response.json({ error: "sign in first" }, { status: 401 });
  if (!(VENDORS as readonly string[]).includes(vendor)) return Response.json({ error: `unknown vendor ${vendor}` }, { status: 404 });
  try {
    return credentialResponse(await mintCredential(vendor as Vendor, env, user, fetch));
  } catch (err) {
    // Never echo vendor details to the browser; log them server-side. The
    // helper has already removed the API key from the message.
    console.error("voice credential:", err instanceof Error ? err.message : err);
    const status = err instanceof CredentialMintError ? 502 : 500;
    return Response.json({ error: "could not mint a voice credential" }, { status });
  }
}

function need(env: Env, name: string): string {
  const v = env[name];
  if (!v) throw new Error(`${name} is not set on the server`);
  return v;
}
