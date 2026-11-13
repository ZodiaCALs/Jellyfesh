# ADR-0015: Secrets and credentials

* Status: Proposed
* Date: 2026-11-05

## Context

Plugins need tokens, cookies and API keys. The brief requires OS keystores
and forbids storing passwords.

**Verified (crates.io and crate READMEs, 2026-11-05):**

* `keyring` **4.2.0** (2026-08-29, MIT/Apache, MSRV 1.88) wraps
  `keyring-core` 1.0.1 with per-platform stores: macOS Keychain, Windows
  Credential Manager, Linux Secret Service (zbus) by default; optional
  keyutils.
* `android-native-keyring-store` 1.0.0 (2026-04-21) keeps secrets in
  SharedPreferences encrypted with Android Keystore keys and reaches them
  over JNI from Rust; Kotlin only initialises `ndk-context` (which cpal
  also needs).

## Decision

1. `jf-core` defines `SecretStore` (`get`, `set`, `delete`, namespaced keys).
   Desktop uses `keyring` 4.x. Android uses `android-native-keyring-store`
   unless spike S11 finds a problem, in which case the Kotlin shell
   implements `SecretStore` as a UniFFI foreign trait over Keystore.
2. Keys are `jellyfesh/<plugin-id>/<name>`. One master entry per plugin
   holds a random 256-bit key that encrypts the plugin's cookie jar file
   (AES-GCM), so a jar never sits in the clear on disk.
3. **No stored passwords.** Password flows are exchange-only (ADR-0004).
   Subsonic is the hard case: classic token auth needs the password on every
   request (`md5(password + salt)`). **Decision (2026-11-05):** use the
   OpenSubsonic API key when the server offers `apiKeyAuthentication`;
   otherwise store one salt + token pair in the keystore (never the
   password), tell the person it works like a password for that server, and
   switch to an API key automatically once the server supports it.
4. Linux without a Secret Service provider (some minimal desktops): the app
   says so and offers session-only login rather than writing secrets in the
   clear.
5. Logout deletes the keystore entries and the cookie jar, then calls the
   plugin's `logout` so it can revoke server-side where the service allows.

## Consequences

* One Rust code path on all four platforms if the Android store holds up.
* Secret Service availability is a support issue on Linux, handled by an
  explicit message rather than a silent downgrade.
