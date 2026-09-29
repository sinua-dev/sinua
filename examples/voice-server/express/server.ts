// Express: POST /api/voice/:vendor
// Client: new GeminiLiveVoiceSource({ credentialUrl: "/api/voice/gemini" })
import express from "express";
import { mintCredential, VENDORS, type Vendor } from "../shared/credentials.ts";

const app = express();

app.post("/api/voice/:vendor", async (req, res) => {
  const user = currentUserId(req);
  if (!user) return void res.status(401).json({ error: "sign in first" });
  const vendor = req.params.vendor;
  if (!(VENDORS as readonly string[]).includes(vendor)) return void res.status(404).json({ error: `unknown vendor ${vendor}` });
  try {
    res.set("Cache-Control", "no-store").json(await mintCredential(vendor as Vendor, process.env, user));
  } catch (err) {
    console.error("voice credential:", err instanceof Error ? err.message : err);
    res.status(502).json({ error: "could not mint a voice credential" });
  }
});

/**
 * Replace with your auth (passport's `req.user`, a session …). Until then every
 * request is refused, unless you opt in locally with VOICE_ALLOW_ANONYMOUS=1 --
 * never set that in production.
 */
function currentUserId(_req: express.Request): string | null {
  return process.env.VOICE_ALLOW_ANONYMOUS === "1" ? "anonymous" : null;
}

app.listen(Number(process.env.PORT ?? 3000));
