# Changelog: AI Lyrics plugin

## 0.0.7 — 2026-11-18

* First release, with Jellyfesh v0.0.7 (plugin API 1.2, a Jellyfesh Pro
  capability): word-timed lyrics made with Whisper on the device (Tiny,
  Base, Small or Large Turbo) or online with your own API key (OpenAI,
  Groq, or a Custom server that speaks OpenAI's speech-to-text API), after
  you allow sending the song's audio. Plain lyrics are timed to the
  recognised words; without them, lines are made from what was recognised.
  Enhanced LRC with a time for every word. Settings in English, Chinese
  (Simplified and Traditional) and Japanese. The online providers were not
  contacted from tests, only a mock server built from their documentation.
