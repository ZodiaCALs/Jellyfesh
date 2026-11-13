# ADR-0010: Apple Music vendor adapters

* Status: **Deferred to the commercial phase** (2026-11-05: developer accounts wait until then; ADR-0016)
* Date: 2026-11-05
* Findings: [`../sources-report.md`](../sources-report.md#apple-music)

## Context

The brief plans native MusicKit on macOS and Android, and MusicKit JS in a
lazily created, destroyed-after-use webview on Windows and Linux. Apple
Music is DRM-protected; the engine never sees its audio (Vendor mode).

**Verified:**

* macOS: `ApplicationMusicPlayer` needs **macOS 14+**; `SystemMusicPlayer`
  is not available on macOS. With the MusicKit App Service enabled on the
  App ID, MusicKit generates developer tokens itself, so no private key
  ships in the app. Needs an Apple Developer Program membership, the
  `NSAppleMusicUsageDescription` key, user authorisation and an active
  subscription (`canPlayCatalogContent`).
* Android: Apple's MusicKit page still offers "MusicKit for Android" behind
  a developer login, but its documentation URL returns 404, and Apple's API
  docs say automatic user-token management is not available on Android.
* MusicKit JS v3: the developer token is an ES256 JWT signed with our
  MusicKit private key, `exp` at most 6 months, optional `origin` claim.
* WebView2: no Microsoft documentation of DRM support; an open feedback
  issue asks for Widevine and says PlayReady works. WebKitGTK builds ship
  without EME (`ENABLE_ENCRYPTED_MEDIA` off; its CDM backend is
  developer-only), so it cannot play Widevine content.
* Apple Developer Program License Agreement §3.3.6(D) (2026-10-29): MusicKit
  may only be used to give **your users access to their own Apple Music
  subscriptions**; "full songs must be enabled for playback"; playback must
  be user-initiated; content may be played "only as rendered by the
  MusicKit APIs or MusicKit JS"; it "cannot be synchronized with any other
  content"; artwork and music text may not be used apart from playback or
  playlist management; MusicKit JS only as a stand-alone, unmodified library.
  The ADPLA defines an "Application" as software for Apple-branded products.
* App Review 4.5.2(i): user-initiated playback, standard controls, no
  monetisation of Apple Music access.
* The Apple Music API exposes only a `hasLyrics` flag; there is no lyrics
  API for third parties.

## Decision

1. **macOS: yes (Milestone 5).** A Swift adapter wraps
   `ApplicationMusicPlayer` and implements the core's `ExternalPlayer`
   interface (load, play, pause, seek, skip, state stream). Catalog search,
   library and playlists come from MusicKit in Swift and are exposed to the
   core through an `ExternalCatalog` foreign trait, so search and browsing
   reuse the plugin UI.
2. **Android: spike before commitment.** S12 obtains the current MusicKit
   for Android SDK and its licence, and checks sign-in and playback against
   today's Apple Music app. If any of the three fails, Android ships without
   Apple Music.
3. **Windows: not until three conditions hold.** (a) S13 proves full-track
   playback of MusicKit JS in WebView2 (`requestMediaKeySystemAccess` for
   PlayReady or Widevine, then a full track, not a 30-second preview);
   (b) a legal reading that the ADPLA permits MusicKit JS inside a Windows
   desktop app; (c) a developer-token service: a tiny endpoint that signs
   short-lived tokens (hours, not months) with an `origin` claim. Shipping a
   pre-signed 6-month token in the app is rejected: anyone could extract it,
   and revoking it means rotating our key.
4. **Linux: no Apple Music.** WebKitGTK cannot play the DRM streams, a
   preview-only integration would break "full songs must be enabled", and
   bundling a Widevine CDM breaks the brief's non-goal on proprietary
   components.
5. **Isolation rules for Apple Music items** (conservative reading of
   §3.3.6(D), pending review ⚑):
   * No crossfade, EQ, ReplayGain or visualisation (impossible anyway; the
     UI says so with the Vendor playback badge).
   * **No third-party synced lyrics during Apple Music playback**, because
     the ADPLA forbids synchronising MusicKit content with other content.
   * Artwork-derived theme colour is allowed only while the item plays.
   * Search results appear in their own Apple Music group, with Apple's
     identity badge; they are never deduplicated against local or other
     sources' tracks.
   * Nothing from MusicKit is stored in our library database beyond what
     MusicKit itself caches; a queue may hold Apple Music items, which
     always play through the adapter.

## Consequences

* Apple Music is a macOS feature at first; the other platforms depend on
  spikes and legal review.
* The Vendor and Remote modes share one `ExternalPlayer` contract, so
  Spotify Remote (ADR-0011) reuses the same session plumbing.
* We need an Apple Developer Program team, an App ID with the MusicKit App
  Service, and (for Windows) a MusicKit key and a token service. ⚑
* Open question for M5: whether MusicKit works in Developer ID (non-App
  Store) builds; reports conflict.
