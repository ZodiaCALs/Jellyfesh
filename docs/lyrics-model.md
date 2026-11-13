# Lyrics model (`LyricDoc`) and parser coverage

Status: **proposed (Step 0)**. Implemented in `jf-lyrics` in Milestone 1.

Every source (embedded tags, sidecar files, plugins) converts into one
model, so Now Playing renders all lyrics the same way.

## Model

```text
LyricDoc {
  language: Option<BCP47>,            // e.g. "ja", "zh-Hans"; drives font fallback (Misty INT-04)
  timing: Unsynced | Line | Word,     // best precision present in the doc
  offset_ms: i32,                     // from [offset:] or TTML; already applied to times
  agents: Vec<Agent>,                 // singers/voices: id, name, kind (person | group | other)
  lines: Vec<Line>,
  tracks: Vec<AuxTrack>,              // translation and romanization tracks
  source: SourceInfo,                 // format, origin (file, tag, plugin id), licence text if any
}

Line {
  id: u32,
  start_ms, end_ms: Option<u32>,      // end inferred from next line when absent
  agent: Option<AgentId>,             // duets (TTML ttm:agent, enhanced LRC "v1:")
  text: String,                       // NFC, as displayed
  words: Vec<Word>,                   // empty when timing is Line
  background: Vec<Word>,              // background vocals (TTML x-bg spans), shown smaller
  direction: Ltr | Rtl | Auto,        // per line; Arabic and Hebrew lyrics exist
}

Word { start_ms, end_ms, text: String, joins_next: bool }  // joins_next: no space before the next word (CJK)

AuxTrack {
  kind: Translation | Romanization,
  language: BCP47,                    // e.g. "en", "ja-Latn", "zh-Latn-pinyin"
  lines: Map<LineId, String or Vec<Word>>,   // aligned by line; word timing optional
}
```

Rules:

* Times are normalised to milliseconds from the start of the audio, after
  applying offsets. Overlapping lines are allowed (duets); the cursor picks
  the latest started line per agent.
* `LyricTimeline` (sent to shells) flattens line starts and word starts into
  sorted arrays so shells only binary-search (ADR-0001).
* Text keeps the source's characters; we never transliterate. Romanization
  comes only from a source that provides it.

## Formats

| Format | Coverage | Notes |
| --- | --- | --- |
| LRC | `[mm:ss.xx]` with 1–3 fraction digits, multiple stamps per line, `[offset:]`, `[ti:]`/`[ar:]`/`[al:]`/`[length:]` metadata | Blank timed lines mark instrumental gaps |
| Enhanced LRC (A2) | Word stamps `<mm:ss.xx>` inside lines; `v1:`/`v2:` voice prefixes | Last word ends at the next line start or `end` stamp |
| Bilingual LRC | Two lines with the same stamp | The second becomes a translation track (heuristic: script differs); a toggle turns this off |
| TTML | `tt/body/div/p/span` with `begin`/`end`/`dur` in clock and offset time; `itunes:timing` Line or Word; `ttm:agent`; `x-bg` background spans; `ttm:role="x-translation"` and `x-roman` sidecar tracks | Parsed with `quick-xml`; entities and namespaces handled; no DTD or external entities (XXE-safe) |
| WebVTT | Cues, cue timings, `<c>` and `<v Speaker>` spans, inline timestamps `<00:00:01.000>` for word timing | Positioning and styling ignored |
| Plain text | Unsynced | Shown as a static, scrollable text |
| Embedded tags | ID3 `USLT` (unsynced), `SYLT` (synced; decoded via lofty's frame parser), Vorbis `LYRICS`/`UNSYNCEDLYRICS`, MP4 `©lyr` | Embedded LRC text inside `USLT` is detected and parsed as LRC |
| Plugin payloads | Any of the above, plus `json-syllables` (a simple JSON word list for services with proprietary word-timing formats, converted by the plugin) | Plugins convert proprietary formats themselves; the host only accepts these declared formats |

## Golden tests

`core/jf-lyrics/tests/golden/<format>/<case>.<ext>` → `<case>.json`
(pretty, stable key order). Cases include: CJK with and without spaces,
Japanese with furigana-style romanization track, Arabic (RTL) lines, mixed
scripts, duets with agents, background vocals, zero-length words, unsorted
stamps, repeated stamps, negative offsets, CRLF and BOM, invalid XML,
truncated files, huge files (limits), and timestamps past the track length.
A failing golden test prints a diff; updating goldens needs an explicit
`UPDATE_GOLDEN=1` run that shows up in review.

## Display behaviour (shell side, from tokens)

* One size for every line; the active line changes colour only (ink-90 vs
  ink-50); the karaoke sweep fills words linearly over their duration.
* The active line scrolls to 38% of the viewport in 380 ms (Fluid). Manual
  scrolling pauses follow mode for 4 s.
* Reduce motion: no sweep, no animated scroll; the active line is marked and
  the view jumps.
* Translation and romanization tracks can be toggled per track and shown
  below each line in the smaller lyric styles.
