"""Writes the archives and cases tools/settings-harness runs through ShareX's
ZipManager and ShareXrs's tests replay: crates/sharex-helpers/tests/fixtures/zip/.

Each archive is made here with fixed time stamps, so the fixtures don't change
from run to run. They cover what SettingManager.Import meets: a ShareX backup,
names that try to leave the folder, rooted names, folders, other extensions,
upper-case extensions, backslashes, the size limit and a file that isn't a zip.
"""
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates" / "sharex-helpers" / "tests" / "fixtures" / "zip"
STAMP = (2026, 11, 19, 10, 20, 30)


def archive(name, entries):
    path = OUT / name
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for entry_name, data in entries:
            info = zipfile.ZipInfo(entry_name, date_time=STAMP)
            info.compress_type = zipfile.ZIP_STORED if entry_name.endswith("/") else zipfile.ZIP_DEFLATED
            z.writestr(info, data)
    return name


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    backup = archive("backup.sxb", [
        ("ApplicationConfig.json", '{\r\n  "ShowTray": true\r\n}'),
        ("UploadersConfig.json", '{\r\n  "ImgurAccountType": "Anonymous"\r\n}'),
        ("HotkeysConfig.json", '{\r\n  "Hotkeys": []\r\n}'),
        ("History.db", b"SQLite format 3\x00" + bytes(48)),
    ])
    hostile = archive("hostile.sxb", [
        ("../escape.json", "{}"),
        ("sub/../../escape2.json", "{}"),
        ("/rooted.json", "{}"),
        ("C:/drive.json", "{}"),
        ("folder/", ""),
        ("folder/nested/inner.json", '{"a":1}'),
        ("Upper.JSON", "{}"),
        ("legacy.xml", "<x/>"),
        ("notes.txt", "text"),
        ("noext", "x"),
        ("back\\slash.json", "{}"),
        ("dots/./same.json", "{}"),
        ("trail.json.", "{}"),
    ])
    sizes = archive("sizes.sxb", [
        ("a.json", "0123456789"),
        ("b.bin", "0123456789"),
    ])
    (OUT / "not-a-zip.sxb").write_bytes(b"This is not a zip archive.\r\n")

    filt = "json-xml"
    cases = [
        {"name": "backup-import", "archive": backup, "retain": True, "filter": filt, "max": 1_000_000_000},
        {"name": "backup-all", "archive": backup, "retain": True, "filter": None, "max": 0},
        {"name": "hostile-import", "archive": hostile, "retain": True, "filter": filt, "max": 1_000_000_000},
        {"name": "hostile-all", "archive": hostile, "retain": True, "filter": None, "max": 0},
        {"name": "hostile-flat", "archive": hostile, "retain": False, "filter": filt, "max": 0},
        {"name": "sizes-at-limit", "archive": sizes, "retain": True, "filter": filt, "max": 20},
        {"name": "sizes-over-limit", "archive": sizes, "retain": True, "filter": filt, "max": 19},
        {"name": "not-a-zip", "archive": "not-a-zip.sxb", "retain": True, "filter": filt, "max": 1_000_000_000},
        # ShareXrs's own export (written by `cargo test -p sharex-helpers
        # --test zip` with SHAREXRS_WRITE_FIXTURES=1), read as ShareX imports.
        {"name": "sharexrs-export-import", "archive": "sharexrs-export.sxb", "retain": True, "filter": filt, "max": 1_000_000_000},
    ]
    (OUT / "cases.json").write_text(json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8")
    print(f"{len(cases)} cases in {OUT}")


if __name__ == "__main__":
    main()
