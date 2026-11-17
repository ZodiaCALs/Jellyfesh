# ADR-0019: AI Lyrics: plugin API 1.2 audio and speech, Whisper on the device

* Status: **Accepted**
* Date: 2026-11-17
* Amends: ADR-0003 (runtime), ADR-0004 (packaging and sandbox), ADR-0018 (API 1.1)

## Context

For v0.0.7 the owner asked for AI lyrics "with online and offline modes, as
a plugin". Their answers (2026-11-17):

* **What it makes: synced lyrics.** For a song without lyrics, recognise the
  vocals and time every word. For a song with only plain (unsynced) lyrics,
  time the existing text to the audio.
* **Modes: the person's own key, and a local model.** Online: any
  OpenAI-compatible speech API (OpenAI, Groq or a self-hosted server) with
  the person's own API key, sending audio only after they agree. Offline: an
  open Whisper model, downloaded only when asked, running natively on the
  device.
* **One AI Lyrics plugin** holds the settings and picks the mode. The plugin
  gets new, limited access to a song's audio; the offline model runs in the
  app, not in the plugin.
* AI Lyrics is part of **Jellyfesh Pro** (ADR-0021).

Plugins run in a sandbox with 200 ms of CPU, 10 s of wall-clock time and
64 MiB per call (ADR-0004) and cannot read files. Whisper needs the whole
song and minutes of native compute, so neither the audio nor the model can
live inside the sandbox.

Facts checked on 2026-11-17:

* OpenAI's `POST /v1/audio/transcriptions` returns word timestamps only for
  `whisper-1`, with `response_format=verbose_json` and
  `timestamp_granularities[]=word`; the `gpt-4o-*-transcribe` models answer
  `json` only. Uploads are limited to 25 MB.
* Groq's OpenAI-compatible endpoint
  (`https://api.groq.com/openai/v1/audio/transcriptions`) supports
  `verbose_json` with word and segment granularities for
  `whisper-large-v3` and `whisper-large-v3-turbo`.
* `whisper-rs` 0.16.0 (Unlicense) binds whisper.cpp (MIT), which gives token
  timestamps aligned by DTW over each model's alignment heads.
* whisper.cpp's `models/README.md` publishes the converted models at
  `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin`
  with SHA-1 sums: `tiny` 75 MiB `bd577a113a864445d4c299885e0cb97d4ba92b5f`,
  `base` 142 MiB `465707469ff3a37a2b9b8d8f89f2f99de7299dac`, `small` 466 MiB
  `55356645c2b361a969dfd0ef2c5a50d530afd8d5`, `large-v3-turbo-q5_0` 547 MiB
  `e050f7970618a659205450ad97eb95a18d69c9ee`. The Whisper weights are MIT
  licensed by OpenAI.

## Decision

1. **The AI Lyrics plugin** (`org.jellyfesh.ai-lyrics`, official, signed,
   bundled with every app) makes word-timed lyrics for a song the person
   chooses. Its settings pick the mode:
   * **On This Device** (the default): the `speech` import runs Whisper in
     the app with the chosen model (Tiny, Base, Small, Large Turbo; Base by
     default). Nothing leaves the device.
   * **Online**: the plugin reads the song's audio through the `audio`
     import and sends it to the person's provider (OpenAI, Groq or a Custom
     server address), with the API key the person entered through **Sign
     In** (an `ask-fields` flow; the key lives in the system keystore).
   * **Create Automatically** (off by default) makes lyrics for each song
     that starts playing without synced lyrics.

   Plain lyrics are timed by aligning their words with the recognised words
   (below). Lyrics it makes are enhanced LRC with word timing, labelled as
   made by AI everywhere they appear.

2. **Plugin API 1.2** (`jellyfesh:plugin@1.2.0`). Every 1.0 and 1.1
   interface is unchanged; 1.0 and 1.1 plugins load unchanged, and 1.1 hosts
   refuse 1.2 packages through the manifest check (ADR-0018). New:
   * manifest capabilities `make_lyrics` (export `maker`), `appearance`
     (export `appearance`, ADR-0020), and the permissions `audio` and
     `speech` (imports); the host refuses an import the manifest does not
     declare (`not-declared`), and the install screen lists them;
   * world `plugin`: `include remote-source`, `import audio`,
     `import speech`, `export maker`, `export appearance`. The SDK builds
     `plugin` and answers `unsupported` for what a plugin does not implement;
     hosts look up `maker` and `appearance` through worlds `maker-part` and
     `appearance-part`, as they look up `remote` (ADR-0018).

3. **The new interfaces** (names and shapes; the WIT files carry the full
   documentation):

   ```wit
   /// The audio of the song a `maker.make` call is about (permission `audio`).
   interface audio {
       /// A song in one `make` call; meaningless outside that call.
       type track-handle = u64;
       record clip-info { duration-ms: u64 }
       variant audio-error {
           not-declared,
           /// The person has not allowed sending audio to these origins.
           no-consent(list<string>),
           /// The song cannot be read (another device plays it; its source
           /// gives no audio).
           unavailable(string),
           cancelled,
       }
       info: func(track: track-handle) -> result<clip-info, audio-error>;
       /// 16 kHz mono 16-bit PCM WAV of [start-ms, start-ms + length-ms),
       /// at most 8 minutes per call.
       wav: func(track: track-handle, start-ms: u64, length-ms: u64)
           -> result<list<u8>, audio-error>;
   }

   /// Speech recognition in the app (permission `speech`).
   interface speech {
       use audio.{track-handle};
       record model { id: string, name: string, bytes: u64, ready: bool }
       record options {
           model: string,
           /// BCP 47 hint; none detects the language.
           language: option<string>,
           /// Text that steers recognition, such as the lyrics being timed.
           prompt: option<string>,
       }
       record word { text: string, start-ms: u32, end-ms: u32, confidence: f32 }
       record segment { text: string, start-ms: u32, end-ms: u32, words: list<word> }
       record transcript { language: option<string>, segments: list<segment> }
       variant speech-error {
           not-declared, model-missing(string), unavailable(string),
           cancelled, failed(string),
       }
       models: func() -> list<model>;
       transcribe: func(track: track-handle, opts: options)
           -> result<transcript, speech-error>;
   }

   /// Making lyrics (capability `make_lyrics`).
   interface maker {
       use types.{plugin-error};
       use lyrics.{lyric-payload};
       use audio.{track-handle};
       enum mode { on-device, online }
       variant need {
           nothing,
           /// The online mode needs the person's API key (Sign In).
           sign-in,
           /// The on-device mode needs this model downloaded.
           model(string),
           unavailable(string),
       }
       record plan {
           mode: mode,
           /// "Whisper Base on this device", "whisper-1 at api.openai.com".
           engine: string,
           /// The origin audio goes to in the online mode.
           upload-to: option<string>,
           need: need,
           automatic: bool,
       }
       record song {
           track: track-handle,
           title: string,
           artist: option<string>,
           album: option<string>,
           duration-ms: u64,
           /// Plain lyrics to time, when the song has them.
           text: option<string>,
           language: option<string>,
       }
       record made { lyrics: lyric-payload, engine: string }
       /// What `make` would do with the current settings. Cheap; no network.
       plan: func() -> result<plan, plugin-error>;
       make: func(song: song) -> result<made, plugin-error>;
   }
   ```

4. **Consent and enforcement for the online mode.** Before a job whose plan
   has `upload-to`, the core asks the person once per plugin and origin
   ("AI Lyrics will send this song's audio to api.openai.com to make its
   lyrics. The provider's terms and privacy policy apply, and it may charge
   your account.", Cancel, Allow) and records the answer in settings
   (`Command::AllowAudioUpload`; Settings shows it with **Stop Allowing**).
   The host enforces it: `audio.wav` answers `no-consent` unless the plugin
   has at least one allowed origin, and once a call has read audio, the rest
   of that call may send HTTP requests only to allowed origins and may not
   write to `kv`. The on-device mode reads no audio in the plugin and needs
   no consent.

5. **On-device recognition: `core/jf-speech`.** A crate over `whisper-rs`
   (feature `whisper`, on for the macOS, Windows, Linux and Android builds,
   off for wasm32), CPU only in v0.0.7:
   * a catalogue of the four models above (id, name, file, approximate size,
     SHA-1 from whisper.cpp, SHA-256 once CI has computed it, DTW preset);
   * downloads only on request, to `<data>/speech-models/`, resumable with
     HTTP ranges, verified before use, cancellable, deletable;
   * decoding the song with the engine's decoders and resampling to 16 kHz
     mono on the job's thread (library files and the sources' streams);
   * `Engine::transcribe(pcm, options, progress, cancel)`: word timing from
     whisper.cpp's DTW token timestamps, words assembled from tokens (one
     word per character run for CJK), progress through whisper.cpp's
     progress callback, cancellation through its abort callback; at most
     `min(4, cores − 1)` threads; one recognition at a time.
   Jellyfesh Web has no on-device mode in v0.0.7: `speech` answers
   `unavailable` there.

6. **Alignment** (in the plugin, golden-tested): the plain lyrics and the
   recognised words are split into units (words for space-separated
   scripts; characters for Chinese, Japanese and Korean, a recognised CJK
   word's time divided by character), normalised (NFKC, case folding,
   punctuation removed), and aligned globally (Needleman–Wunsch with a band;
   matching by edit-distance similarity). Matched units take the recognised
   times; unmatched runs are spread between their anchors by length; times
   are made monotonic with at least 60 ms per word; each lyric line starts
   at its first word. Without plain lyrics, lines come from the recognised
   segments, split at pauses over 0.8 s and after about 10 words (16 CJK
   characters).

7. **Core.** A slice `AiLyrics` (`available`, the plugin, its `plan`, what
   is needed first, the jobs with their stage, and the speech models with
   their download state); commands `MakeLyrics { song }` (the current song
   or a library song), `CancelLyrics { job }`, `DeleteMadeLyrics { song }`,
   `AllowAudioUpload` and `RevokeAudioUpload { plugin, origin }`,
   `DownloadSpeechModel`, `CancelSpeechModelDownload` and
   `DeleteSpeechModel { id }`; `LyricsSource::Made { plugin, engine }`.
   Made lyrics are kept in `<data>/made-lyrics/` (an `.lrc` and a small JSON
   with the engine and date per song), never shared, and used when a song's
   own lyrics are missing or unsynced. Job stages come from what the host
   sees: Reading Audio (`audio.wav`), Sending to {origin} (HTTP while the
   call holds audio), Recognising (with whisper.cpp's progress), Timing
   Words (after the last import), then Done, Failed or Cancelled. One job
   runs at a time; the others wait.

8. **Limits for `maker.make`:** 30 minutes of wall-clock time, 10 s of plugin
   CPU (time inside host imports does not count), 128 MiB of memory;
   `audio.wav` at most 8 minutes per call; HTTP bodies stay at 16 MiB.
   Other exports keep ADR-0004's limits.

9. **Pro.** `make_lyrics` and `appearance` are Pro capabilities: the host
   loads such plugins but refuses their calls (`pro-required`) unless
   Jellyfesh Pro is active (ADR-0021).

## Consequences

* Songs that play on another device (remote sources) or only open in
  another app have no audio, so AI Lyrics says it cannot make their lyrics.
* Recognition of singing over loud music is imperfect; the made lyrics are
  labelled, can be made again with another model, and can be deleted.
  Separating the vocals first (a further model) is left for later.
* whisper.cpp adds a C++ build (CMake) to every native app build.
* Online mode costs the person whatever their provider charges.

## Tests

* Golden tests for alignment and line making (`plugins/ai-lyrics/tests/golden`:
  English, Chinese, Japanese, repeated choruses, missing and extra words,
  instrumental gaps), and for the OpenAI-compatible client against a local
  mock server (word timestamps, segments only, errors, chunking).
* Sandbox and limit tests in the plugin host: undeclared imports, consent,
  HTTP and `kv` after reading audio, handles from another call, the 8-minute
  cap, cancellation, CPU budget excluding import time, the wall-clock cap.
* `jf-speech`: the catalogue, resumed and verified downloads against a local
  server, and in CI a recognition smoke test with `ggml-tiny` and speech
  from `espeak-ng` (skipped locally unless `JF_WHISPER_MODEL` is set).
* The core's job states with a fake engine and a test plugin; FFI contract
  vectors for the new types.

## Revisit when

* OpenAI or another provider offers word timing in its newer models.
* GPU back-ends (Metal, Vulkan) or a browser engine are worth their build cost.
* Vocal separation becomes cheap enough for phones.
