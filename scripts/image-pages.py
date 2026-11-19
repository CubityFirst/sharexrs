"""Which of an executable's pages are resident, by PE section, and which of
them hold `sharex-strings`' text, language by language (Windows).

  powershell -File scripts/wsmap.ps1 -Id <pid> -PagesOf sharexrs.exe -PagesOut pages.txt
  python scripts/image-pages.py --exe <the same sharexrs.exe> --pages pages.txt \
      --strings target/release/build/sharex-strings-<hash>/out

`--strings` is a `sharex-strings` OUT_DIR from the split-by-language layout
(`<n>.txt`, `<n>.ends`, `strings.rs` for the language names): its strings
are what's looked for in the exe's `.rdata`, so it works on an exe of any
layout as long as the text is the same. A string counts for a language when
it's at least 16 bytes, differs from English's, and its first 16 bytes
belong to no other language's string; a page holds a language's text when
one of those strings touches it. Pages mostly of 8-byte pointers into the
image (the old layout's per-key tables of 29 `Option<&str>`) are counted
apart.
"""

import argparse
import ast
import os
import re
import struct

PAGE = 4096
PREFIX = 16


def sections(exe):
    pe = struct.unpack_from("<I", exe, 0x3C)[0]
    count, opt_size = struct.unpack_from("<H", exe, pe + 6)[0], struct.unpack_from("<H", exe, pe + 20)[0]
    opt = pe + 24
    image_base = struct.unpack_from("<Q", exe, opt + 24)[0]
    image_size = struct.unpack_from("<I", exe, opt + 56)[0]
    out = []
    for i in range(count):
        at = opt + opt_size + 40 * i
        name = exe[at:at + 8].rstrip(b"\0").decode()
        vsize, rva, raw_size, raw_at = struct.unpack_from("<IIII", exe, at + 8)
        out.append({"name": name, "rva": rva, "vsize": vsize, "data": exe[raw_at:raw_at + min(raw_size, vsize)]})
    return image_base, image_size, out


def blobs(folder):
    names = ast.literal_eval(re.search(r"pub const LANGUAGES: \[&str; \d+\] = (\[.*?\]);",
                                       open(os.path.join(folder, "strings.rs"), encoding="utf-8").read()).group(1))
    langs = []
    for i, name in enumerate(names):
        text = open(os.path.join(folder, f"{i}.txt"), "rb").read()
        raw = open(os.path.join(folder, f"{i}.ends"), "rb").read()
        ends = struct.unpack(f"<{len(raw) // 4}I", raw)
        values, start = [], 0
        for e in ends:
            if e & 0x80000000:
                values.append(None)
                continue
            values.append(text[start:e])
            start = e
        langs.append((name or "en", values, (text, raw)))
    return langs


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--exe", required=True)
    ap.add_argument("--pages", required=True, help="resident RVAs, hex, one a line (wsmap.ps1 -PagesOf)")
    ap.add_argument("--strings", required=True, help="a sharex-strings OUT_DIR")
    a = ap.parse_args()

    exe = open(a.exe, "rb").read()
    resident = {int(line, 16) // PAGE for line in open(a.pages) if line.strip()}
    image_base, image_size, secs = sections(exe)
    print(f"{a.exe}: {len(exe) / 2**20:.1f} MB; resident {len(resident) * 4} KB of the image")
    for s in secs:
        pages = range(s["rva"] // PAGE, (s["rva"] + s["vsize"] + PAGE - 1) // PAGE)
        print(f"  {s['name']:8} {s['vsize'] / 1024:9.0f} KB, resident {sum(p in resident for p in pages) * 4:7} KB")

    rdata = next(s for s in secs if s["name"] == ".rdata")
    data, base = rdata["data"], rdata["rva"]

    # Pointer pages: most 8-byte words point into the image.
    pointer_pages = set()
    lo, hi = image_base, image_base + image_size
    for off in range(0, len(data) - PAGE + 1, PAGE):
        words = struct.unpack_from(f"<{PAGE // 8}Q", data, off)
        if sum(lo <= w < hi for w in words) * 2 >= len(words):
            pointer_pages.add((base + off) // PAGE)
    print(f"  pointer pages in .rdata: {len(pointer_pages) * 4} KB, resident {len(pointer_pages & resident) * 4} KB")

    langs = blobs(a.strings)
    english = langs[0][1]
    owner = {}  # first 16 bytes -> (language index, length), or None if shared
    for li, (_, values, _) in enumerate(langs):
        for ki, v in enumerate(values):
            if v is None or len(v) < PREFIX or (li and v == english[ki]):
                continue
            k = v[:PREFIX]
            if k in owner and (owner[k] is None or owner[k][0] != li):
                owner[k] = None
            else:
                owner[k] = (li, min(len(v), owner.get(k, (li, len(v)))[1]))
    pages = [set() for _ in langs]
    for i in range(len(data) - PREFIX + 1):
        hit = owner.get(data[i:i + PREFIX])
        if hit:
            li, n = hit
            pages[li].update(range((base + i) // PAGE, (base + i + n - 1) // PAGE + 1))
    every = set().union(*pages)
    print(f"  pages holding translations: {len(every) * 4} KB, resident {len(every & resident) * 4} KB")
    print("    language, its strings + ends table: the pages its strings touch (resident);"
          " the blobs' own pages (resident) if each is in one piece")
    blob_pages = set()
    for (name, _, parts), p in zip(langs, pages):
        size = sum(map(len, parts))
        line = f"    {name:6} {size / 1024:4.0f} KB: {len(p) * 4:5} KB ({len(p & resident) * 4:4} KB)"
        found = [data.find(b) for b in parts]
        if all(at >= 0 for at in found):
            own = set().union(*(range((base + at) // PAGE, (base + at + len(b) - 1) // PAGE + 1)
                                for at, b in zip(found, parts)))
            blob_pages |= own
            line += f"; blobs {len(own) * 4:4} KB ({len(own & resident) * 4:4} KB)"
        print(line)
    if blob_pages:
        print(f"  all blobs: {len(blob_pages) * 4} KB, resident {len(blob_pages & resident) * 4} KB")


if __name__ == "__main__":
    main()
