# Selling Jellyfesh: the store checklist

What each store needs before Jellyfesh Pro can be sold there (ADR-0021 §7).
The packaging and signing are ready in the repository and run in
`release.yml` as soon as their secrets exist ([`docs/github.md`](github.md),
"Secrets and variables"); everything below is for the owner to do, in the
stores' own consoles. Facts were checked on 2026-11-17 and 2026-11-18;
consoles change their menus often, so follow the words, not the exact path.

## Before any store

* **The legal documents.** [`legal/`](../legal/README.md) holds drafts of the
  End User Licence Agreement, the Privacy Policy and the Terms of Sale. A
  qualified lawyer must review them and fill in the blanks (the seller's
  name and address, governing law, refund policy, contact address) before
  anything is sold. Their pages on the website are what the stores and the
  apps link to: `<website>/legal/privacy.html`, `<website>/legal/eula.html`
  and `<website>/legal/terms-of-sale.html`.
* **The seller.** The person or company that sells, with its address, tax
  registrations (VAT or GST where you sell) and a bank account for payouts;
  organisations need a D-U-N-S number for Apple and Google.
* **Prices** are set in each store and in the licence-key shop, never in the
  code. Pro is one non-consumable purchase: product id `org.jellyfesh.pro`
  (Google Play: `pro`).
* **Builds that sell Pro are built without Pro Preview**: run the release
  workflow with **Pro Preview** unticked. A Pro Preview build treats Pro as
  active and never offers to buy it.
* **Licence keys.** `tools/jf-licence` (ADR-0021; it comes with the
  `core/jf-licence` crate, which is not in the tree yet) makes the key pair:
  `keygen --key-id 1 --out <a path outside the repository>`. Keep the
  private key in a password manager and put the printed `id:hex` line in
  the repository variable `JF_LICENCE_PUBLIC_KEYS`. Keys are sold through a
  shop of your choice (a merchant of record handles VAT for you); issue each
  with `jf-licence issue` and keep it with its order, so a lost key can be
  sent again.
* **Store screenshots** show the real app. The test library and mock servers
  in `docs/screenshots/` are invented: they may appear only where it is
  clear they are sample music, and never with another service's logo or
  catalogue unless that service allows it.

## Questions to settle before the first submission

* **Licence keys in store builds.** App Review guideline 3.1.1 names licence
  keys among the mechanisms an app may not use to unlock features, and
  2.4.5(vi) says Mac App Store apps may not require them; 3.1.3(b) lets an
  app open features bought elsewhere only when they are also sold in the
  app. The safe course for the Mac App Store build is to hide **Enter
  Licence Key** (`JF_DISTRIBUTION_APP_STORE`). Google Play's payments policy
  must be read for the same question (it could not be fetched from here).
  The Microsoft Store's policy 10.8 lets apps that are not games use their
  own commerce, so keys can stay there; check the current wording.
* **Plugins.** Guideline 2.5.2 forbids downloading or installing code that
  changes an app's features; for Mac apps 2.4.5(iv) forbids downloading
  additional code to add functionality; 4.7 allows plug-ins not embedded in
  the binary, with an index of what is offered, filtering, reporting and age
  limits. Jellyfesh's official plugins are signed and bundled, but **Install
  from File…** installs any plugin package. Decide, before submitting,
  whether store builds keep Install from File… (and explain it in the review
  notes) or hide it. Google Play's Device and Network Abuse policy forbids
  downloading executable code except code run by a virtual machine or
  interpreter with indirect access to Android APIs; the plugins are
  WebAssembly in the app's own sandbox, which is the argument to make, but
  read the current policy first.
* **Comments from services** (ADR-0022) are other people's words: Apple's
  guideline 1.2 asks apps with user-generated content for filtering,
  reporting and blocking. Check whether read-only comments from a service
  fall under it.
* **The services' permission.** Guidelines 5.2.2 and 5.2.3 ask for
  permission from services whose content an app streams; keep each
  official service's developer terms and approvals ready for App Review.

## Apple: the Mac App Store and the direct download

**Accounts and agreements**

* The Apple Developer Program (organisations: D-U-N-S number).
* App Store Connect → Business: the Paid Apps agreement, tax forms and bank
  account; in-app purchases cannot be sold without it.
* Certificates (Certificates, Identifiers & Profiles), each exported from
  Keychain Access with its private key as a .p12:
  * **Developer ID Application**: the direct download
    (`APPLE_DEVELOPER_ID_P12`, `APPLE_DEVELOPER_ID_PASSWORD`, `APPLE_TEAM_ID`);
  * **Apple Distribution** and **Mac Installer Distribution**: the Mac App
    Store app and package (`APPLE_MAS_APP_P12`, `APPLE_MAS_INSTALLER_P12`,
    `APPLE_MAS_PASSWORD`).
* The App ID `org.jellyfesh.Jellyfesh` (In-App Purchase is on for every App
  ID; add the MusicKit App Service for Apple Music), and a **Mac App Store**
  provisioning profile for it (`APPLE_MAS_PROFILE`).
* An App Store Connect API key with the Developer role, for notarization
  (`APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`, `APPLE_NOTARY_KEY`).

**The product**

* App Store Connect → the app → In-App Purchases: a **Non-Consumable** with
  Product ID `org.jellyfesh.pro`, reference name "Jellyfesh Pro", its price,
  the display name and description in English, Simplified Chinese,
  Traditional Chinese and Japanese, and a review screenshot of the Pro
  sheet. Decide whether Family Sharing applies.
* Restore Purchases must stay in the app (non-consumables).
* Local tests: `platforms/macos/Jellyfesh/Jellyfesh.storekit` holds
  `org.jellyfesh.pro` for StoreKit testing when the app runs from Xcode
  (the scheme's run action uses it). Its price is a test value, not the
  price.

**App Privacy** (App Store Connect → App Privacy): **Data Not Collected**,
matching the Privacy Policy. Apple counts as collected only data that
leaves the device so that the developer or its partners (the SDKs and
vendors whose code is in the app) can access it beyond serving the request.
Jellyfesh has no analytics, advertising or crash-reporting code; what it
sends goes to the servers and services the person sets up, and the audio of
AI Lyrics' online mode goes, after consent, to the provider the person
chose, with their own key. If the lawyer reads this differently, the
fallback answer is "Audio Data", not linked to the person and not used for
tracking, for App Functionality. Privacy policy URL:
`<website>/legal/privacy.html`.

**Export compliance.** Jellyfesh's core does not use only the operating
system's encryption: it implements TLS with rustls and ring, and encrypts
the plugins' cookie jars with AES-GCM. In App Store Connect's questionnaire
that is "standard encryption algorithms instead of, or in addition to, using
or accessing the encryption within Apple's operating system", which needs
the French encryption declaration when the app is sold in France (or France
left out of its countries). That is why `Info.plist` does not claim
`ITSAppUsesNonExemptEncryption` = NO: App Store Connect asks at each upload
until the answer is settled, after which `ITSAppUsesNonExemptEncryption`
(YES) and `ITSEncryptionExportComplianceCode` can go into the Info.plist.
Ask the lawyer whether a U.S. export classification or report is needed
too.

**Age rating.** Answer App Store Connect's questionnaire for what people can
reach: their own music and the catalogues of the services they set up
(lyrics and song titles can be explicit), and, from v0.0.7, comments from
those services.

**Screenshots.** At least one, up to ten, 16:10: 1280×800, 1440×900,
2560×1600 or 2880×1800.

**Build and upload.** Run the release workflow with the Apple secrets set;
the run's `store-macos` artifact holds
`Jellyfesh-<version>-Mac-App-Store.pkg`, signed by
`platforms/macos/scripts/package-mas.sh` with
`Jellyfesh-AppStore.entitlements`: the sandbox; network client (sources, AI
Lyrics, model downloads); network server (the loopback port that receives a
browser sign-in); user-selected files, read and write (music folders,
Install from File…, playlist export) with app-scoped bookmarks to keep
access; the Music folder, read only; and JIT with unsigned executable memory
for the WebAssembly plugin runtime. Upload it with Transporter, or with
`xcrun altool --upload-package` and the same API key. Before the first
submission, the macOS app must keep access to the folders people add across
launches with security-scoped bookmarks (today it passes the paths straight
to the core, which only works outside the sandbox), and add
`NSLocalNetworkUsageDescription` if servers on the local network should
work without surprises.

**Review notes** (App Store Connect → the version → App Review Information),
a starting point:

> Jellyfesh plays the music in folders the reviewer chooses and from
> servers and services the person sets up (Jellyfin, Subsonic, Spotify
> remote control, QQ Music and NetEase Cloud Music through their official
> APIs). Jellyfesh Pro (AI Lyrics and Customize) is the in-app purchase
> org.jellyfesh.pro. AI Lyrics works on the device by default: choose
> Download for the Tiny model in Settings › Extensions › AI Lyrics, then
> Create Synced Lyrics on a song without lyrics. Its online mode sends the
> song's audio to a speech recognition provider only with the person's own
> API key and only after they allow it in a dialog that names the provider.
> Plugins run in a WebAssembly sandbox; the official ones are signed and
> included in the app. [Describe the decision on Install from File… and
> licence keys here.] [Demo server or account details, if the reviewer
> should try a source.]

**The direct download.** With the Developer ID and notary secrets, the
release's `Jellyfesh-<version>-macOS.dmg` is signed (hardened runtime,
secure timestamp), notarized and stapled, so it opens without the Open
Anyway step. Whether a Developer ID build may use MusicKit is still open
(docs/macos.md).

## Microsoft Store

**Accounts and agreements**

* A Partner Center developer account (check the current fee for individuals
  and companies), with the payout and tax profiles that paid add-ons need.
* Reserve the name "Jellyfesh". Product identity (the app → Product
  identity) gives `MS_STORE_IDENTITY_NAME`, `MS_STORE_PUBLISHER` and
  `MS_STORE_PUBLISHER_NAME`.

**The product**

* An add-on of type **Durable** with Product ID `org.jellyfesh.pro` (if
  Partner Center refuses the dots, use `pro` and tell whoever writes the
  Windows store code), lifetime "Forever", its price and its listing in the
  four languages.
* Test it with the add-on hidden from the public (private audience or
  package flights) before the release.

**Privacy and ratings**

* Partner Center asks whether the app accesses, collects or transmits
  personal information: yes, because it sends people's sign-ins to the
  services they set up, so give the privacy policy URL,
  `<website>/legal/privacy.html`.
* The age rating comes from the IARC questionnaire (same answers as for
  Apple).
* The restricted capability `runFullTrust` needs a sentence: Jellyfesh is a
  WinUI 3 desktop app with the Windows App SDK.
* Licence agreement: the Store applies its Standard Application License
  Terms unless you give your own; decide with the lawyer.

**Screenshots.** At least one desktop screenshot, 1366×768 or larger, up to
ten.

**Build and upload.** With the three identity secrets set, the run's
`store-windows` artifact holds `Jellyfesh-<version>-Microsoft-Store.msixbundle`
(x64 and Arm64, made by `platforms/windows/scripts/build.ps1 -StorePackage`
with the Partner Center identity and a package version `X.Y.Z.0`). Upload
it in the submission's Packages page; it is unsigned, and the Store signs
it.

## Google Play

**Accounts and agreements**

* A Play Console developer account (one-time registration fee, identity
  verification; organisations need a D-U-N-S number) and a payments
  profile, which paid in-app products need.
* **Play App Signing.** Give Play Jellyfesh's existing Android release key
  as the app signing key (Play Console → the app → App integrity → use your
  own key, exported with Google's PEPK tool), so the APK on the release page
  and the app from Google Play have the same signature and update each
  other. The release workflow signs the App Bundle with the same keystore,
  which then serves as the upload key; Play also accepts a separate upload
  key. If Play generates the app signing key instead, the two builds can
  never update each other.

**The product**

* Monetize with Play → Products → One-time products: product ID `pro`, its
  price and its title and description in the four languages.
* Monetize with Play → Monetization setup → Licensing: the app's licence
  key (a base64 public key) goes into the secret or variable
  `PLAY_LICENSE_KEY`; the app checks purchase signatures with it
  (`BuildConfig.PLAY_LICENSE_KEY`). Play Billing Library 9.1.0 is in the
  build (Play requires 8.0.0 or later for updates).
* Test with licence testers (Settings → License testing) on the internal
  testing track.

**Data safety** (App content → Data safety), matching the Privacy Policy:
no data collected and no data shared, because nothing goes to the developer
or to third parties of the developer's choosing: requests go to the
servers and services the person sets up, and the AI Lyrics audio to the
provider they chose, with their own key, after a prominent consent dialog.
Google's own wording (support.google.com could not be read from here)
treats data transmitted off the device by the app as collected, with
exceptions for user-initiated transfers and ephemeral processing; if the
lawyer reads the AI Lyrics upload as collection, declare "Audio: music
files", collected, processed ephemerally, optional, for app functionality.
Encryption in transit: yes, except for a server the person adds with an
`http://` address.

**App content.** The privacy policy URL; ads: none; the IARC content
rating; the target audience (not designed for children); the foreground
service declaration for media playback (`FOREGROUND_SERVICE_MEDIA_PLAYBACK`);
the permission `READ_MEDIA_AUDIO` as the core function of a music player.

**Technical checks.** Play requires native libraries that support 16 KB
memory pages for apps that target Android 15 or later: check the App
Bundle's `libjf_ffi.so`, `libc++_shared.so` and JNA's `libjnidispatch.so`
(Android Studio's APK Analyzer, or `zipalign -c -P 16 -v 4` on a built
APK) before the first upload.

**Graphics.** Phone screenshots (two to eight), a 512×512 icon and a
1024×500 feature graphic.

**Build and upload.** With the Android secrets set, the run's
`store-android` artifact holds `Jellyfesh-<version>-Google-Play.aab`, built
with the distribution `google-play` and signed with jarsigner. Upload it to
the internal testing track first.

## Linux, Jellyfesh Web and the licence-key shop

No store sells Pro there: on Linux and in Jellyfesh Web, Pro comes only
with a licence key. The Terms of Sale say so, and so does each release's
text. The direct downloads of the other apps take licence keys as well.

## Which secrets for which store

| Store | Secrets and variables ([`docs/github.md`](github.md)) |
| --- | --- |
| Mac App Store | `APPLE_MAS_APP_P12`, `APPLE_MAS_INSTALLER_P12`, `APPLE_MAS_PASSWORD`, `APPLE_MAS_PROFILE`, `APPLE_TEAM_ID` |
| macOS direct download | `APPLE_DEVELOPER_ID_P12`, `APPLE_DEVELOPER_ID_PASSWORD`, `APPLE_TEAM_ID`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID`, `APPLE_NOTARY_KEY` |
| Microsoft Store | `MS_STORE_IDENTITY_NAME`, `MS_STORE_PUBLISHER`, `MS_STORE_PUBLISHER_NAME` |
| Windows direct download | `WINDOWS_SIGNING_PFX`, `WINDOWS_SIGNING_PASSWORD` (see the note on hardware keys in docs/github.md) |
| Google Play and the APK | `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`, `PLAY_LICENSE_KEY` |
| Licence keys (every app) | the variable `JF_LICENCE_PUBLIC_KEYS` |
