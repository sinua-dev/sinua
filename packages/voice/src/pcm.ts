/**
 * Raw-PCM helpers for the WebSocket-transport voice adapters
 * (`GeminiLiveVoiceSource` / Gemini Live today). Kept DOM-free and side-
 * effect-free on purpose: this is the one part of a PCM adapter that can be
 * proven correct outside a browser (a node round-trip), so it lives apart from the
 * WebSocket/Web Audio wiring that can't.
 *
 * Conventions match Gemini Live's wire format (ai.google.dev/api/live):
 * 16-bit signed little-endian PCM, mono, base64-encoded inside JSON. The
 * Float32 <-> Int16 scaling is the same one Google's own browser sample
 * uses (`live-api-web-console`'s worklet: `float * 32768`), with one
 * addition -- clamping to [-1, 1] before the multiply, which that sample
 * omits and which otherwise wraps a hot mic sample from +1.01 into a loud
 * negative click.
 */

const B64_CHUNK = 0x8000; // String.fromCharCode.apply's safe argument-count ceiling

export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function bytesToBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += B64_CHUNK) {
    bin += String.fromCharCode.apply(null, bytes.subarray(i, i + B64_CHUNK) as unknown as number[]);
  }
  return btoa(bin);
}

/** PCM16 LE bytes -> samples in [-1, 1). A trailing odd byte is ignored. */
export function pcm16ToFloat32(bytes: Uint8Array): Float32Array {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const n = Math.floor(bytes.byteLength / 2);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) out[i] = view.getInt16(i * 2, true) / 32768;
  return out;
}

/**
 * Samples in [-1, 1] -> PCM16 LE bytes. Out-of-range input is clamped, not
 * wrapped. Symmetric `* 32768` (the exact inverse of `pcm16ToFloat32`'s
 * `/ 32768`), with only the single unrepresentable value +1.0 clamped to
 * 32767 -- so a round trip is exact to within rounding everywhere else.
 */
export function float32ToPcm16(samples: Float32Array): Uint8Array {
  const out = new Uint8Array(samples.length * 2);
  const view = new DataView(out.buffer);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    view.setInt16(i * 2, Math.max(-32768, Math.min(32767, Math.round(s * 32768))), true);
  }
  return out;
}

/**
 * `"audio/pcm;rate=24000"` -> 24000. The Live API guide documents 24 kHz
 * output, but one official sample labels the same stream `rate=16000` --
 * so the rate is always read from the actual chunk's mimeType, and
 * `fallback` is only for a mimeType that carries none.
 */
export function parsePcmRate(mimeType: string | undefined, fallback = 24000): number {
  const m = /rate=(\d+)/i.exec(mimeType ?? "");
  return m ? Number(m[1]) : fallback;
}

export interface AudioFormat {
  codec: "pcm" | "ulaw";
  rate: number;
}

/**
 * ElevenLabs-style format strings -- `"pcm_16000"`, `"pcm_44100"`,
 * `"ulaw_8000"` (the values `conversation_initiation_metadata` carries in
 * `agent_output_audio_format` / `user_input_audio_format`). Anything
 * unparseable returns `fallback`.
 */
export function parseAudioFormat(
  format: string | undefined,
  fallback: AudioFormat = { codec: "pcm", rate: 16000 }
): AudioFormat {
  const m = /^(pcm|ulaw)_(\d+)$/i.exec((format ?? "").trim());
  if (!m) return fallback;
  return { codec: m[1].toLowerCase() as AudioFormat["codec"], rate: Number(m[2]) };
}

/**
 * G.711 μ-law decode (for `ulaw_8000` agents -- telephony-configured
 * ElevenLabs agents emit this). Taken from the reference implementation in
 * `rochars/alawmulaw` (`lib/mulaw.js`, MIT): complement the byte, split
 * sign / 3-bit exponent / 4-bit mantissa, reconstruct via the per-exponent
 * base table, negate on sign. Full-scale is 32124, not 32767, by design of
 * the codec.
 */
const ULAW_DECODE_TABLE = [0, 132, 396, 924, 1980, 4092, 8316, 16764];

export function ulawToFloat32(bytes: Uint8Array): Float32Array {
  const out = new Float32Array(bytes.length);
  for (let i = 0; i < bytes.length; i++) {
    const u = ~bytes[i] & 0xff;
    const sign = u & 0x80;
    const exponent = (u >> 4) & 0x07;
    const mantissa = u & 0x0f;
    let sample = ULAW_DECODE_TABLE[exponent] + (mantissa << (exponent + 3));
    if (sign) sample = -sample;
    out[i] = sample / 32768;
  }
  return out;
}
