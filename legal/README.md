# Legal documents

**Status: drafts, not in force.** These documents were written for v0.0.7
(ADR-0021 §6) from what the code and the design records say Jellyfesh
does. They have not been reviewed by a lawyer and are not legal advice.
Before Jellyfesh is sold, a qualified lawyer in the seller's country must
review them, fill in the blanks and answer the questions below. Each
document says so at the top.

| File | Document | On the website |
| --- | --- | --- |
| [`EULA.md`](EULA.md) | End User Licence Agreement | `legal/eula.html` |
| [`PRIVACY.md`](PRIVACY.md) | Privacy Policy | `legal/privacy.html` |
| [`TERMS-OF-SALE.md`](TERMS-OF-SALE.md) | Terms of Sale for Jellyfesh Pro | `legal/terms-of-sale.html` |

## Where they appear

* **Website:** `website/legal/` holds the three documents and an index
  page in the site's style, linked as **Legal** from every page's footer.
  The pages are generated from these files: edit the Markdown here, then
  run `python3 tools/release/legal_pages.py`. CI runs it with `--check` and
  fails when the pages differ from the Markdown. While a document still
  says it is a draft, its page asks search engines not to index it.
* **Apps:** About and the Pro sheet link **Licence Agreement**, **Privacy
  Policy** and **Terms of Sale** (docs/ui.md). Link to the website pages:
  `<website>/legal/eula.html`, `<website>/legal/privacy.html` and
  `<website>/legal/terms-of-sale.html`.
* **Stores:** each store asks for a privacy policy address, and the App
  Store and Microsoft Store can show a licence agreement
  ([`docs/store-release.md`](../docs/store-release.md)).

The documents use only the Markdown the site generator understands:
headings, paragraphs, bullet lists, tables, bold, code and links (no
numbered lists, italics or quotes). Links between them use the file names
(`EULA.md`), which the generator turns into the pages' names.

## Blanks

| Blank | Meaning |
| --- | --- |
| [Seller legal name], [Address] | Who makes and sells Jellyfesh, and their postal address (Apple requires both in a custom licence agreement) |
| [Contact email] | Where people write about Jellyfesh, purchases and privacy |
| [Effective date] | When each document takes effect |
| [Governing law], [Courts] | The law that governs and where disputes go |
| [Liability cap] | The minimum cap on liability (EULA §14) |
| [Licence key shop] | Where licence keys are sold, and whether it is the merchant of record |
| [Refund policy] | The refund promise for licence keys beyond the law (Terms of Sale §6) |
| [Email provider] | Who hosts the contact email |
| [Retention period for orders], [Retention period for messages] | How long order records and messages are kept |
| [Supervisory authority] | The data protection authority for the seller |
| [Minimum age] | The age Jellyfesh is directed at (Privacy §12) |
| Privacy §5, the order details | What the shop actually passes on |
| Privacy §7, transfers | Transfers outside the UK and the EEA and their safeguards |

## Facts the documents rely on

Check these again when the v0.0.7 features are finished, and whenever
Jellyfesh changes what leaves the device:

* **No account, analytics, advertising or tracking:** no analytics or
  crash-reporting code in the apps or the core; the README and the
  website's privacy page say the same.
* **Plugins:** each reaches only the servers in its manifest (ADR-0003,
  ADR-0004); sign-ins in the system keychain and cookie jars encrypted with
  a key kept there (ADR-0015, `core/jf-plugin-host/src/cookies.rs`); no
  stored passwords.
* **AI Lyrics** (ADR-0019): on-device by default; models from
  `huggingface.co` only on request; the online mode sends audio (16 kHz
  mono WAV, at most 8 minutes per request) to OpenAI, Groq or a custom
  server with the person's own key, only after consent per plugin and
  origin. **Check what the finished plugin sends besides the audio**
  (language, a prompt with the lyrics, song details) and correct Privacy
  §4 if needed.
* **Update check** (ADR-0021 §5): GitHub's releases API, at most once a
  day, direct downloads only. Check its request headers when it exists.
* **Licence keys** (ADR-0021 §2): no name or email in the key; checked
  offline; withdrawn keys listed in updates.
* **Editions** (ADR-0021 §4): app store copies and release builds do not
  include the unofficial community plugins.
* **Website:** GitHub Pages, no cookies, the language choice in local
  storage (`website/assets/site.js`). **Jellyfesh Web:** docs/web.md,
  "Privacy and storage".

## Decisions taken from the design, for the owner to confirm

* Pro includes the Pro features of later versions at no extra cost
  (ADR-0021 §1).
* A licence key is personal and may be used on any devices the person owns
  or controls: keys carry no name and are checked offline, so no device
  limit could be enforced anyway.
* Lost keys are sent again on request with the order number (the shop
  must keep each key with its order), and keys are delivered by email.
* Updates "for as long as we develop it" (Terms of Sale §8).
* The source code is public, but the licence agreement grants no right to
  reuse it (ADR-0016; EULA §4).
* The licence covers use "for your own use"; say so if business use should
  be allowed or excluded.

## Questions for the lawyer

* **App Store licence agreement.** Use Apple's standard licence agreement
  or this one. A custom agreement must include Apple's minimum terms; EULA
  §17 follows them and must be checked against Apple's current text. The
  Microsoft Store applies its Standard Application License Terms unless we
  provide our own.
* **Licence keys in app store copies.** App Review guideline 3.1.1 names
  licence keys among the mechanisms an app may not use to unlock features,
  3.1.3(b) allows access to features bought elsewhere when they are also
  sold in the app, and 2.4.5(vi) says Mac apps may not require licence
  keys. Terms of Sale §3 is written so that store copies need not accept
  keys. The Google Play payments policy must be checked too
  (docs/store-release.md).
* **Selling licence keys:** merchant of record, VAT and sales tax,
  invoices, and the consent to immediate supply that ends the EU and UK
  right to cancel (Terms of Sale §6). Some countries require more: for
  example, selling online to Japan requires a disclosure under the Act on
  Specified Commercial Transactions.
* **Consumer law** in the countries where Jellyfesh is sold (its apps
  speak English, Chinese and Japanese): the disclaimers and the liability
  cap (EULA §13 to §15), conformity and the duty to supply updates for
  digital content (EU Directive 2019/770), and ending a paid licence for a
  breach (EULA §12).
* **Withdrawing licence keys** after refunds, chargebacks or publication
  (Terms of Sale §5).
* **Community plugins.** Release pages offer the unofficial NetEase Cloud
  Music and QQ Music plugins, made in this repository, as separate
  downloads. They use the services' private web interfaces and may be
  against their terms (ADR-0012). Should a seller distribute them, and is
  EULA §6 enough?
* **AI Lyrics:** lyrics are copyright works; the made lyrics are labelled
  as made by AI and kept for private listening. Are EULA §7 and Terms of
  Sale §7 enough, including any labelling duties for machine-made content?
* **Privacy roles:** we treat ourselves as controller only for orders and
  messages, not for data kept on the device or sent by the person to the
  services and providers they choose. Children (Privacy §12), California,
  and the privacy laws of the countries where Jellyfesh is sold.
* **Export controls:** the core implements TLS with rustls and ring and
  uses AES-GCM, so it is not limited to the operating system's encryption
  (docs/store-release.md, "Export compliance"). Classification and any
  reports.
* **Language:** the documents are in English only; some countries require
  consumer terms in their language.
