// Cloudflare Worker: POST /api/voice/<vendor>
// Secrets: `wrangler secret put OPENAI_API_KEY` (and the others you use).
import { handleCredentialRequest, type Env } from "../../shared/credentials.ts";

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const match = /^\/api\/voice\/([a-z]+)$/.exec(new URL(req.url).pathname);
    if (!match) return new Response("not found", { status: 404 });
    return handleCredentialRequest(req, match[1], env, await currentUserId(req, env));
  },
};

/**
 * Replace with your auth (Cloudflare Access JWT, a session cookie …). Until then
 * every request is refused, unless you opt in locally with
 * VOICE_ALLOW_ANONYMOUS=1 (.dev.vars) -- never set that in production.
 */
function currentUserId(_req: Request, env: Env): Promise<string | null> {
  return Promise.resolve(env.VOICE_ALLOW_ANONYMOUS === "1" ? "anonymous" : null);
}
