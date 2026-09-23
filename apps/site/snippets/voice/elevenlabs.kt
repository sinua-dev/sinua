package snippets.voice

import dev.sinua.elevenlabs.ElevenLabsVoiceSource

// A public agent's agent_id, or a signed wss:// URL from your backend for a private one.
// The agent's user input format must be PCM (e.g. pcm_16000).
fun elevenLabsVoice(agentIdOrSignedUrl: String) = ElevenLabsVoiceSource(credential = agentIdOrSignedUrl)
