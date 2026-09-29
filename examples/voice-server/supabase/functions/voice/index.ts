// Supabase Edge Function (Deno): POST /functions/v1/voice/<vendor>
// Secrets: `supabase secrets set OPENAI_API_KEY=…`. deno.json maps @sinua/voice.
import { handleCredentialRequest } from "../../../shared/credentials.ts";

Deno.serve(async (req) => {
  const vendor = new URL(req.url).pathname.split("/").pop() ?? "";
  return handleCredentialRequest(req, vendor, Deno.env.toObject(), await currentUserId(req));
});

/**
 * Replace with Supabase auth: `supabase.auth.getUser(jwt)` from the
 * Authorization header. Until then every request is refused, unless you opt in
 * locally with VOICE_ALLOW_ANONYMOUS=1 -- never set that in production.
 */
function currentUserId(_req: Request): Promise<string | null> {
  return Promise.resolve(Deno.env.get("VOICE_ALLOW_ANONYMOUS") === "1" ? "anonymous" : null);
}
