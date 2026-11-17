# Jellyfesh Privacy Policy

**Draft for review by a qualified lawyer before Jellyfesh is sold.** This
text is not legal advice and is not in force. Words in square brackets,
such as [Contact email], are blanks to be filled in.

Last updated: [Effective date]

## 1. In short

* Jellyfesh has no account, no analytics, no advertising and no tracking.
  The apps send nothing about you, your library or how you use them to us.
* Your library, playlists, play history, settings and lyrics are kept on
  your device.
* Information leaves your device only when you use something that needs
  it: a source you set up, AI Lyrics in its online mode, a speech model
  download, the update check of copies downloaded from our website, or a
  purchase. Each is described below.
* We hold personal information only if you buy a licence key from us or
  write to us.

## 2. Who we are

This policy is made by [Seller legal name] of [Address] ("we", "us"), who
makes Jellyfesh. For the information we hold ourselves (section 7), we are
the data controller. You can reach us at [Contact email].

It covers the Jellyfesh apps for macOS, Windows, Linux and Android,
Jellyfesh Web, the `jf` command-line player, the official plugins and the
Jellyfesh website.

## 3. What stays on your device

Jellyfesh keeps these on your device, and does not send them to us:

* **Your library:** the folders you add, the songs' file locations and
  tags, artwork, playlists, favourites and play history.
* **Settings:** your choices in Settings, the sources and plugins you have
  installed and their settings (such as a server address), the providers
  you have allowed AI Lyrics to send audio to, and your licence key if you
  enter one.
* **Sign-ins:** tokens, cookies and keys that let a plugin stay signed in
  to a service, and the API key you give AI Lyrics. Jellyfesh keeps these
  in your system's keychain: the Keychain on macOS, Credential Manager on
  Windows, the Secret Service on Linux and storage encrypted with the
  Android Keystore on Android. A plugin's cookies are kept in a file that
  is encrypted with a key held in the keychain. Jellyfesh never stores
  your passwords: where a service signs in with a password, it is
  exchanged once for a token.
* **Lyrics made by AI Lyrics** and the speech models you download.

Uninstalling a source in Jellyfesh deletes its settings and sign-in, and
removing a folder takes its songs out of the library (the files stay where
they are). Removing the Jellyfesh app itself can leave its data folder and
keychain entries on your device, which you can delete. Backups you make of
your device, and Android's own backup if it is turned on, can include
Jellyfesh's data; they are handled by you or your backup provider.

## 4. What leaves your device, and when

**Sources you set up.** Online sources work through plugins. When you use
a source, its plugin contacts the server or service you set up, directly
from your device: for example your own Jellyfin, Navidrome or other
Subsonic server, or QQ Music, Spotify or NetEase Cloud Music with your own
account, key or credentials. The plugin sends what the service needs to do
what you asked: your sign-in, what you search for, browse or play, and, for
Spotify, the commands for the device you control. Each plugin can only
contact the servers its package names. That service's own terms and privacy
policy apply to what it receives. We do not receive any of it.

**Community plugins** are made by others or are unofficial. They contact
the services they name in the same way, within the same limits, but we do
not control what they send. They stay off until you allow community plugins.

**AI Lyrics, online mode.** Only if you choose the online mode, and only
after you allow it for a provider, AI Lyrics sends the audio of the song
you are making lyrics for (as 16 kHz mono sound, in parts of up to eight
minutes) to the speech recognition provider you chose, such as OpenAI,
Groq or a server you name, with your own API key. The request can also
include hints that help recognition, such as the song's language or the
lyrics you are timing. The provider's terms and privacy policy apply, and
the provider may charge your account. You can stop allowing a provider at
any time in Settings, under Extensions, AI Lyrics, Sending Audio.

**AI Lyrics, on this device.** In this mode, which is the default, the
song's audio does not leave your device. The speech models it uses are
downloaded from Hugging Face (huggingface.co) only when you choose to
download one; Hugging Face receives the request, including your IP
address, as for any download.

**Update check.** Copies of Jellyfesh downloaded from our website or our
releases page ask GitHub (api.github.com) whether a newer version exists,
at most once a day while **Check for Updates Automatically** is on, and
when you choose **Check for Updates**. The request carries nothing that
identifies you beyond what any internet request carries, such as your IP
address. Copies from an app store and Jellyfesh Web do not do this.

**Links you open.** **Open in** a service, and links in Jellyfesh, open the
service's app or your browser, which then work under their own terms.

## 5. Purchases and licence keys

**In an app store.** If you buy Jellyfesh Pro in Apple's App Store, the
Microsoft Store or Google Play, the store handles the purchase and your
payment under its own terms and privacy policy. Jellyfesh asks the store,
on your device, whether you own Jellyfesh Pro. We do not receive your
payment details. The stores give us reports about sales, which do not
identify you by name.

**Licence keys.** If you buy a licence key from [Licence key shop], the
shop processes your payment. We receive [the order details: your name,
email address, country, the product, the price and the order number] to
deliver your key, to help you if you lose it, and to keep the records that
tax law requires.

A licence key contains a random licence number, the product, the day it
was issued and a signature. It does not contain your name or email
address. Jellyfesh checks licence keys on your device; it does not contact
us or anyone else to activate or check a key.

## 6. What we do not do

We do not run analytics, crash reporting or advertising in Jellyfesh, we
do not track you across apps or websites, and we do not sell or share
personal information. App stores and operating systems may collect
information under their own policies, such as crash reports and usage
statistics you agreed to share with developers, and give us summaries of
it; we use those only to improve Jellyfesh.

## 7. Information we hold, and why

We hold personal information only in these cases:

* **Licence key orders** (section 5): to deliver and support your key,
  because it is needed for our contract with you, and to keep accounting
  records, because the law requires it.
* **Messages you send us**, by email or through our issue tracker: to
  answer them, because it is in our legitimate interest to help you, or
  because it is needed for our contract with you. Issues on GitHub are
  public and are also governed by GitHub's privacy statement.

We share this information only with the service providers who help us
provide it ([Licence key shop], [Email provider]), with professional
advisers, and with authorities when the law requires it. [Describe any
transfers outside the United Kingdom or the European Economic Area and
their safeguards.]

We keep order records for [Retention period for orders] and messages for
[Retention period for messages], then delete them.

## 8. Your rights

Depending on where you live, you may have the right to ask us for a copy of
the personal information we hold about you, to have it corrected or
deleted, to restrict or object to how we use it, and to receive it in a
portable form. Write to [Contact email]. You may also complain to a data
protection authority, such as [Supervisory authority].

We do not sell or share personal information, as those words are used in
California law, and we do not use sensitive personal information.

The information Jellyfesh keeps on your device is under your control: you
can change or delete it in Jellyfesh, or delete the app and its data.

## 9. Jellyfesh Web

Jellyfesh Web runs in your browser. The website it comes from sends the
app's files and receives nothing back from the app. Jellyfesh Web talks
only to the servers you sign in to. It keeps your settings, library and
plugins in your browser's storage, with sign-ins encrypted using a key the
browser keeps; it uses no cookies. Clearing the site's data in your
browser removes all of it. If you host Jellyfesh Web yourself, your own
server's practices apply to it.

## 10. Our website

The Jellyfesh website is hosted on GitHub Pages. It uses no cookies, no
analytics and no third-party scripts or fonts. It remembers the language
you choose in your browser's local storage. GitHub, which hosts the site,
may log requests, including IP addresses, under GitHub's privacy statement.

## 11. Security

Connections to online services use HTTPS. A server you add yourself is
reached at the address you give: if that address starts with `http://`,
the connection is not encrypted. Plugins run in a sandbox that limits what
they can do and which servers they can contact. Sign-ins are kept in your
system's keychain. No system is perfectly secure; keep your devices up to
date.

## 12. Children

Jellyfesh is not directed at children under [Minimum age], and we do not
knowingly collect personal information from them.

## 13. Changes to this policy

When Jellyfesh changes what information leaves your device, we will update
this policy before the change is released and say what changed in the
release notes.

## 14. Contact

[Seller legal name], [Address]. Email: [Contact email].
