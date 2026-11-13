#!/usr/bin/env python3
"""Writes THIRD_PARTY_NOTICES.md: the licences of every Rust crate compiled
into the shipped binaries, for distribution with them.

The crates are the normal (not dev or build) dependencies of the shipped
packages, on every target platform, as `cargo tree` resolves them. Licence
texts come from each crate's own files in the Cargo registry; a crate that
ships none is listed with its SPDX expression and the standard text of that
licence is included once.

    python3 tools/licenses/generate.py [--check] [--out FILE]
                                       [--npm DIR:NAME[,NAME...]] [packages...]

--out writes FILE instead of THIRD_PARTY_NOTICES.md at the repository root.
--npm adds npm packages installed in DIR/node_modules whose code a build puts
into a shipped program (Jellyfesh Web: Svelte's runtime, Vite's loader).
With --check, exits 1 when the file is out of date.
"""
import glob
import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "THIRD_PARTY_NOTICES.md")
# Everything shipped: the CLI, the Linux app, the FFI library inside the macOS,
# Windows and Android apps, the plugin author tool, and the official plugins
# every app bundles.
DEFAULT_PACKAGES = ["jf-cli", "jellyfesh-gtk", "jf-ffi", "jf-plugin", "jf-subsonic", "jf-jellyfin"]
# Where an npm package's licence file goes on to list what the package's own
# build tooling bundles, which does not reach the programs built with it.
NPM_BUNDLED_SECTION = "\n# Licenses of bundled dependencies"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)([-_.].*)?$", re.I)


def workspace_packages():
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT))
    return {p["name"] for p in meta["packages"]}, meta


def dependencies(package):
    """name, version, licence and source of every normal dependency of `package`."""
    out = subprocess.check_output(
        ["cargo", "tree", "-e", "normal", "--target", "all", "-p", package,
         "--prefix", "none", "--format", "{p}|{l}"], cwd=ROOT, text=True)
    deps = {}
    for line in out.splitlines():
        line = line.strip()
        if not line or "|" not in line:
            continue
        ident, licence = line.split("|", 1)
        licence = licence.replace("(*)", "").strip()
        ident = ident.replace("(*)", "").strip()
        parts = ident.split()
        name, version = parts[0], parts[1].lstrip("v")
        local = "(" in ident and ("/" in ident.split("(", 1)[1])
        deps[(name, version)] = (licence.strip() or "UNKNOWN", local)
    return deps


def package_authors():
    """(name, version) -> authors, from the full metadata."""
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1"], cwd=ROOT))
    return {(p["name"], p["version"]): p.get("authors") or [] for p in meta["packages"]}


def registry_dir(name, version):
    home = os.environ.get("CARGO_HOME", os.path.expanduser("~/.cargo"))
    hits = glob.glob(os.path.join(home, "registry", "src", "*", f"{name}-{version}"))
    return hits[0] if hits else None


def licence_files(path):
    files = []
    for entry in sorted(os.listdir(path)):
        full = os.path.join(path, entry)
        if os.path.isfile(full) and LICENSE_FILE.match(entry):
            files.append(full)
    return files


def parse_args(argv):
    """packages, npm (folder, name) pairs, output file, --check."""
    packages, npm, out, check = [], [], OUT, False
    args = iter(argv)
    for arg in args:
        if arg == "--check":
            check = True
        elif arg in ("--out", "--npm"):
            value = next(args, None)
            if value is None:
                sys.exit(f"{arg} needs a value")
            if arg == "--out":
                out = os.path.abspath(value)
            else:
                folder, _, names = value.partition(":")
                npm += [(folder, name) for name in names.split(",") if name]
        elif arg.startswith("--"):
            sys.exit(f"unknown option {arg}")
        else:
            packages.append(arg)
    return packages, npm, out, check


def npm_package(folder, name):
    """version, licence, licence files and authors of an installed npm package."""
    path = os.path.join(ROOT, folder, "node_modules", *name.split("/"))
    with open(os.path.join(path, "package.json"), encoding="utf-8") as fh:
        meta = json.load(fh)
    licence = meta.get("license") or "UNKNOWN"
    if isinstance(licence, dict):
        licence = licence.get("type") or "UNKNOWN"
    if licence.startswith("(") and licence.endswith(")"):
        licence = licence[1:-1]
    author = meta.get("author") or []
    if isinstance(author, dict):
        author = author.get("name") or []
    authors = [author] if isinstance(author, str) else []
    return meta["version"], licence, licence_files(path), authors


def main():
    args, npm, out, check = parse_args(sys.argv[1:])
    ours, _ = workspace_packages()
    authors = package_authors()
    packages = [p for p in (args or DEFAULT_PACKAGES) if p in ours]
    deps = {}
    for p in packages:
        deps.update(dependencies(p))
    texts = {}       # sha -> text
    rows = []        # (name, version, licence, [sha])
    for (name, version), (licence, local) in sorted(deps.items()):
        if local or name in ours:
            continue  # our own crates
        src = registry_dir(name, version)
        shas = []
        if src:
            for f in licence_files(src):
                with open(f, encoding="utf-8", errors="replace") as fh:
                    text = fh.read().strip()
                sha = hashlib.sha256(text.encode()).hexdigest()[:12]
                texts.setdefault(sha, text)
                shas.append(sha)
        rows.append((name, version, licence, shas, authors.get((name, version), [])))
    for folder, name in npm:
        version, licence, files, who = npm_package(folder, name)
        shas = []
        for f in files:
            with open(f, encoding="utf-8", errors="replace") as fh:
                text = fh.read().split(NPM_BUNDLED_SECTION)[0].strip()
            sha = hashlib.sha256(text.encode()).hexdigest()[:12]
            texts.setdefault(sha, text)
            shas.append(sha)
        rows.append((name, version, licence, shas, who))
    sources = ", ".join(f"`{p}`" for p in packages)
    if npm:
        sources += "; and the npm packages " + ", ".join(f"`{n}`" for _, n in npm)
    lines = [
        "# Third-party notices",
        "",
        "Jellyfesh is distributed with the following open-source components.",
        "Each is listed with its licence; the licence texts follow. Generated by",
        "`tools/licenses/generate.py` from the dependency graph of: " + sources + ".",
        "",
        "| Component | Version | Licence | Texts |",
        "| --- | --- | --- | --- |",
    ]
    for name, version, licence, shas, who in rows:
        if shas:
            refs = ", ".join(f"[{s}](#t-{s})" for s in shas)
        else:
            holders = "; ".join(re.sub(r"\s*<[^>]*>", "", a) for a in who) or "its authors"
            refs = f"standard text below; copyright {holders}"
        lines.append(f"| {name} | {version} | {licence} | {refs} |")
    lines += ["", "## Licence texts", ""]
    for sha, text in sorted(texts.items()):
        lines += [f'<a id="t-{sha}"></a>', f"### {sha}", "", "```text", text, "```", ""]
    standard = os.path.join(ROOT, "LICENSES")
    if os.path.isdir(standard):
        lines += ["## Standard licence texts", "",
                  "For components that ship no licence file, the text of the licence named",
                  "in the table applies:", ""]
        for f in sorted(os.listdir(standard)):
            with open(os.path.join(standard, f), encoding="utf-8") as fh:
                lines += [f"### {os.path.splitext(f)[0]}", "", "```text", fh.read().strip(), "```", ""]
    content = "\n".join(lines) + "\n"
    shown = os.path.relpath(out, ROOT)
    if shown.startswith(".."):
        shown = out
    if check:
        current = open(out, encoding="utf-8").read() if os.path.exists(out) else ""
        if current != content:
            print(f"{shown} is out of date: run tools/licenses/generate.py", file=sys.stderr)
            sys.exit(1)
        print(f"{shown} is up to date")
        return
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(content)
    missing = [r for r in rows if not r[3]]
    print(f"{len(rows)} components, {len(texts)} distinct licence texts; {len(missing)} use the standard texts")


if __name__ == "__main__":
    main()
