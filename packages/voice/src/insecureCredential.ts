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
