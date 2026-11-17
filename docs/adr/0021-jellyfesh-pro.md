# ADR-0021: Jellyfesh Pro, editions and commercial readiness

* Status: **Accepted**
* Date: 2026-11-17
* Amends: ADR-0012 (community plugins), ADR-0016 (distribution, licence and versions)

## Context

The owner asked to "make it ready to be a commercial app" in v0.0.7 and chose
**Free + Pro** (2026-11-17): a free app with a Pro unlock (AI Lyrics,
Customize) bought in the app stores or with a licence key; an EULA, a
privacy policy, store packaging and signing set up; and the unofficial
community plugins as a separate download in commercial builds.

What exists: unsigned direct downloads from GitHub releases (ADR-0016), no
store accounts yet (the Apple Developer Program and MusicKit are still open
owner items), Android release signing from `ANDROID_*` secrets, the plugin
release key kept out of the repository, "no account, no analytics, no
tracking". v0.0.7 is a public beta and the stores' products do not exist
yet.

Facts checked on 2026-11-17: StoreKit 2 verifies App Store transactions on
the device (`Transaction.currentEntitlements`, `VerificationResult`);
`Windows.Services.Store.StoreContext` reports add-on licences for packaged
apps (`GetAppLicenseAsync`) and needs the window handle in desktop apps;
Google Play Billing Library 9.1.0 (June 2026) is current and 8.0.0 or later
is required for updates; `ed25519-dalek` 3.0.0 is already a workspace
dependency.

## Decision

1. **Free and Pro.** Everything in v0.0.6 stays free. **Jellyfesh Pro** is
   one non-consumable purchase (no subscription: online AI Lyrics uses the
   person's own key, so Jellyfesh has no running costs to pass on) and
   unlocks the Pro capabilities: `make_lyrics` and `appearance` (ADR-0019,
   ADR-0020) and future Pro features. Prices are set in each store and on
   the licence-key shop, not in the code.

2. **Where Pro comes from** (the core decides; apps only report what their
   store says):
   * **App stores.** The store builds sell product `org.jellyfesh.pro` (App
     Store: in the app's group; Microsoft Store: a durable add-on; Google
     Play: a one-time product `pro`). The app's store glue verifies with the
     platform (StoreKit 2 verification, `StoreContext` licences, Play
     Billing purchase state and signature) and calls
     `Core::set_store_entitlement(StoreEntitlement { store, product, active,
     transaction })`; it offers **Buy Jellyfesh Pro** with the store's price
     and **Restore Purchases**.
   * **Licence keys** for every app (the only way on Linux and the web):
     `JFP1-…`, Crockford base32 of a 16-byte payload (format 1, key id,
     product, flags, a random 64-bit licence id, the issue day) and an
     Ed25519 signature over `"jellyfesh licence v1\0" ‖ payload`, verified
     offline by `core/jf-licence` against the public keys built in
     (`JF_LICENCE_PUBLIC_KEYS`, a repository variable: public keys are not
     secret) and a revocation list shipped in updates. Keys hold no name or
     email. `tools/jf-licence` makes the owner's key pair (the private key
     stays outside the repository, in a password manager or the secret
     `JF_LICENCE_SIGNING_KEY`) and issues and checks keys.
     `Command::ActivateLicence { key }` and `RemoveLicence`.
   * **Pro Preview.** Builds made with `JF_PRO_PREVIEW=1` treat Pro as
     active and say so ("Pro features are free during the public beta").
     The v0.0.7 public beta is built with it (the release workflow's
     `pro_preview` input, on by default for pre-releases), because no store
     product or key shop exists yet; turning it off is one input.
   The `Pro` slice: `active`, `source` (None, Preview, Store { store },
   Licence { id suffix, issued }), `preview`, `licence_error`
   (Malformed, UnknownKey, BadSignature, WrongProduct, Revoked; the apps
   word them), `stores` the build can sell in.

3. **Gating.** A plugin with a Pro capability installs and shows, but its
   calls answer `pro-required` while Pro is inactive; the app shows the Pro
   sheet instead of an error. Turning Pro off stops jobs and returns the
   `Look` slice to the defaults; nothing is deleted.

4. **Distribution and editions.** The app tells the core how it was
   distributed (`CoreConfig.distribution`: Direct, AppStore,
   MicrosoftStore, GooglePlay, Web, Source). **Commercial builds** (every
   build the release workflow makes, `JF_EDITION=commercial`) do not bundle
   the unofficial community plugins; their packages are separate downloads
   on the release page, installed with **Install from File…**, and **Allow
   Community Plugins** still gates them (ADR-0012). Builds from source
   (`JF_EDITION=community`, the default) bundle them as in v0.0.6. Whether
   App Store builds may install plugins from files at all is a question for
   App Review (guideline 2.5.2); it stays possible until review says
   otherwise.

5. **Updates.** Store builds update through their stores. Direct builds
   check GitHub's releases API at most once a day while **Check for Updates
   Automatically** is on (the default for Direct; hidden elsewhere) and on
   **Check for Updates**; a newer version (pre-releases included while
   Jellyfesh is in beta) shows "Jellyfesh 0.0.8 Is Available" with a link to
   its release page. Nothing is installed automatically in v0.0.7. The
   request carries no identifier beyond what any HTTPS request does.

6. **Legal documents** (`legal/`, also on the website and linked from
   About and the Pro sheet): End User Licence Agreement, Privacy Policy and
   Terms of Sale, written as **drafts for review by a qualified lawyer**
   before Jellyfesh is sold, with the seller's name, address and governing
   law left as marked blanks. The privacy policy states exactly what leaves
   the device: what plugins send to the services the person sets up, audio
   sent by online AI Lyrics after consent, the update check, and store
   purchases handled by the stores.

7. **Packaging and signing**, each step skipped with a notice when its
   secrets are missing:
   * **macOS:** Developer ID signing, notarization and stapling for direct
     downloads (`APPLE_DEVELOPER_ID_P12`, `APPLE_DEVELOPER_ID_PASSWORD`,
     `APPLE_TEAM_ID`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`,
     `APPLE_NOTARY_KEY`); a sandboxed Mac App Store package
     (`APPLE_MAS_APP_P12`, `APPLE_MAS_INSTALLER_P12`, `APPLE_MAS_PASSWORD`,
     `APPLE_MAS_PROFILE`) with its own entitlements; a StoreKit
     configuration file for local purchase tests.
   * **Windows:** an MSIX upload package for the Microsoft Store with the
     identity from Partner Center (`MS_STORE_IDENTITY_NAME`,
     `MS_STORE_PUBLISHER`, `MS_STORE_PUBLISHER_NAME`), and signing of the
     direct package (`WINDOWS_SIGNING_PFX`, `WINDOWS_SIGNING_PASSWORD`).
   * **Android:** a signed App Bundle for Google Play from the existing
     `ANDROID_*` secrets, with Play Billing 9.1.0 and the licence key from
     Play Console (`PLAY_LICENSE_KEY`, a public key) for purchase
     signatures.
   * **Linux and the web:** unchanged packages; Pro by licence key.
   `docs/store-release.md` lists what each store needs before submission
   (products, privacy answers, export compliance, screenshots, review
   notes).

## Consequences

* The source stays public and all rights reserved (ADR-0016); someone who
  builds it can remove the gate. The gate keeps honest people honest and
  pays for nothing that needs a server.
* Store purchases cannot be tested end to end until the store accounts and
  products exist; the StoreKit configuration file and Play's test tracks
  cover what can be tested before.
* The legal texts are not legal advice and must be reviewed before sale.

## Tests

* `jf-licence`: key round trips with a key pair made in the test (never a
  release key), every error, case and separator tolerance, revocation,
  unknown key ids.
* The core: Pro sources and their precedence, gating of Pro calls, the
  `Look` reset, store entitlements, update comparison against a local mock
  of the releases API (pre-releases, newer, older, malformed).
* Packaging scripts: shellcheck and dry runs without secrets in CI.
