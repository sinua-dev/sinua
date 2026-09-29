// On your server only: the vendor key stays here. Any runtime with fetch and WebCrypto
// (Node, Next.js, Cloudflare Workers, Deno, Supabase Edge Functions).
import { credentialResponse, mintOpenAIRealtimeCredential } from "@sinua/voice/server";

// POST /api/voice/openai
export async function POST(req: Request): Promise<Response> {
  if (!(await currentUser(req))) return Response.json({ error: "sign in first" }, { status: 401 });
  const credential = await mintOpenAIRealtimeCredential({
    apiKey: process.env.OPENAI_API_KEY!,
    model: "gpt-realtime",
    voice: "marin",
    instructions: "You are a concise voice assistant.",
  });
  return credentialResponse(credential); // { credential, expiresAt }, Cache-Control: no-store
}

declare function currentUser(req: Request): Promise<string | null>; // your auth
declare const process: { env: Record<string, string | undefined> };
