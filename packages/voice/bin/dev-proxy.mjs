// `npx @sinua/voice dev-proxy`: a local stand-in for your backend's credential
// endpoint, so you can try voice before you write one. It reads vendor keys from
// .env / .env.local, listens on 127.0.0.1 only, and answers
//   POST /openai  /gemini  /elevenlabs  /livekit
// with the shared shape `{ credential, expiresAt?, url? }`, minted by
// @sinua/voice/server. Point a voice source's `credentialUrl` at it.
//
// It is a development tool: the keys stay on your machine, the browser only
// ever gets short-lived credentials, and it never prints a key.

import { createServer } from "node:http";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  CredentialMintError,
  mintGeminiLiveCredential,
  mintLiveKitCredential,
  mintOpenAIRealtimeCredential,
  signElevenLabsUrl,
} from "../dist/server.js";

export const DEFAULT_PORT = 8787;

/** What each route needs from the environment. */
export const VENDORS = {
  openai: ["OPENAI_API_KEY"],
  gemini: ["GEMINI_API_KEY"],
  elevenlabs: ["ELEVENLABS_API_KEY", "ELEVENLABS_AGENT_ID"],
  livekit: ["LIVEKIT_API_KEY", "LIVEKIT_API_SECRET", "LIVEKIT_URL"],
};

/** `KEY=value` lines; `#` comments, optional `export `, optional quotes. No expansion. */
export function parseEnv(text) {
  const out = {};
  for (const line of text.split(/\r?\n/)) {
    const m = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/.exec(line);
    if (!m || line.trimStart().startsWith("#")) continue;
    let v = m[2];
    if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) v = v.slice(1, -1);
    else v = v.replace(/\s+#.*$/, "");
    out[m[1]] = v;
  }
  return out;
}

/** .env then .env.local from `dir`, under the real environment (which wins). */
export function loadEnv(dir, processEnv = process.env) {
  const env = {};
  for (const name of [".env", ".env.local"]) {
    const p = join(dir, name);
    if (existsSync(p)) Object.assign(env, parseEnv(readFileSync(p, "utf8")));
  }
  for (const [k, v] of Object.entries(processEnv)) if (v !== undefined) env[k] = v;
  return env;
}

export function configuredVendors(env) {
  return Object.keys(VENDORS).filter((v) => VENDORS[v].every((k) => env[k]));
}

/** Mints for one vendor from `env`. Optional per-vendor model/voice and one shared instructions. */
export async function mint(vendor, env, fetchImpl) {
  const f = fetchImpl ? { fetch: fetchImpl } : {};
  const instructions = env.SINUA_INSTRUCTIONS || undefined;
  switch (vendor) {
    case "openai":
      return mintOpenAIRealtimeCredential({
        apiKey: env.OPENAI_API_KEY,
        model: env.SINUA_MODEL_OPENAI || "gpt-realtime",
        voice: env.SINUA_VOICE_OPENAI || "marin",
        instructions,
        ...f,
      });
    case "gemini":
      return mintGeminiLiveCredential({
        apiKey: env.GEMINI_API_KEY,
        model: env.SINUA_MODEL_GEMINI || "gemini-3.8-live",
        voice: env.SINUA_VOICE_GEMINI || undefined,
        instructions,
        ...f,
      });
    case "elevenlabs":
      return signElevenLabsUrl({ apiKey: env.ELEVENLABS_API_KEY, agentId: env.ELEVENLABS_AGENT_ID, ...f });
    case "livekit":
      return mintLiveKitCredential({
        apiKey: env.LIVEKIT_API_KEY,
        apiSecret: env.LIVEKIT_API_SECRET,
        url: env.LIVEKIT_URL,
        room: env.LIVEKIT_ROOM || "sinua-dev",
        identity: `sinua-dev-${Math.random().toString(36).slice(2, 8)}`,
      });
    default:
      throw new Error(`unknown vendor ${vendor}`);
  }
}

const LOCAL_ORIGIN = /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(:\d+)?$/;

/**
 * The request handler, separate from the socket so tests can drive it.
 * `allowOrigins`: extra exact origins (e.g. a hosted Studio) besides localhost.
 */
export function createHandler({ env, allowOrigins = [], fetch: fetchImpl, log = () => {} }) {
  const allowed = (origin) => !origin || LOCAL_ORIGIN.test(origin) || allowOrigins.includes(origin);
  return async (req, res) => {
    const origin = req.headers.origin;
    const send = (status, body, extra = {}) => {
      const headers = { "Cache-Control": "no-store", Vary: "Origin", ...extra };
      if (origin && allowed(origin)) headers["Access-Control-Allow-Origin"] = origin;
      if (body !== undefined) headers["Content-Type"] = "application/json";
      res.writeHead(status, headers);
      res.end(body === undefined ? undefined : JSON.stringify(body));
    };
    if (!allowed(origin)) return send(403, { error: `origin ${origin} not allowed (start with --allow-origin ${origin})` });

    const vendor = (req.url || "/").split("?")[0].replace(/^\/+|\/+$/g, "");
    if (req.method === "OPTIONS") {
      // CORS preflight, plus Chrome's Private Network Access one (a public
      // page calling localhost).
      return send(204, undefined, {
        "Access-Control-Allow-Methods": "POST, OPTIONS",
        "Access-Control-Allow-Headers": req.headers["access-control-request-headers"] || "Content-Type, Accept",
        ...(req.headers["access-control-request-private-network"] === "true"
          ? { "Access-Control-Allow-Private-Network": "true" }
          : {}),
        "Access-Control-Max-Age": "600",
      });
    }
    if (!(vendor in VENDORS)) return send(404, { error: `use POST /${Object.keys(VENDORS).join(", /")}` });
    if (req.method !== "POST") return send(405, { error: "use POST" }, { Allow: "POST, OPTIONS" });
    const missing = VENDORS[vendor].filter((k) => !env[k]);
    if (missing.length) return send(501, { error: `${vendor} is not configured: set ${missing.join(", ")} in .env` });
    try {
      const credential = await mint(vendor, env, fetchImpl);
      log(`${vendor}: minted`);
      return send(200, credential);
    } catch (err) {
      // The vendor refused or was unreachable: a gateway error, with the
      // vendor's own (key-redacted) message.
      const message = err instanceof Error ? err.message : String(err);
      log(`${vendor}: ${message}`);
      return send(502, { error: message, ...(err instanceof CredentialMintError ? { vendorStatus: err.status } : {}) });
    }
  };
}

/** Starts the proxy on 127.0.0.1. Resolves with the node server once listening. */
export function startDevProxy({ env, port = DEFAULT_PORT, allowOrigins, fetch: fetchImpl, log } = {}) {
  const server = createServer(createHandler({ env, allowOrigins, fetch: fetchImpl, log }));
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}
