"""Writes the cases tools/cli-harness runs through ShareX's own code and
ShareXrs's tests replay: crates/sharex-helpers/tests/fixtures/cli/cases.json
(CLIManager, BinaryWriter) and
crates/sharexrs-native-messaging/tests/fixtures/cases.json (the native
messaging input JSON).

    python -I tools/cli-harness/make_cases.py
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]

ARRAYS = [
    [],
    ["-silent"],
    ["-SILENT", "-Portable"],
    ["-s", "-p"],
    ["/silent", "--portable", "-"],
    ["-silent", r"C:\a.png"],
    ["-silent", r"C:\a.png", r"C:\b.png"],
    [r"C:\a.png", r"D:\folder", "https://example.com/x.png"],
    ["-workflow", "Capture region (Upload)", "-task", "Fullscreen"],
    ["-task", "-silent"],
    ["-CustomUploader", r"C:\up\x.sxcu"],
    ["-ImageEffect", r"C:\fx\e.sxie", "-ImageEffects"],
    ["-OCR", r"C:\img.png", "-ImageEditor", r"C:\img.png"],
    ["-ExitShareX"],
    ["-NativeMessagingInput", r"C:\Temp\tmp1.json"],
    ["-silent", "", "-portable"],
    ["", "-silent"],
    ["-a", "b", "", "c"],
    ["-ÉCRAN", "-ıd", "-ſilent", "-straße"],
    ["-multi", "-m", "-NoHotkeys", "-AutoClose", "-sandbox", "-uninstall"],
    ["-x=1", "-y:2", "- spaced", '-"quoted"'],
    ["file with spaces.txt", "-", "-", "x"],
    ["-a", "-b", "c", "-d", "e", "f", "g"],
    ["é", "日本語", "😀", "a\u0000b", "x" * 300],
]

QUERIES = [
    ["silent"], ["s"], ["silent", "s"], ["-silent"], ["--silent"], ["portable", "p"], ["p"], ["Portable"],
    [""], ["-"], ["multi", "m"], ["NoHotkeys"], ["nohotkeys"], ["AutoClose"], ["sandbox"], ["uninstall"],
    ["workflow"], ["task"], ["écran"], ["ÉCRAN"], ["ID"], ["silent", ""], ["SILENT"], ["SILENT".lower()],
    ["straSSe"], ["STRASSE"], ["STRAßE"], ["a"], ["x=1"], ['"quoted"'],
]

PARAMETERS = [
    "workflow", "task", "CustomUploader", "ImageEffect", "OCR", "ImageEditor", "NativeMessagingInput",
    "silent", "a", "b", "d", r"C:\a.png", "x", "",
]

STRINGS = [
    "", "-silent", "-silent ", "-a b ", '-a "b c" d ', '-a "b c"', 'ab"c d" e ', '  ""  x\ty\n',
    '"C:\\Program Files\\ShareX\\ShareX.exe" -silent ', '-a ""b"" ', "x\u00a0y\u3000z ", '"unterminated ',
    '-workflow "My workflow" -task Fullscreen ', "a\u2028b\u0085c ",
]

NATIVE = [
    '{"Action":1,"URL":"data:image/png;base64,AAAA","Text":null}',
    '{"action":"UploadText","text":"hello"}',
    '{"Action":"uploadimage","Url":"https://x.y/z.png"}',
    '{"Action":"ShortenURL","URL":"https://x.y"}',
    '{"Action":5,"URL":"https://x.y"}',
    '{"Action":0}',
    '{"Action":9}',
    '{"Action":-1}',
    '{"Action":"Nope"}',
    '{"Action":"1"}',
    '{"Action":"UploadVideo, UploadAudio"}',
    '{"Action":2.0}',
    '{"Action":true}',
    '{"Action":null,"URL":"u"}',
    '{"URL":"https://x.y/a.mp4","Text":"t"}',
    '{"Text":5}',
    '{"Text":true}',
    '{"URL":["a"]}',
    '{"Unknown":1,"Text":"x"}',
    "{}",
    "null",
    "",
    "   ",
    "[]",
    "not json",
    '{"Text":"a"',
    '{"Text":"a"} trailing',
    "{'Text':'single'}",
    '{Text:"unquoted"}',
    '{"Text":"a","Text":"b"}',
    '{"text":"a","Text":"b"}',
    '\ufeff{"Text":"bom"}',
    '{"Text":"line\\nbreak","URL":"\\u00e9"}',
    '{"Action":"UPLOADAUDIO","URL":"https://x.y/a.mp3"} // comment',
    '{"Text":"x","URL":["a"]}',
    '{"URL":["a"],"Text":"x"}',
    '{"Action":[1],"Text":"x"}',
    '{"Action":{},"Text":"x"}',
    '{"Text":{"a":1}}',
    '{"Text":"x",}',
    '{"Text":"x" "URL":"y"}',
    '{"Action":"  UploadText "}',
    '{"Action":"uploadtext,shortenurl"}',
    '{"Action":"UploadText,"}',
    '{"Action":1e0}',
    '{"Action":"0x1"}',
    '{"Action":" 2 "}',
    '{"Action":"-3"}',
    '{"Action":2147483648}',
    '{"Action":-0}',
    '{"Action":""}',
    '{"Text":1.50}',
    '{"Text":1e3}',
    '{"Text":-0.0}',
    '{"Text":12345678901234567890}',
    '{"Text":null,"URL":""}',
    '{"Text":undefined}',
    '{"Text":"a\\u0000b"}',
    '/* c */ {"Text":"c"}',
    '{"Text":\'it\\\'s\'}',
    '{"Text":"x"}{"Text":"y"}',
    '{"Text":"x",\n"URL":"y"\n',
    '{"Text":"x","URL":',
    '{"Text":"x","URL"',
    '{"Text":"x","URL":"y',
    '{"Text":"\\x"}',
    '{"Text":NaN}',
    '{"Text":Infinity}',
    '{"Text":new Date(1)}',
    '{"Text":"x", "Action":"UploadText" /* c */ }',
    '{"Text":"😀"}',
    "\n\t {\"Text\":\"ws\"}",
]


def main():
    cli = ROOT / "crates/sharex-helpers/tests/fixtures/cli/cases.json"
    cli.parent.mkdir(parents=True, exist_ok=True)
    cli.write_text(
        json.dumps({"arrays": ARRAYS, "queries": QUERIES, "parameters": PARAMETERS, "strings": STRINGS},
                   ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8")
    native = ROOT / "crates/sharexrs-native-messaging/tests/fixtures/cases.json"
    native.parent.mkdir(parents=True, exist_ok=True)
    native.write_text(json.dumps({"inputs": NATIVE}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
