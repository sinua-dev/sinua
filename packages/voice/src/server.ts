// @sinua/voice/server -- mint the short-lived credentials the voice sources take,
// in YOUR backend. Server-only: every function refuses to run where there is a
// DOM, so a long-lived key can't be minted from a browser bundle by accident.
//
// No dependencies: `fetch` and WebCrypto only, so the same code runs on Node 18+,
// Next.js route handlers, Cloudflare Workers, Deno and Supabase Edge Functions.
// Each function returns the shared shape `{ credential, expiresAt?, url? }`
// (`SinuaCredential`); answer your endpoint with `credentialResponse(…)`.
//
// Nothing here logs, and an API key never ends up in a thrown message.

import type { SinuaCredential } from "./credential.js";

export type { SinuaCredential } from "./credential.js";

type Fetch = typeof fetch;

interface Common {
  /** The vendor's long-lived secret. Read it from your server's environment. */
  apiKey: string;
  /** For tests, or a proxy: replaces the global `fetch`. */
  fetch?: Fetch;
}

/** A vendor call that failed; `status` is the vendor's HTTP status (0 if unreachable). */
export class CredentialMintError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "CredentialMintError";
  }
}

export interface OpenAIRealtimeMintOptions extends Common {
  /** Realtime model, e.g. `gpt-realtime`. */
  model: string;
  /** Output voice, e.g. `marin`. */
  voice?: string;
  /** System instructions, fixed for the session. */
  instructions?: string;
  /** Lifetime of the `ek_`, 10–7200 s. Default 600. */
  expiresInSeconds?: number;
}

/**
 * OpenAI Realtime: `POST /v1/realtime/client_secrets` → `{ credential: "ek_…", expiresAt }`.
 * The session's model, voice and instructions are fixed here; the client can't change them.
 */
export async function mintOpenAIRealtimeCredential(o: OpenAIRealtimeMintOptions): Promise<SinuaCredential> {
  assertServer("mintOpenAIRealtimeCredential");
  const session: Record<string, unknown> = {
    type: "realtime",
    model: o.model,
    // `server_vad`: the speech_started/stopped events the web source maps to states.
    audio: { input: { turn_detection: { type: "server_vad" } }, ...(o.voice ? { output: { voice: o.voice } } : {}) },
  };
  if (o.instructions) session.instructions = o.instructions;
  const data = await call(o, "OpenAI client_secrets", "https://api.openai.com/v1/realtime/client_secrets", {
    method: "POST",
    headers: { Authorization: `Bearer ${o.apiKey}`, "Content-Type": "application/json" },
    body: JSON.stringify({ expires_after: { anchor: "created_at", seconds: o.expiresInSeconds ?? 600 }, session }),
  });
  const value = str(data.value) ?? str((data.client_secret as Record<string, unknown> | undefined)?.value);
  if (!value) throw new CredentialMintError("OpenAI client_secrets: the response had no `value`", 502);
  const expiresAt = num(data.expires_at);
  return { credential: value, ...(expiresAt !== undefined ? { expiresAt } : {}) };
}

export interface GeminiLiveMintOptions extends Common {
  /** Live model, e.g. `gemini-3.8-live` (with or without `models/`). */
  model: string;
  /** A prebuilt voice name, e.g. `Kore`. */
  voice?: string;
  /** System instructions, locked into the token. */
  instructions?: string;
  /** How long the token can carry messages. Default 1800 s (Google's default). */
  expiresInSeconds?: number;
  /** How long a *new* session can start with it. Default 60 s (Google's default). */
  newSessionWindowSeconds?: number;
  /** Sessions it can start. Default 1. */
  uses?: number;
}

/**
 * Gemini Live: `POST /v1beta/auth_tokens` → `{ credential: "auth_tokens/…", expiresAt }`.
 * The model, voice and instructions are sent as the token's setup and **locked**
 * (`fieldMask`), so the client's own setup can't change them; everything else in
 * the client's setup (session resumption, transcription) still applies.
 */
export async function mintGeminiLiveCredential(o: GeminiLiveMintOptions): Promise<SinuaCredential> {
  assertServer("mintGeminiLiveCredential");
  const now = Date.now();
  const expiresAt = Math.floor(now / 1000) + (o.expiresInSeconds ?? 1800);
  const setup: Record<string, unknown> = { model: o.model.startsWith("models/") ? o.model : `models/${o.model}` };
  const generationConfig: Record<string, unknown> = { responseModalities: ["AUDIO"] };
  const locked = ["model", "generationConfig.responseModalities"];
  if (o.voice) {
    generationConfig.speechConfig = { voiceConfig: { prebuiltVoiceConfig: { voiceName: o.voice } } };
    locked.push("generationConfig.speechConfig");
  }
  setup.generationConfig = generationConfig;
  if (o.instructions) {
    setup.systemInstruction = { parts: [{ text: o.instructions }] };
    locked.push("systemInstruction");
  }
  const data = await call(o, "Gemini auth_tokens", "https://generativelanguage.googleapis.com/v1beta/auth_tokens", {
    method: "POST",
    headers: { "x-goog-api-key": o.apiKey, "Content-Type": "application/json" },
    body: JSON.stringify({
      uses: o.uses ?? 1,
      expireTime: new Date(expiresAt * 1000).toISOString(),
      newSessionExpireTime: new Date(now + (o.newSessionWindowSeconds ?? 60) * 1000).toISOString(),
      bidiGenerateContentSetup: setup,
      fieldMask: locked.join(","),
    }),
  });
  const name = str(data.name);
  if (!name) throw new CredentialMintError("Gemini auth_tokens: the response had no `name`", 502);
  return { credential: name, expiresAt };
}

export interface ElevenLabsSignOptions extends Common {
  /** The private agent's id. */
  agentId: string;
}

/**
 * ElevenLabs Agents: `GET /v1/convai/conversation/get-signed-url` →
 * `{ credential: "wss://…", expiresAt }` (a signed URL is valid 15 minutes).
 * A *public* agent needs none of this: pass its agent id to the source directly.
 */
export async function signElevenLabsUrl(o: ElevenLabsSignOptions): Promise<SinuaCredential> {
  assertServer("signElevenLabsUrl");
  const url = `https://api.elevenlabs.io/v1/convai/conversation/get-signed-url?agent_id=${encodeURIComponent(o.agentId)}`;
  const data = await call(o, "ElevenLabs get-signed-url", url, { headers: { "xi-api-key": o.apiKey } });
  const signed = str(data.signed_url);
  if (!signed) throw new CredentialMintError("ElevenLabs get-signed-url: the response had no `signed_url`", 502);
  return { credential: signed, expiresAt: Math.floor(Date.now() / 1000) + 15 * 60 };
}

export interface LiveKitMintOptions {
  /** LiveKit API key (the JWT issuer). */
  apiKey: string;
  /** LiveKit API secret (signs the JWT). */
  apiSecret: string;
  /** The LiveKit server URL the client connects to, `wss://…`. */
  url: string;
  /** Room to join. */
  room: string;
  /** This participant's identity. */
  identity: string;
  /** Display name. */
  name?: string;
  /** Token lifetime. Default 600 s. */
  ttlSeconds?: number;
}

/**
 * LiveKit: a room-join JWT (HS256, signed with WebCrypto, no SDK) →
 * `{ credential: <jwt>, url, expiresAt }`. Grants join, publish and subscribe
 * for `room` only.
 */
export async function mintLiveKitCredential(o: LiveKitMintOptions): Promise<SinuaCredential> {
  assertServer("mintLiveKitCredential");
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + (o.ttlSeconds ?? 600);
  const claims: Record<string, unknown> = {
    iss: o.apiKey,
    sub: o.identity,
    nbf: now,
    exp: expiresAt,
    video: { roomJoin: true, room: o.room, canPublish: true, canSubscribe: true, canPublishData: true },
  };
  if (o.name) claims.name = o.name;
  return { credential: await signJwtHs256(claims, o.apiSecret), url: o.url, expiresAt };
}

/** The endpoint's answer: the credential as JSON, never cached. */
export function credentialResponse(c: SinuaCredential, init: ResponseInit = {}): Response {
  const headers = new Headers(init.headers);
  headers.set("Content-Type", "application/json");
  headers.set("Cache-Control", "no-store");
  return new Response(JSON.stringify(c), { ...init, headers });
}

function assertServer(fn: string): void {
  if (typeof document !== "undefined") {
    throw new Error(
      `@sinua/voice/server ${fn}() ran in a browser. It needs your vendor's secret key, which must never ship ` +
        "to a client: call it from your backend and pass the voice source a `credentialUrl` instead.",
    );
  }
}

async function call(o: Common, what: string, url: string, init: RequestInit): Promise<Record<string, unknown>> {
  const f = o.fetch ?? fetch;
  let res: Response;
  try {
    res = await f(url, init);
  } catch (err) {
    throw new CredentialMintError(`${what}: request failed (${redact(err instanceof Error ? err.message : String(err), o.apiKey)})`, 0);
  }
  let body = "";
  try {
    body = await res.text();
  } catch {
    /* no body */
  }
  if (!res.ok) throw new CredentialMintError(`${what} returned ${res.status}: ${redact(body.slice(0, 300), o.apiKey)}`, res.status);
  try {
    return JSON.parse(body) as Record<string, unknown>;
  } catch {
    throw new CredentialMintError(`${what}: the response was not JSON`, 502);
  }
}

function redact(text: string, secret: string): string {
  return secret ? text.split(secret).join("[redacted]") : text;
}

function str(v: unknown): string | undefined {
  return typeof v === "string" && v ? v : undefined;
}

function num(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

function base64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

async function signJwtHs256(claims: Record<string, unknown>, secret: string): Promise<string> {
  const enc = new TextEncoder();
  const part = (o: unknown) => base64url(enc.encode(JSON.stringify(o)));
  const input = `${part({ alg: "HS256", typ: "JWT" })}.${part(claims)}`;
  const key = await crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const sig = new Uint8Array(await crypto.subtle.sign("HMAC", key, enc.encode(input)));
  return `${input}.${base64url(sig)}`;
}
