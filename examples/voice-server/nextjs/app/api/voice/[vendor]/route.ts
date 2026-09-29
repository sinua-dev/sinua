// Next.js (App Router): app/api/voice/[vendor]/route.ts
// Client: new OpenAIRealtimeVoiceSource({ credentialUrl: "/api/voice/openai" })
import { handleCredentialRequest } from "../../../../../shared/credentials.ts";

export const runtime = "nodejs"; // "edge" works too: the helper is fetch + WebCrypto only
export const dynamic = "force-dynamic";

export async function POST(req: Request, ctx: { params: Promise<{ vendor: string }> }): Promise<Response> {
  const { vendor } = await ctx.params;
  const user = await currentUserId(req);
  return handleCredentialRequest(req, vendor, process.env, user);
}

/**
 * Replace with your auth (NextAuth `auth()`, Clerk `auth()`, a session cookie …).
 * Until then every request is refused, unless you opt in locally with
 * VOICE_ALLOW_ANONYMOUS=1 -- never set that in production.
 */
function currentUserId(_req: Request): Promise<string | null> {
  return Promise.resolve(process.env.VOICE_ALLOW_ANONYMOUS === "1" ? "anonymous" : null);
}
