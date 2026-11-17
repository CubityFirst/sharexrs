#!/usr/bin/env python3
"""Downloads what a ShareXrs release bundles beside the program, checks each
download against its SHA256 in packaging/bundled.json, and lays it out the
way the program looks for it (docs/RELEASING.md):

    <out>/ffmpeg[.exe]                       FFmpeg (ShareX: ffmpeg.exe beside ShareX.exe)
    <out>/onnxruntime.dll | libonnxruntime.dylib | libonnxruntime.so
    <out>/Models/PP-OCRv6_small_{det,rec}/inference.{onnx,yml}
    <out>/Licenses/*.txt                      ShareXrs's, FFmpeg's (with where its
                                              source is), ONNX Runtime's, PaddleOCR's,
                                              and the Rust crates in the program

    python3 scripts/fetch-bundled.py --platform windows-x64 --out dist/payload \
        [--cache DIR] [--rust-target x86_64-pc-windows-msvc]

Platforms: windows-x64, windows-arm64, macos-arm64, linux-x64. Downloads are
kept in the cache folder (default: $SHAREXRS_BUNDLED_CACHE, else
~/.cache/sharexrs-bundled) under their SHA256 and checked again on every use.
A download whose hash differs is an error: nothing unchecked is packaged.

Python's standard library only, so it runs as-is on every CI runner.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "packaging" / "bundled.json"
PLATFORMS = ("windows-x64", "windows-arm64", "macos-arm64", "linux-x64")
USER_AGENT = "ShareXrs-release-packaging"


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch(url, sha256, cache):
    """The file at `url`, from the cache or downloaded, with this SHA256."""
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / sha256
    if path.is_file():
        if sha256_of(path) == sha256:
            print(f"cached   {url}")
            return path
        path.unlink()
    print(f"download {url}")
    tmp = path.with_suffix(".part")
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=600) as response, open(tmp, "wb") as f:
        shutil.copyfileobj(response, f, 1 << 20)
    actual = sha256_of(tmp)
    if actual != sha256:
        tmp.unlink()
        sys.exit(f"error: {url}\n  expected SHA256 {sha256}\n  got             {actual}")
    tmp.replace(path)
    return path


def extract(archive, url, member, dest):
    """Copies one member of a zip (or .nupkg) or tar archive to `dest`, by its
    exact name, so nothing else in the archive is ever written out."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    if url.endswith((".zip", ".nupkg")):
        with zipfile.ZipFile(archive) as z, z.open(member) as src, open(dest, "wb") as out:
            shutil.copyfileobj(src, out)
    elif url.endswith((".tar.xz", ".tar.gz", ".tgz")):
        with tarfile.open(archive) as t:
            info = t.getmember(member)
            if not info.isfile():
                sys.exit(f"error: {member} in {url} isn't a regular file")
            with t.extractfile(info) as src, open(dest, "wb") as out:
                shutil.copyfileobj(src, out)
    else:
        sys.exit(f"error: don't know how to unpack {url}")


def repository_url():
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("repository"):
            return line.split("=", 1)[1].strip().strip('"')
    return ""


def ffmpeg_source_notice(entry):
    repo = repository_url()
    lines = [
        "FFmpeg",
        "======",
        "",
        "ShareXrs ships an unmodified FFmpeg program beside it, which it runs for",
        "screen recording and video conversion, as ShareX does. FFmpeg is free",
        "software under the GNU General Public License version 3 (this build was",
        "configured with --enable-gpl --enable-version3); its licence is in",
        "FFmpeg_license.txt.",
        "",
        f"This build: {entry['build']}",
        f"Downloaded from: {entry['url']}",
        f"SHA256 of that download: {entry['sha256']}",
        "",
        "Its Corresponding Source:",
        "",
    ]
    lines += [f"- {s}" for s in entry["source"]]
    lines += [
        "",
        "Written offer: for at least three years after the ShareXrs release this",
        "copy came with, and for as long as that release is offered for download,",
        "the ShareXrs project will give anyone who asks a copy of the complete",
        "Corresponding Source of this FFmpeg build, on a medium customarily used",
        "for software interchange, for no more than the cost of providing it.",
    ]
    if repo:
        lines.append(f"Ask by opening an issue at {repo}/issues.")
    return "\n".join(lines) + "\n"


def rust_crates(target):
    """The crates compiled into sharexrs for `target`, with their licences,
    from `cargo metadata` (normal dependencies only, followed from sharexrs)."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    meta = json.loads(out)
    packages = {p["id"]: p for p in meta["packages"]}
    members = set(meta["workspace_members"])
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(i for i in members if packages[i]["name"] == "sharexrs")
    seen, stack = set(), [root]
    while stack:
        node = nodes[stack.pop()]
        for dep in node["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]) and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])
    crates = sorted((packages[i] for i in seen - members), key=lambda p: (p["name"], p["version"]))
    lines = [
        "Rust crates compiled into ShareXrs",
        "==================================",
        "",
        f"For {target}. Each crate's licence text is in its source package on",
        "crates.io and in its repository.",
        "",
    ]
    for p in crates:
        lic = p.get("license") or "(see its licence file)"
        where = p.get("repository") or f"https://crates.io/crates/{p['name']}"
        lines.append(f"{p['name']} {p['version']}: {lic} ({where})")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--platform", required=True, choices=PLATFORMS)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--cache", type=Path, default=Path(os.environ.get(
        "SHAREXRS_BUNDLED_CACHE", Path.home() / ".cache" / "sharexrs-bundled")))
    parser.add_argument("--rust-target", help="also list the Rust crates built into sharexrs for this target triple")
    args = parser.parse_args()

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    out = args.out
    if out.exists():
        shutil.rmtree(out)
    licenses = out / "Licenses"
    licenses.mkdir(parents=True)
    windows = args.platform.startswith("windows")

    # FFmpeg, beside the program.
    ff = manifest["ffmpeg"][args.platform]
    archive = fetch(ff["url"], ff["sha256"], args.cache)
    ffmpeg = out / ("ffmpeg.exe" if windows else "ffmpeg")
    extract(archive, ff["url"], ff["member"], ffmpeg)
    (licenses / "FFmpeg_source.txt").write_text(ffmpeg_source_notice(ff), encoding="utf-8", newline="\n")

    # ONNX Runtime, beside the program (macos-bundle.sh moves it to Frameworks).
    ort = manifest["onnxruntime"]
    archive = fetch(ort["url"], ort["sha256"], args.cache)
    member = ort["members"][args.platform]
    extract(archive, ort["url"], member, out / Path(member).name)
    for name, as_name in ort["licenses"].items():
        extract(archive, ort["url"], name, licenses / as_name)

    # The OCR models, in Models beside the program.
    for f in manifest["models"]["files"]:
        path = fetch(f["url"], f["sha256"], args.cache)
        dest = out / "Models" / f["path"]
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, dest)

    # Licences: ShareXrs's own, and the ones vendored from ShareX's Licenses.
    shutil.copyfile(ROOT / "LICENSE", licenses / "ShareXrs_license.txt")
    for f in sorted((ROOT / "packaging" / "Licenses").glob("*.txt")):
        shutil.copyfile(f, licenses / f.name)
    if args.rust_target:
        (licenses / "Rust_crates.txt").write_text(rust_crates(args.rust_target), encoding="utf-8", newline="\n")

    if not windows:
        for p in out.iterdir():
            if p.is_file():
                p.chmod(0o755)

    for p in sorted(out.rglob("*")):
        if p.is_file():
            print(f"{p.stat().st_size:>12}  {p.relative_to(out).as_posix()}")


if __name__ == "__main__":
    main()
