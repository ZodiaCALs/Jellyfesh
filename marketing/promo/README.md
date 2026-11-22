# Jellyfesh v0.0.7 promo video

The public beta's promotion video: 70 seconds, 1920 × 1080, 60 fps, H.264
with an original music bed, no voice-over. The finished file is published
with the website, at
`website/assets/video/jellyfesh-v0.0.7-public-beta.mp4` (with `poster.jpg`
next to it). This folder holds only what makes it.

| Path | What it is |
| --- | --- |
| `scene/index.html`, `scene/scene.css`, `scene/scene.js` | The video as a web page: every scene, drawn for a time *t* that the render script sets |
| `scene/content.js` | The invented sample content: albums, songs, lyric lines, comments |
| `assets/covers/` | Abstract gradient covers for the invented albums |
| `assets/themes/`, `assets/backdrops/` | The theme colours and Now Playing backdrops Jellyfesh's core makes from those covers |
| `tools/make_assets.py` | Makes the covers, and the themes and backdrops with `jf theme` |
| `music/make_music.py` | Synthesises the music bed (numpy only) |
| `render.mjs` | Renders every frame in Chromium and encodes the video with ffmpeg |
| `fonts/` | Inter (the stand-in for the apps' system font) and the faces the Customize scene switches between, with their licences |
| `CREDITS.md` | Fonts and licences, and what is original |

## Beats

The music is 96 BPM, so a bar is 2.5 s; every cut lands on a bar.

| Time | Beat |
| --- | --- |
| 0:00 | Cold open: the aurora, "Your music. Beautifully.", the Jellyfesh lettering |
| 0:07.5 | AI Lyrics: Create Synced Lyrics, "Recognising… 42%", "Timing Words…", the lyrics filling word by word, "Made by AI Lyrics · Whisper Base on this device", the two modes |
| 0:20 | The lyrics bar in the player bar, then Desktop Lyrics |
| 0:27.5 | Customize: corners, glass, aurora, typeface, lyrics glow and colour, the three Now Playing layouts |
| 0:40 | Discover (For You, Charts, Recently Added), Get Info (Hi-Res Lossless, FLAC · 24-bit · 96 kHz), Comments |
| 0:50 | One app, five platforms |
| 0:57.5 | Jellyfesh Pro: AI Lyrics and Customize, free during the public beta |
| 1:02.5 | End card: Jellyfesh · Public Beta v0.0.7, the website, the disclaimer |

## Re-rendering

Needs Node 18 or later with Playwright and its Chromium (`npm i playwright`
and `npx playwright install chromium`; a global installation is found too,
or set `PLAYWRIGHT_MODULE` to its folder), ffmpeg with libx264, and Python 3
with numpy (Pillow too for the covers).

```bash
# Optional: only when the covers or their colours should change.
cargo build -p jf-cli                       # jf, for the core's theme colours
python3 marketing/promo/tools/make_assets.py

# The music (also made by render.mjs when it is missing).
python3 marketing/promo/music/make_music.py marketing/promo/build/music.wav

# The video and the poster, into website/assets/video/.
node marketing/promo/render.mjs
```

`render.mjs` serves the repository on a local port (the scene uses the
website's brand files), renders the frames with three pages at once, keeps
them as JPEG in `build/frames` (about 1.5 GB at 60 fps) for a two-pass x264
encode at 2.55 Mb/s, then deletes them and adds the music (AAC 192 kb/s)
with `+faststart`. Options:

| Option | Does |
| --- | --- |
| `--fps 30` | Another frame rate (default 60) |
| `--from 7.5 --to 20` | Renders only that part, without music, to `build/part.mp4` |
| `--stills 2.6,12.6,44.8` | Saves PNG stills of those times to `build/stills/` |
| `--build DIR` | Puts frames and intermediate files somewhere else |
| `--workers N` | Pages rendering at once (default 3) |
| `--recycle N` | Frames a page draws before it is replaced (default 120). Headless Chromium holds on to memory for every frame whose transforms change scale, so long renders run out of memory without this |
| `--kbps N` | Video bitrate for the full render (default 2550) |
| `--keep-frames` | Keeps `build/frames` after the encode |
| `--poster-at S` | The time the poster is taken from (default 6.3, the lettering) |

To look at a moment while editing, open `scene/index.html` through any local
web server at the repository root, then call `await buildAll()` and
`renderAt(12.6)` in the browser console.

## Rules the video keeps

* **Simulated, and says so.** Every screen is a recreation of Jellyfesh's
  real design (the Luminous tokens, the web app's components and the core's
  colours), never a capture, and the label "Simulated screens · Sample
  content" stays on screen while screens are shown. The end card says
  "Screens are simulated with sample songs and artwork. Features as of public
  beta v0.0.7; some vary by platform and device."
* **Invented content.** Every artist, album, song, lyric line, comment,
  account and source name is made up (`scene/content.js`), and the covers
  are abstract gradients.
* **Only what v0.0.7 does** (`docs/releases/v0.0.7.md`, `docs/ui.md`):
  on-device AI Lyrics is not claimed for Jellyfesh Web or Windows on Arm (a
  footnote says so), online mode uses the person's own key, Pro is free
  during the beta through Pro Preview, and there are no prices.
* **No one else's marks.** No company or service logos, no device frames
  and no system window controls: platforms are named in words over plain
  rounded windows, and the typefaces are open-licensed.
