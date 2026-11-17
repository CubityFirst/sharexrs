"""Generates crates/sharex-ui/src/keys.rs from ShareX.Icons/*Icons.cs.

    python scripts/gen-icon-keys.py <path to ShareX checkout>
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1]) / "ShareX.Icons"
out = [
    "//! The icon keys of `ShareX.Icons`, generated from its `*Icons.cs` by",
    "//! `scripts/gen-icon-keys.py`: same constant names, same `prefix/name` keys.",
    "",
]
for cls in ["AppIcons", "ImageEditorIcons", "ImageEffectIcons", "PhotoEditorIcons", "PixelEditorIcons"]:
    src = (root / f"{cls}.cs").read_text(encoding="utf-8-sig")
    m = re.search(r'const string Prefix = "([^"]*)"', src)
    prefix = m.group(1) if m else ""
    consts = re.findall(r'public const string (\w+) = (Prefix \+ )?"([^"]+)"', src)
    mod = re.sub(r"(?<!^)([A-Z])", r"_\1", cls).lower()
    out.append(f"/// `ShareX.Icons.{cls}`.")
    out.append("#[allow(non_upper_case_globals)]")
    out.append(f"pub mod {mod} {{")
    for name, has_prefix, val in consts:
        out.append(f'    pub const {name}: &str = "{prefix + val if has_prefix else val}";')
    out.append("}")
    out.append("")
dest = Path(__file__).resolve().parent.parent / "crates/sharex-ui/src/keys.rs"
dest.write_text("\n".join(out), encoding="utf-8")
print(f"{dest}: {sum(l.strip().startswith('pub const') for l in out)} keys")
