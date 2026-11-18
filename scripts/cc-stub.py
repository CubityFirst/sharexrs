"""A do-nothing C compiler and archiver for linting macOS from Windows.

`cargo clippy --target aarch64-apple-darwin` runs build scripts, and
libwebp-sys's compiles libwebp with `cc`, which needs a C compiler for the
target (and the macOS SDK). Clippy never links, so empty object files and
archives are enough: `scripts/clippy-macos.ps1` points `CC_aarch64_apple_darwin`
and `AR_aarch64_apple_darwin` here. Real macOS builds (CI) use the real
compiler.

Compiler: writes an empty file at `-o <path>` (or `-o<path>`). Archiver
(`ar cq lib.a a.o ...`, `ar s lib.a`): writes an empty `!<arch>` archive at the
first `.a` argument.
"""

import sys

args = sys.argv[1:]
out = None
for i, a in enumerate(args):
    if a == "-o" and i + 1 < len(args):
        out = args[i + 1]
        break
    if a.startswith("-o") and len(a) > 2:
        out = a[2:]
        break
if out is not None:
    open(out, "wb").close()
    sys.exit(0)
for a in args:
    if a.endswith(".a") or a.endswith(".lib"):
        with open(a, "wb") as f:
            f.write(b"!<arch>\n")
        sys.exit(0)
# Anything else (`-E` for compiler family detection, `--version`): succeed
# without output.
sys.exit(0)
