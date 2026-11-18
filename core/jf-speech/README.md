# jf-speech

On-device speech recognition for AI Lyrics (ADR-0019 §5): the Whisper model
catalogue, model downloads and word-timed transcripts.

* `catalogue()` and `model(id)`: the four models the app offers.
* `ModelStore`: downloads a model only when asked, into a folder the app
  chooses (`<data>/speech-models/`). Downloads continue with HTTP ranges,
  can be cancelled (what arrived is kept) and are checked against the
  published SHA-1 (and SHA-256 once pinned) before the model is used.
* `Recognizer` and `recognizer()`: 16 kHz mono audio in, segments with timed
  words out. With the feature `whisper`, `recognizer()` gives whisper.cpp's
  recogniser (through [whisper-rs](https://crates.io/crates/whisper-rs));
  without it, `None` (Jellyfesh Web).

## The feature `whisper`

whisper-rs-sys builds whisper.cpp 1.8.3 from source for the CPU, so a build
with the feature needs:

* CMake and a C and C++ compiler;
* libclang, for bindgen (or `WHISPER_DONT_GENERATE_BINDINGS=1`, which uses
  the bindings whisper-rs-sys ships, generated on Linux x86-64).

whisper-rs-sys passes every `CMAKE_*`, `GGML_*` and `WHISPER_*` environment
variable to that CMake build. Builds that are shipped need `GGML_NATIVE=OFF`:
otherwise ggml is compiled for the build machine's processor (`-march=native`,
or on Windows the instruction sets it finds there) and can crash on the
people's computers. On Windows the static C runtime needs
`CMAKE_POLICY_DEFAULT_CMP0091=NEW` and `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded`,
and an optimised build `CMAKE_C_FLAGS_RELEASE` and `CMAKE_CXX_FLAGS_RELEASE`
set to `-O2 -Ob2 -DNDEBUG` (CI sets them; see `.github/workflows/ci.yml`).
On Android the library needs `libc++_shared.so` next to it
(`cargo ndk --link-libcxx-shared`). ggml does not build for Windows on Arm
with MSVC; it needs clang-cl there.

## Tests

```sh
cargo test -p jf-speech                      # catalogue, downloads, word assembly
cargo test -p jf-speech --features whisper   # also language codes and DTW presets
JF_WHISPER_MODEL=ggml-tiny.bin JF_WHISPER_WAV=speech.wav JF_WHISPER_EXPECT=hello,world \
  cargo test -p jf-speech --features whisper --test whisper_smoke -- --ignored
```

The download tests run against a local HTTP server that serves test data,
not models. The smoke test needs a real model and 16 kHz mono speech; CI's
`speech` job runs it with Tiny and a sentence spoken by espeak-ng.

## Models

The models are OpenAI's [Whisper](https://github.com/openai/whisper) weights,
released by OpenAI under the MIT License, converted to the `ggml` format by
whisper.cpp and published at
`https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{id}.bin`.
None is part of the app: a model is downloaded only when the person asks for
it, and can be deleted again.

| Id | Name | Size | SHA-1 |
| --- | --- | --- | --- |
| `tiny` | Tiny | 75 MiB | `bd577a113a864445d4c299885e0cb97d4ba92b5f` |
| `base` | Base (default) | 142 MiB | `465707469ff3a37a2b9b8d8f89f2f99de7299dac` |
| `small` | Small | 466 MiB | `55356645c2b361a969dfd0ef2c5a50d530afd8d5` |
| `large-v3-turbo-q5_0` | Large Turbo | 547 MiB | `e050f7970618a659205450ad97eb95a18d69c9ee` |

Sizes and SHA-1 sums are whisper.cpp's (`models/README.md`, checked
2026-11-17). All four are multilingual.
