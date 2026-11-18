#!/usr/bin/env python3
"""Builds the website for a release into dist/site/.

    python3 tools/release/build_site.py VERSION [--base-url URL] [--app PATH]
                                        [--releases FILE]
                                        [--release-assets FILE --sums FILE]

* copies website/ and, from dist/, every release file of VERSION into
  site/downloads/ (or links to --base-url, e.g. the GitHub release page's
  download URL, instead of copying). A release has one file per platform:
  Jellyfesh-<version>-macOS.dmg, -Windows-Setup.exe, -Linux.flatpak,
  -Android.apk, -Web.zip and -Community-Plugins.zip;
* with --app, puts Jellyfesh Web at site/app/: PATH is its built folder
  (platforms/web/dist) or the Jellyfesh-<version>-Web.zip of a release;
* with --release-assets and --sums, takes the download list from a published
  release instead of dist/: FILE is `gh release view TAG --json assets`
  (names and sizes), the sums its SHA256SUMS.txt, and --base-url the
  release's download URL (web.yml, when the site is deployed from main);
* with --releases, takes the download list from a releases.json an earlier
  build wrote (the jellyfesh-site zip that releases before 0.0.7 carried);
* writes releases.json (version, the files with platform, label, size,
  SHA-256 and URL, and web_app when the site has Jellyfesh Web) for the
  download page, shots.json from docs/screenshots/linux/, the release notes
  as releases/latest.html and THIRD_PARTY_NOTICES.html;
* copies the picture of the app under the home page's hero
  (assets/app-now-playing.png) from Jellyfesh Web's screenshots, taken
  against the test servers' invented library: the page labels it as sample
  music.

Only files that exist are listed: platforms without a build show "coming
soon" on the page instead of a link.
"""
import hashlib
import html
import json
import os
import re
import shutil
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# (pattern, platform, label) in display order; {v} is the version. One file per platform
# (docs/github.md, "What a release holds").
FILES = [
    (r"Jellyfesh-{v}-macOS\.dmg", "macos", "Jellyfesh {v} for macOS (.dmg)"),
    (r"Jellyfesh-{v}-Windows-Setup\.exe", "windows", "Jellyfesh {v} Setup for Windows (.exe)"),
    (r"Jellyfesh-{v}-Linux\.flatpak", "linux", "Jellyfesh {v} for Linux (Flatpak)"),
    (r"Jellyfesh-{v}-Android\.apk", "android", "Jellyfesh {v} for Android (.apk)"),
    (r"Jellyfesh-{v}-Web\.zip", "web", "Jellyfesh Web {v} to host yourself (.zip)"),
    (r"Jellyfesh-{v}-Community-Plugins\.zip", "community", "Community plugins {v} (.zip)"),
]


def option(name):
    """The value after `name` on the command line, or None."""
    if name in sys.argv:
        return sys.argv[sys.argv.index(name) + 1]
    return None


def add_app(source, site):
    """Puts Jellyfesh Web (a built folder or a release zip) at site/app/."""
    target = os.path.join(site, "app")
    shutil.rmtree(target, ignore_errors=True)
    if os.path.isdir(source):
        shutil.copytree(source, target)
    else:
        with zipfile.ZipFile(source) as archive:
            for member in archive.namelist():
                parts = member.replace("\\", "/").split("/")
                if member.startswith("/") or ".." in parts:
                    sys.exit(f"{source}: unsafe path {member!r}")
            archive.extractall(target)
    if not os.path.exists(os.path.join(target, "index.html")):
        sys.exit(f"{source}: no index.html, not a Jellyfesh Web build")


def release_files(version, assets_path, sums_path, base_url):
    """The download list of a published release, from its assets and SHA256SUMS.txt."""
    with open(assets_path, encoding="utf-8") as fh:
        assets = {a["name"]: a for a in json.load(fh).get("assets", [])}
    sums = {}
    with open(sums_path, encoding="utf-8") as fh:
        for line in fh:
            parts = line.split()
            if len(parts) == 2:
                sums[parts[1].lstrip("*")] = parts[0].lower()
    files = []
    for pattern, platform, label in FILES:
        rx = re.compile("^" + pattern.replace("{v}", re.escape(version)) + "$")
        for name in sorted(assets):
            if rx.match(name):
                files.append({"platform": platform, "label": label.replace("{v}", version), "name": name,
                              "url": f"{base_url}/{name}", "size": assets[name].get("size", 0),
                              "sha256": sums.get(name, "")})
    return files


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def inline(text):
    text = html.escape(text, quote=False)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", r'<a href="\2">\1</a>', text)
    return text


def markdown(md):
    """The subset our docs use: headings, paragraphs, lists, tables, fences."""
    out, para, items, table, fence = [], [], [], [], None

    def flush():
        nonlocal para, items, table
        if para:
            out.append("<p>" + inline(" ".join(para)) + "</p>")
            para = []
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items = []
        if table:
            rows = [r for r in table if not re.match(r"^\|\s*-", r)]
            cells = [[c.strip() for c in r.strip().strip("|").split("|")] for r in rows]
            head = "".join(f"<th>{inline(c)}</th>" for c in cells[0])
            body = "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in cells[1:])
            out.append(f'<div class="table"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>')
            table = []

    for line in md.splitlines():
        if fence is not None:
            if line.startswith("```"):
                out.append("<pre><code>" + html.escape("\n".join(fence)) + "</code></pre>")
                fence = None
            else:
                fence.append(line)
            continue
        if line.startswith("```"):
            flush()
            fence = []
        elif m := re.match(r"^(#{1,4})\s+(.*)", line):
            flush()
            level = min(len(m.group(1)) + 1, 4)
            out.append(f"<h{level}>{inline(m.group(2))}</h{level}>")
        elif line.startswith("|"):
            if para or items:
                flush()
            table.append(line)
        elif m := re.match(r"^\s*[*-]\s+(.*)", line):
            if para or table:
                flush()
            items.append(m.group(1))
        elif line.startswith("  ") and items:
            items[-1] += " " + line.strip()
        elif not line.strip():
            flush()
        else:
            if table:
                flush()
            para.append(line.strip())
    flush()
    return "\n".join(out)


def page(title, body):
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)} · Jellyfesh</title>
  <link rel="icon" href="{{root}}assets/icon-ice.svg" type="image/svg+xml">
  <link rel="stylesheet" href="{{root}}assets/site.css">
  <style>.prose table{{border-collapse:collapse;width:100%;font-size:14px}}.prose td,.prose th{{border-bottom:1px solid var(--line);padding:6px 8px;text-align:left;vertical-align:top}}.table{{overflow-x:auto}}.prose pre{{overflow-x:auto;background:var(--field-2);padding:12px;border-radius:10px;font-size:13px}}</style>
</head>
<body>
  <header class="top"><div class="wrap">
    <a class="brand" href="{{root}}index.html"><img src="{{root}}assets/icon-ice.svg" alt=""><span>Jellyfesh</span></a>
    <nav aria-label="Site"><a href="{{root}}index.html">Home</a><a href="{{root}}download.html">Download</a><a href="{{root}}privacy.html">Privacy</a></nav>
  </div></header>
  <main><section><div class="wrap prose">
{body}
  </div></section></main>
</body>
</html>
"""


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    version = sys.argv[1].lstrip("v")
    base_url = option("--base-url")
    if base_url:
        base_url = base_url.rstrip("/")
    app = option("--app")
    releases = option("--releases")
    release_assets = option("--release-assets")
    sums = option("--sums")
    if release_assets and not (sums and base_url):
        sys.exit("--release-assets needs --sums and --base-url")
    dist = os.path.join(ROOT, "dist")
    site = os.path.join(dist, "site")
    shutil.rmtree(site, ignore_errors=True)
    shutil.copytree(os.path.join(ROOT, "website"), site)

    files = []
    present = sorted(os.listdir(dist)) if os.path.isdir(dist) and not (releases or release_assets) else []
    if release_assets:
        files = release_files(version, release_assets, sums, base_url)
    if releases:
        with open(releases, encoding="utf-8") as fh:
            earlier = json.load(fh)
        version = earlier.get("version", version)
        files = [f for f in earlier.get("files", []) if re.match(r"^https?://", f.get("url", ""))]
    for pattern, platform, label in FILES:
        rx = re.compile("^" + pattern.replace("{v}", re.escape(version)) + "$")
        for name in present:
            if rx.match(name):
                path = os.path.join(dist, name)
                if base_url:
                    url = f"{base_url}/{name}"
                else:
                    os.makedirs(os.path.join(site, "downloads"), exist_ok=True)
                    shutil.copy2(path, os.path.join(site, "downloads", name))
                    url = f"downloads/{name}"
                files.append({"platform": platform, "label": label.replace("{v}", version), "name": name,
                              "url": url, "size": os.path.getsize(path), "sha256": sha256(path)})
    manifest = {"version": version, "files": files}
    if app:
        add_app(app, site)
        manifest["web_app"] = "app/"
    with open(os.path.join(site, "releases.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2)

    hero_shot = os.path.join(ROOT, "docs", "screenshots", "web", "now-playing-artwork-dark-desktop-mock-server.png")
    if os.path.exists(hero_shot):
        shutil.copy2(hero_shot, os.path.join(site, "assets", "app-now-playing.png"))

    shots = []
    shot_dir = os.path.join(ROOT, "docs", "screenshots", "linux")
    if os.path.isdir(shot_dir):
        os.makedirs(os.path.join(site, "assets", "shots"), exist_ok=True)
        for name in sorted(os.listdir(shot_dir)):
            if name.endswith(".png"):
                shutil.copy2(os.path.join(shot_dir, name), os.path.join(site, "assets", "shots", name))
                caption = os.path.splitext(name)[0].replace("-", " ").replace("_", " ")
                shots.append({"src": f"assets/shots/{name}", "alt": f"Jellyfesh on Linux: {caption}", "caption": caption})
    with open(os.path.join(site, "shots.json"), "w", encoding="utf-8") as fh:
        json.dump(shots, fh, indent=2)

    notes = os.path.join(ROOT, "docs", "releases", f"v{version}.md")
    os.makedirs(os.path.join(site, "releases"), exist_ok=True)
    if os.path.exists(notes):
        body = markdown(open(notes, encoding="utf-8").read())
        html_page = page(f"Release {version}", body).replace("{root}", "../")
        for name in (f"v{version}.html", "latest.html"):
            with open(os.path.join(site, "releases", name), "w", encoding="utf-8") as fh:
                fh.write(html_page)
    notices = os.path.join(ROOT, "THIRD_PARTY_NOTICES.md")
    if os.path.exists(notices):
        body = markdown(open(notices, encoding="utf-8").read())
        with open(os.path.join(site, "THIRD_PARTY_NOTICES.html"), "w", encoding="utf-8") as fh:
            fh.write(page("Third-party notices", body).replace("{root}", ""))
    web = ", Jellyfesh Web at app/" if app else ""
    print(f"site: {site} ({len(files)} downloads, {len(shots)} screenshots{web})")


if __name__ == "__main__":
    main()
