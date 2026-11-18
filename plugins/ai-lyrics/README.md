# AI Lyrics for Jellyfesh

Makes word-timed lyrics with Whisper: for a song without lyrics it
recognises the singing and times every word, and for a song with only plain
(unsynced) lyrics it times that text to the audio. The plugin
(`org.jellyfesh.ai-lyrics`) ships inside every Jellyfesh app and is also
published as a separate `.jfpkg`, signed with the Jellyfesh release key. It
is part of **Jellyfesh Pro** (ADR-0019, ADR-0021). For people using it:
`docs/plugins/ai-lyrics.md`.

## What it does

* **On This Device** (the default): the app recognises the song with the
  Whisper model chosen in the settings (Tiny, Base, Small or Large Turbo),
  downloaded once when asked for. Nothing leaves the device.
* **Online**: the song's audio goes to OpenAI, Groq or a Custom server that
  speaks OpenAI's speech-to-text API, with the person's own API key, and
  only after the person allows sending audio to that address. Long songs go
  in parts (OpenAI three minutes, Groq eight, a Custom server two) that
  overlap by two seconds; the words in an overlap are kept once.
* **Plain lyrics** are timed by aligning their words with the recognised
  ones (Needleman–Wunsch in a band, words or CJK characters as units,
  NFKC and case folded); words not recognised share the time between the
  words around them. Without plain lyrics, lines come from the recognised
  segments, split at pauses over 0.8 s and after about ten words (sixteen
  CJK characters).
* The result is enhanced LRC with a time for every word, which the app
  labels as made by AI. The plain lyrics also steer recognition: their
  first lines (about 200 tokens) are the recogniser's prompt.

## Settings

| Group | Setting | Values |
| --- | --- | --- |
| Mode | Mode | On This Device (default), Online |
| On This Device | Model | Tiny, Base (default), Small, Large Turbo |
| Online | Provider | OpenAI (default), Groq, Custom |
| Online | Server Address | for Custom: the API address up to its version, such as `http://192.168.1.20:8000/v1` |
| Online | Model | empty: `whisper-1` at OpenAI, `whisper-large-v3-turbo` at Groq, `whisper-1` on a Custom server |
| Language | Language | Detect (default: the lyrics' language when known, else the model decides), or one of 30 languages |
| Automatic | Create Automatically | off (default): when on, each song that starts playing without synced lyrics |

The settings come in English, Simplified and Traditional Chinese and
Japanese (`l10n/`).

## Signing in (Online)

**Sign In** asks for the provider's API key and checks it with one request
that costs nothing (`GET <base>/models`). The key is kept in the system
keystore together with the address it was entered for, so it is never sent
to another provider after the settings change. A Custom server may need no
key at all.

## What is sent where

| Mode | Sent |
| --- | --- |
| On This Device | nothing |
| Online, OpenAI | the song's audio (16 kHz mono WAV) and the key to `api.openai.com` |
| Online, Groq | the same to `api.groq.com` |
| Online, Custom | the same to the Server Address |

The plugin's allowlist is exactly `api.openai.com`, `api.groq.com` and the
Server Address. Once a job has read audio, the app lets it reach only the
addresses the person allowed.

## Tests

```sh
cargo test -p jf-ai-lyrics
UPDATE_GOLDEN=1 cargo test -p jf-ai-lyrics golden   # accept new golden output, then review it
```

* Golden tests (`tests/golden`, invented lyrics and recognised words):
  English, Chinese and Japanese alignment, a chorus written twice and one
  written once but sung twice, missing and extra words, an instrumental,
  lyrics of another song, and line making in English, Chinese and Japanese,
  from segments without word times and around annotations such as
  `[Music]`. Every result must read back through the core's lyrics parser
  with every word timed.
* The real component in the real host (`tests/ai_lyrics.rs`): the plan for
  every mode, model state, key and provider; making lyrics on the device
  with the testkit's audio and recognition; the online client against a
  mock OpenAI-compatible server (words, segments only, HTTP errors, a key
  the provider refuses, parts and their overlaps); consent; sign-in.

OpenAI and Groq themselves were not contacted: the request and answer
shapes follow their documentation as ADR-0019 records it.
