import { FatalConnectError, isRetryableHttpStatus } from "./realtimeReconnect.js";

/**
 * The one credential contract every vendor source takes, on every platform
 * (docs/audio-pipeline.md, *Credentials*).
 *
 * Your backend mints a short-lived credential and answers with this JSON --
 * the same shape for OpenAI (`ek_…`), Gemini (`auth_tokens/…`), ElevenLabs (a
 * signed `wss://` URL) and LiveKit (a room JWT plus the server `url`).
 * `@sinua/voice/server` mints it for you; `npx @sinua/voice dev-proxy` serves
 * it locally so you can try voice before you write a backend.
 */
export interface SinuaCredential {
  /** The short-lived credential itself. */
  credential: string;
  /** When it stops working, in Unix seconds, if the vendor says. */
  expiresAt?: number;
  /** LiveKit only: the server URL (`wss://…`) the token is for. */
  url?: string;
}

/**
 * Returns a fresh credential. Called on **every** connect and reconnect, so an
 * expired or spent one is never reused. A plain string counts as
 * `{ credential }`.
 */
export type CredentialProvider = () => Promise<SinuaCredential | string>;

export interface CredentialOptions {
  /**
   * A provider (the production shape), or one fixed value: a credential you
   * pasted for a single session, or ElevenLabs' public agent id.
   */
  credential?: string | CredentialProvider;
  /**
   * Shorthand for a provider that `POST`s to your endpoint (no body,
   * `cache: "no-store"`, cookies as for any same-origin fetch) and reads a
   * `SinuaCredential` back. Takes precedence over `credential`.
   */
  credentialUrl?: string;
}

/** True when the source can get a *new* credential on a reconnect. */
export function canRefreshCredential(opts: CredentialOptions): boolean {
  return !!opts.credentialUrl || typeof opts.credential === "function";
}

/**
 * The credential for this (re)connect. Throws `FatalConnectError` for anything
 * a retry can't fix (a bad shape, a 4xx from `credentialUrl`); network errors,
 * timeouts, 429 and 5xx stay retryable. The credential never appears in an
 * error message.
 */
export async function resolveCredential(
  vendor: string,
  opts: CredentialOptions,
  { needsUrl = false }: { needsUrl?: boolean } = {},
): Promise<SinuaCredential> {
  let raw: unknown;
  if (opts.credentialUrl) raw = await fetchCredential(vendor, opts.credentialUrl);
  else if (typeof opts.credential === "function") raw = await opts.credential();
  else raw = opts.credential;
  const c = parseCredential(vendor, raw);
  if (needsUrl && !c.url) throw new FatalConnectError(`${vendor}: the credential has no \`url\` (LiveKit needs { credential, url })`);
  return c;
}

/** Validates a provider's or an endpoint's answer against `SinuaCredential`. */
export function parseCredential(vendor: string, raw: unknown): SinuaCredential {
  if (typeof raw === "string") {
    const credential = raw.trim();
    if (!credential) throw new FatalConnectError(`${vendor}: a credential is required`);
    return { credential };
  }
  if (!raw || typeof raw !== "object") throw new FatalConnectError(`${vendor}: a credential is required`);
  const o = raw as Record<string, unknown>;
  if (typeof o.credential !== "string" || !o.credential.trim()) {
    throw new FatalConnectError(`${vendor}: expected { credential: string, expiresAt?, url? }, got keys [${Object.keys(o).join(", ")}]`);
  }
  if (o.expiresAt !== undefined && typeof o.expiresAt !== "number") {
    throw new FatalConnectError(`${vendor}: \`expiresAt\` must be Unix seconds (a number)`);
  }
  if (o.url !== undefined && typeof o.url !== "string") throw new FatalConnectError(`${vendor}: \`url\` must be a string`);
  return {
    credential: o.credential.trim(),
    ...(o.expiresAt !== undefined ? { expiresAt: o.expiresAt as number } : {}),
    ...(o.url !== undefined ? { url: o.url as string } : {}),
  };
}

async function fetchCredential(vendor: string, url: string): Promise<unknown> {
  const res = await fetch(url, { method: "POST", cache: "no-store", headers: { Accept: "application/json" } });
  if (!res.ok) {
    const message = `${vendor}: ${url} returned ${res.status}`;
    throw isRetryableHttpStatus(res.status) ? new Error(message) : new FatalConnectError(message);
  }
  try {
    return await res.json();
  } catch {
    throw new FatalConnectError(`${vendor}: ${url} did not return JSON`);
  }
}
