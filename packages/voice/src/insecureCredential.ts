/**
 * The one rule about long-lived API keys, shared by every adapter that can be
 * handed one.
 *
 * A production credential is short-lived and minted server-side: OpenAI's
 * `ek_…` (`POST /v1/realtime/client_secrets`) or Gemini's `auth_tokens/…`
 * (`POST /v1beta/auth_tokens`). A raw account key is a different object --
 * long-lived, unscoped, and billable -- and on the Web it reaches the browser,
 * where it can be read out of the network panel, out of memory, or (Gemini,
 * whose browser path has no header to put it in) out of the socket URL itself.
 *
 * So a raw key is **refused** unless the caller opts in with
 * `allowInsecureApiKey`. Prior art for the shape: `openai-agents-js` refuses a
 * raw key in a browser unless `useInsecureApiKey` is set
 * (`packages/agents-realtime/src/openaiRealtimeWebRtc.ts`), which is the same
 * source this project's OpenAI adapter was built from.
 *
 * Two placement rules this module exists to keep consistent:
 *
 * - The check runs inside `connect()`, **never a constructor**. The Studio
 *   builds a source *outside* its try block, so a throwing constructor takes
 *   the panel down instead of showing the error inline.
 * - It runs before the microphone, the audio graph and the socket, so a
 *   refused credential never opens a device or a connection.
 */

export interface InsecureCredentialGuard {
  /** The class doing the connecting; it leads the message. */
  vendor: string;
  /** True when the credential already is the vendor's short-lived shape. */
  isEphemeral: boolean;
  /** The caller's opt-in, as given. */
  allowInsecureApiKey: boolean | undefined;
  /** What a short-lived credential looks like, e.g. `auth_tokens/…`. */
  ephemeralShape: string;
  /** How a backend mints one, e.g. `POST /v1beta/auth_tokens`. */
  mintHint: string;
}

/**
 * Returns the refusal message for a raw key used without the opt-in, or `null`
 * when the connect may go ahead. The caller throws it in whatever error type
 * its own reconnect logic treats as fatal -- a refused credential must never be
 * retried.
 *
 * When a raw key *is* allowed, this warns once per call, so a local demo can't
 * quietly turn into a deployment.
 */
export function insecureCredentialRefusal(g: InsecureCredentialGuard): string | null {
  if (g.isEphemeral) return null;
  if (!g.allowInsecureApiKey) {
    return (
      `${g.vendor}: refusing a raw, long-lived API key. Pass a short-lived credential ` +
      `(${g.ephemeralShape}) minted by your own backend (${g.mintHint}). ` +
      "For a local demo only, set `allowInsecureApiKey: true`."
    );
  }
  console.warn(
    `${g.vendor}: connecting with a raw, long-lived API key because \`allowInsecureApiKey\` ` +
      "is set. That key is exposed in the browser -- this is for local demos, not for " +
      `shipping. In a product, send a ${g.ephemeralShape} minted by your backend (${g.mintHint}).`,
  );
  return null;
}
