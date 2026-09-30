/**
 * The one rule about long-lived API keys, shared by every adapter that could
 * be handed one: **a raw key is always refused.**
 *
 * A production credential is short-lived and minted server-side: OpenAI's
 * `ek_…` (`POST /v1/realtime/client_secrets`) or Gemini's `auth_tokens/…`
 * (`POST /v1beta/auth_tokens`). A raw account key is a different object --
 * long-lived, unscoped, and billable -- and on the Web it reaches the browser,
 * where it can be read out of the network panel, out of memory, or (Gemini,
 * whose browser path has no header to put it in) out of the socket URL itself.
 *
 * There used to be an `allowInsecureApiKey` opt-in for local demos. It is gone:
 * `npx @sinua/voice dev-proxy` mints real short-lived credentials on
 * localhost, and `@sinua/voice/server` does it in a backend, so no path needs
 * the key in the client.
 *
 * Placement: the check runs inside `connect()` (never a constructor -- the
 * Studio builds a source outside its try block), on every credential a
 * provider returns, before the microphone, the audio graph and the socket.
 */

export interface InsecureCredentialGuard {
  /** The class doing the connecting; it leads the message. */
  vendor: string;
  /** True when the credential already is the vendor's short-lived shape. */
  isEphemeral: boolean;
  /** What a short-lived credential looks like, e.g. `auth_tokens/…`. */
  ephemeralShape: string;
}

/** The refusal message for anything that isn't the short-lived shape, or `null`. */
export function insecureCredentialRefusal(g: InsecureCredentialGuard): string | null {
  if (g.isEphemeral) return null;
  return (
    `${g.vendor}: expected a short-lived credential (${g.ephemeralShape}); refusing what looks like a raw, ` +
    "long-lived API key. Mint one in your backend with @sinua/voice/server, or run " +
    "`npx @sinua/voice dev-proxy` and pass `credentialUrl`."
  );
}

/** OpenAI's own API host: a credential sent here must be an `ek_`. */
export const OPENAI_API_HOST = "api.openai.com";

/** True when `url` is on OpenAI's own API host; an unparseable URL counts as OpenAI (the strict rule). */
export function isOpenAIHost(url: string): boolean {
  try {
    return new URL(url).hostname === OPENAI_API_HOST;
  } catch {
    return true;
  }
}

/**
 * OpenAI Realtime's credential rule, by where the credential goes:
 *
 * - to OpenAI (`api.openai.com`): it must be an `ek_…` minted by your backend;
 * - to your own calls endpoint (a `callsUrl` on another host, which opens the
 *   OpenAI session with its own key -- the "sideband" setup): it's your own
 *   short-lived token, any shape, but never a raw OpenAI key (`sk-…`).
 *
 * The same rule, and the same message, on iOS and Android (`InsecureCredential`).
 */
export function openAICredentialRefusal(credential: string, callsUrl: string, vendor = "OpenAIRealtimeVoiceSource"): string | null {
  const c = credential.trim();
  if (isOpenAIHost(callsUrl)) return insecureCredentialRefusal({ vendor, isEphemeral: c.startsWith("ek_"), ephemeralShape: "ek_…" });
  if (c.startsWith("sk-")) {
    return `${vendor}: your own calls endpoint takes your own short-lived token, never a raw OpenAI key (sk-…); keep the key in your backend.`;
  }
  return null;
}

/**
 * OpenAI GPT-Live's rule: its session is only ever opened by your server
 * (`POST /v1/live/sessions` with your project key; there is no `ek_`), so the
 * session URL must be your own endpoint, never `api.openai.com`, and a
 * credential for it (optional: your endpoint may use cookies) is your own
 * token, never a raw OpenAI key (`sk-…`). Same rule and messages on iOS and Android.
 */
export function openAILiveRefusal(sessionUrl: string, credential: string | null, vendor = "OpenAILiveVoiceSource"): string | null {
  if (isOpenAIHost(sessionUrl)) {
    return `${vendor}: sessionUrl must be your own endpoint; GPT-Live sessions are opened by your server with its key (POST /v1/live/sessions), never from the app.`;
  }
  if (credential?.trim().startsWith("sk-")) {
    return `${vendor}: your session endpoint takes your own short-lived token, never a raw OpenAI key (sk-…); keep the key in your backend.`;
  }
  return null;
}
