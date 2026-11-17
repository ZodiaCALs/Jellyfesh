#!/usr/bin/env python3
"""Writes the website's legal pages from the documents in legal/:

    python3 tools/release/legal_pages.py           # writes website/legal/*.html
    python3 tools/release/legal_pages.py --check   # exits 1 when they differ (CI)

legal/EULA.md, PRIVACY.md and TERMS-OF-SALE.md become eula.html, privacy.html
and terms-of-sale.html in the site's style (the header, footer and stylesheet
of the other pages), and index.html lists them. The Markdown is converted by
tools/release/build_site.py (headings, paragraphs, bullet lists, tables, bold,
code and links); links between the documents use their file names and become
the pages' names. While a document says it is a draft for review, its first
paragraph is shown as a note and its page asks search engines not to index it.
"""
import html
import os
import re
import sys

from build_site import markdown

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SOURCE = os.path.join(ROOT, "legal")
OUT = os.path.join(ROOT, "website", "legal")
DRAFT = "**Draft for review by a qualified lawyer"

# (Markdown file, page, short name, what it covers)
DOCS = [
    ("EULA.md", "eula.html", "Licence Agreement",
     "What you may do with Jellyfesh, plugins, AI Lyrics, warranty and liability."),
    ("PRIVACY.md", "privacy.html", "Privacy Policy",
     "What Jellyfesh keeps on your device, what leaves it and when, and your rights."),
    ("TERMS-OF-SALE.md", "terms-of-sale.html", "Terms of Sale",
     "Buying Jellyfesh Pro in an app store or with a licence key, Pro Preview and refunds."),
]
PAGES = {source: page for source, page, _, _ in DOCS}

STYLE = (".prose h3{margin:32px 0 8px;font-size:19px;font-weight:600}"
         ".prose .card{margin:20px 0 28px}.prose .card p{margin:0}"
         ".prose ul{padding-left:1.25em}.prose li{margin:4px 0}"
         ".legal-list{list-style:none;padding:0;display:grid;gap:12px}"
         ".legal-list a{font-weight:600}")


def page(title, description, body, draft, current):
    robots = '\n  <meta name="robots" content="noindex">' if draft else ""
    legal_current = ' aria-current="page"' if current else ""
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)} · Jellyfesh</title>
  <meta name="description" content="{html.escape(description)}">{robots}
  <link rel="icon" href="../assets/icon-ice.svg" type="image/svg+xml">
  <link rel="stylesheet" href="../assets/site.css">
  <style>{STYLE}</style>
</head>
<body>
  <!-- Generated from legal/ by tools/release/legal_pages.py: edit the Markdown there, then run the script. -->
  <header class="top">
    <div class="wrap">
      <a class="brand" href="../index.html"><img src="../assets/icon-ice.svg" alt=""><span>Jellyfesh</span></a>
      <nav aria-label="Site">
        <a href="../index.html" data-i18n="nav.home">Home</a>
        <a href="../download.html" data-i18n="nav.download">Download</a>
        <a href="../privacy.html" data-i18n="nav.privacy">Privacy</a>
        <select class="lang" aria-label="Language"></select>
      </nav>
    </div>
  </header>
  <main>
    <section>
      <div class="wrap prose" lang="en">
{body}
      </div>
    </section>
  </main>
  <footer>
    <div class="wrap">
      <img class="wm-light" src="../assets/wordmark-ink.svg" alt="JELLYFESH">
      <img class="wm-dark" src="../assets/wordmark-white.svg" alt="JELLYFESH">
      <span data-i18n="footer.rights">© 2026 Jellyfesh. All rights reserved.</span>
      <nav aria-label="Legal">
        <a href="../privacy.html" data-i18n="nav.privacy">Privacy</a>
        <a href="index.html" data-i18n="footer.legal"{legal_current}>Legal</a>
        <a href="../THIRD_PARTY_NOTICES.html" data-i18n="footer.notices">Third-party notices</a>
      </nav>
    </div>
  </footer>
  <script src="../assets/site.js"></script>
</body>
</html>
"""


def document(source):
    """The page body of one document, its title and whether it is a draft."""
    with open(os.path.join(SOURCE, source), encoding="utf-8") as fh:
        text = fh.read()
    title = re.search(r"^# (.+)$", text, re.M)
    if not title:
        sys.exit(f"legal/{source}: no '# Title' line")
    draft = DRAFT in text
    body = markdown(text)

    def link(match):
        target = html.unescape(match.group(1))
        if target.endswith(".md") and "://" not in target:
            if target not in PAGES:
                sys.exit(f"legal/{source}: links to {target}, which has no page")
            return f'href="{PAGES[target]}"'
        return match.group(0)

    body = re.sub(r'href="([^"]*)"', link, body)
    if draft:
        # The draft notice, the first paragraph after the title, as a note.
        body = re.sub(r"(<p><strong>Draft for review.*?</p>)", r'<div class="card" role="note">\1</div>',
                      body, count=1, flags=re.S)
    return title.group(1).strip(), body, draft


def pages():
    """Every page of website/legal/ by file name."""
    out = {}
    items, any_draft = [], False
    for source, name, short, about in DOCS:
        title, body, draft = document(source)
        any_draft |= draft
        out[name] = page(title, about, body, draft, current=False)
        items.append(f'  <li><a href="{name}">{html.escape(title)}</a><br>{html.escape(about)}</li>')
    note = ""
    if any_draft:
        note = ('<div class="card" role="note"><p><strong>Drafts for review by a qualified lawyer before '
                'Jellyfesh is sold.</strong> These documents are not in force yet; words in square '
                'brackets are blanks still to be filled in.</p></div>\n')
    body = ("<h2>Legal</h2>\n" + note +
            '<ul class="legal-list">\n' + "\n".join(items) + "\n</ul>")
    out["index.html"] = page("Legal", "Jellyfesh's licence agreement, privacy policy and terms of sale.",
                             body, any_draft, current=True)
    return out


def main():
    check = "--check" in sys.argv[1:]
    wanted = pages()
    stale = []
    for name, text in wanted.items():
        path = os.path.join(OUT, name)
        current = None
        if os.path.exists(path):
            with open(path, encoding="utf-8") as fh:
                current = fh.read()
        if current == text:
            continue
        stale.append(name)
        if not check:
            os.makedirs(OUT, exist_ok=True)
            with open(path, "w", encoding="utf-8", newline="\n") as fh:
                fh.write(text)
    extra = sorted(set(os.listdir(OUT)) - set(wanted)) if os.path.isdir(OUT) else []
    if check:
        if stale or extra:
            print("website/legal/ does not match legal/: run python3 tools/release/legal_pages.py "
                  f"(differs: {', '.join(stale) or 'none'}; not generated: {', '.join(extra) or 'none'})")
            sys.exit(1)
        print(f"website/legal/ matches legal/ ({len(wanted)} pages)")
    else:
        if extra:
            print(f"note: website/legal/ also has {', '.join(extra)}, which this script does not write")
        print(f"website/legal/: {len(wanted)} pages, {len(stale)} written")


if __name__ == "__main__":
    main()
