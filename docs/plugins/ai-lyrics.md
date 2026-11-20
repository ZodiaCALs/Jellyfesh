# AI Lyrics

The AI Lyrics plugin makes lyrics that follow the song word by word. For a
song without lyrics it listens to the singing and writes down every word
with its time; for a song that has only plain lyrics (no times) it times
those lyrics to the music. It comes with every Jellyfesh app from v0.0.7
and updates with it; the same plugin is also published as a separate
download (`org.jellyfesh.ai-lyrics`, signed with the Jellyfesh release
key). AI Lyrics is part of **Jellyfesh Pro**. How it works inside:
`plugins/ai-lyrics/README.md` and ADR-0019.

## What it is

AI Lyrics uses Whisper, a speech recognition model, either on your device
or at an online service you choose. The result is labelled in the app as
made by AI: recognition makes mistakes, especially with loud music, several
voices or words sung very fast. When the song has plain lyrics, the plugin
keeps their words and only adds the times, so the text stays right even
where the recognition is not.

## Before you start

* **Jellyfesh Pro.** Making lyrics is a Pro feature; without Pro the
  plugin's actions say so.
* **On This Device** (the default) needs a speech model, downloaded once
  when you first ask for one: Tiny (fastest, least accurate), Base, Small or
  Large Turbo (slowest, most accurate). Larger models need more memory and
  time; Base is a good start.
* **Online** needs an account and an API key at OpenAI or Groq, or the
  address of your own server that speaks OpenAI's speech-to-text API. You
  pay the provider for what you use.

## Set it up

1. In Jellyfesh's settings, open the plugin **AI Lyrics** (among the
   extensions).
2. Choose the **Mode**:
   * **On This Device**: choose a **Model**. Jellyfesh offers to download it
     the first time you make lyrics.
   * **Online**: choose the **Provider** (OpenAI, Groq or Custom; for Custom
     also the **Server Address**, such as `http://192.168.1.20:8000/v1`),
     then **Sign In** with your API key. Jellyfesh checks the key with one
     request that costs nothing and keeps it in your system's keystore,
     tied to that address.
3. Optionally set the **Language** (Detect, the default, uses the lyrics'
   language when known), and **Create Automatically** to make lyrics for
   every song that starts playing without synced lyrics.

## Settings

| Setting | What it does |
| --- | --- |
| Mode | On This Device or Online |
| Model (On This Device) | Tiny, Base (default), Small or Large Turbo |
| Provider (Online) | OpenAI (default), Groq or Custom |
| Server Address (Online) | your server, for Custom |
| Model (Online) | empty for the provider's default: `whisper-1` at OpenAI, `whisper-large-v3-turbo` at Groq |
| Language | Detect, or one of 30 languages |
| Create Automatically | off by default |

The settings are in English, Simplified Chinese, Traditional Chinese and
Japanese.

## Making lyrics

Ask for AI lyrics from the Lyrics pane, a song's menu or Now Playing. The
app shows what is happening (reading the song, sending, recognising, timing
the words) and you can cancel. Online, long songs go in overlapping parts.
The lyrics are kept for the song and show word by word in Now Playing, the
lyrics bar and Desktop Lyrics; you can delete them.

## Privacy

| Mode | What leaves your device |
| --- | --- |
| On This Device | nothing |
| Online, OpenAI | the song's audio (16 kHz mono) and your key, to `api.openai.com` |
| Online, Groq | the same, to `api.groq.com` |
| Online, Custom | the same, to your Server Address |

Before the first upload to an address, Jellyfesh asks whether you allow
sending your music there ("AI Lyrics will send this song's audio to …");
Settings shows what you allowed, with **Stop Allowing**.
While making lyrics, the plugin can reach only the addresses you allowed.
The providers' own terms and privacy policies apply to what you send them.

## When something goes wrong

| Message | What to do |
| --- | --- |
| Jellyfesh Pro is needed | Get Pro, or turn the feature off. |
| The model is not downloaded | Download it when asked, or choose a smaller one. |
| Sign in / the key was refused | Sign in again with a current key from your provider. |
| Not allowed to send audio there | Allow the address when asked, or use On This Device. |
| The service asks to slow down / is unavailable | Wait and try again; your provider's limits apply. |
| The lyrics are wrong in places | Recognition makes mistakes; plain lyrics, if you have them, make the text right. |

The online client was tested against a mock server that follows OpenAI's and
Groq's documentation, not against the live services.
