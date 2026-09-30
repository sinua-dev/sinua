// On your server only: the OpenAI key stays here.
import { createOpenAILiveSession, openAILiveResponse } from "@sinua/voice/server";

// POST /api/voice/openai-live -- the body is the source's `{ sdp }` offer.
export async function POST(req: Request): Promise<Response> {
  if (!(await currentUser(req))) return Response.json({ error: "sign in first" }, { status: 401 });
  const { sdp } = (await req.json()) as { sdp: string };
  const session = await createOpenAILiveSession({
    apiKey: process.env.OPENAI_API_KEY!,
    sdp,
    session: { instructions: "You are a concise voice assistant." }, // model: gpt-live-1
  });
  // Keep session.sessionId if your server joins the call for tools and transcripts.
  return openAILiveResponse(session); // OpenAI's 201 JSON, Cache-Control: no-store
}

declare function currentUser(req: Request): Promise<string | null>; // your auth
declare const process: { env: Record<string, string | undefined> };
