#!/usr/bin/env python3
"""Writes the text of a GitHub release: a Download table with one row per platform, then a link
to the release notes (release.yml's publish job; docs/github.md).

    python3 tools/release/release_body.py --version 0.0.7 --tag v0.0.7 --repository OWNER/NAME
        --files DIR [--notarized] [--windows-signed] [--pro-preview] [--out FILE]

DIR holds the release's files, named Jellyfesh-<version>-<Platform>.<ext>. A platform whose file
is missing says it is coming (the Android APK when the signing key is not set) instead of
linking to nothing. --notarized and --windows-signed say whether the macOS disk image is
notarized and the Windows installer signed; without them, the row says how to open an unsigned
download. --pro-preview says Pro features are free in these builds.
"""
import argparse
import os
import sys

# (platform, file name after "Jellyfesh-<version>-", requirements)
ROWS = [
    ("macOS", "macOS.dmg", "macOS 14.2 or later, Apple silicon or Intel"),
    ("Windows", "Windows-Setup.exe", "Windows 10 version 1809 or later, x64 or Arm64"),
    ("Linux", "Linux.flatpak", "x86_64, with Flatpak"),
    ("Android", "Android.apk", "Android 8.0 or later"),
    ("Web", "Web.zip", "A current browser; a web server of your own"),
    ("Community plugins", "Community-Plugins.zip", "The macOS, Windows, Linux or Android app"),
]


def how_to(platform, name, args):
    """The 'How to install' cell of a platform whose file exists."""
    if platform == "macOS":
        text = "Open the disk image and drag Jellyfesh to Applications."
        if not args.notarized:
            text += (" Not notarized yet: the first time, open System Settings › Privacy & Security"
                     " and choose Open Anyway.")
        return text
    if platform == "Windows":
        text = ("Run it: it installs Jellyfesh for you, without administrator rights, with a Start menu"
                " shortcut, and updates an earlier version.")
        if not args.windows_signed:
            text += " Not signed yet: if SmartScreen appears, choose More info, then Run anyway."
        return text
    if platform == "Linux":
        return f"`flatpak install --user {name}`; the GNOME runtime comes from Flathub."
    if platform == "Android":
        return "Open it on your device and allow installing apps from this source when Android asks."
    if platform == "Web":
        return "Unzip it onto any web server, at any path."
    if platform == "Community plugins":
        return ("The unofficial NetEase Cloud Music and QQ Music plugins, which these apps do not"
                " include: unzip, then Settings › Sources › Install from File… (see the README inside).")
    raise ValueError(platform)


def coming(platform):
    if platform == "Android":
        return ("Coming soon: this release was built without Jellyfesh's Android signing key, so it has"
                " no APK yet.")
    return "Coming soon: not in this release yet."


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--version", required=True, help="the version in the file names, e.g. 0.0.7")
    parser.add_argument("--tag", required=True, help="the release's tag, e.g. v0.0.7")
    parser.add_argument("--repository", required=True, help="OWNER/NAME on GitHub")
    parser.add_argument("--files", required=True, help="the folder with the release's files")
    parser.add_argument("--notarized", action="store_true")
    parser.add_argument("--windows-signed", action="store_true")
    parser.add_argument("--pro-preview", action="store_true")
    parser.add_argument("--out", help="write here instead of standard output")
    args = parser.parse_args()

    base = f"https://github.com/{args.repository}/releases/download/{args.tag}"
    lines = [
        "## Download",
        "",
        "| | File | Requirements | How to install |",
        "| --- | --- | --- | --- |",
    ]
    for platform, suffix, requirements in ROWS:
        name = f"Jellyfesh-{args.version}-{suffix}"
        if os.path.isfile(os.path.join(args.files, name)):
            cell, install = f"[{name}]({base}/{name})", how_to(platform, name, args)
        else:
            cell, install = "—", coming(platform)
        lines.append(f"| **{platform}** | {cell} | {requirements} | {install} |")
    lines += [
        "",
        "Every file's SHA-256 is in [SHA256SUMS.txt]"
        f"({base}/SHA256SUMS.txt): check a download with `sha256sum -c --ignore-missing SHA256SUMS.txt`"
        " on Linux, `shasum -a 256 FILE` on macOS or `Get-FileHash FILE` on Windows.",
        "",
    ]
    if args.pro_preview:
        lines.append("**Jellyfesh Pro** (AI Lyrics and Customize) is free in these builds during the public"
                     " beta (Pro Preview). After the beta it is a one-time purchase: in the app stores, or with"
                     " a licence key, which is the only way on Linux and in Jellyfesh Web.")
    else:
        lines.append("**Jellyfesh Pro** (AI Lyrics and Customize) is a one-time purchase, unlocked in these apps"
                     " with a licence key (Settings › Extensions › Enter Licence Key…); on Linux and in"
                     " Jellyfesh Web a licence key is the only way.")
    notes = f"https://github.com/{args.repository}/blob/{args.tag}/docs/releases/{args.tag}.md"
    lines += ["", f"[What's new in Jellyfesh {args.version}: the release notes]({notes})", ""]
    text = "\n".join(lines)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
