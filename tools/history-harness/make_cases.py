"""Writes cases.json for the history harness (see README.md).

Dates are {"local"|"utc"|"unspecified"|"utc_to_local": "yyyy-MM-ddTHH:mm:ss.fffffff"}:
a wall clock time with that DateTimeKind, or a UTC time converted with
ToLocalTime() (which marks the first 01:xx of a repeated hour). The run
machine's zone matters (both sides use it); GMT Standard Time (London) has
the DST edges these cases poke at.
"""

import json
import random
import sys

rnd = random.Random(22_4)


def local(s):
    return {"local": s}


def utc(s):
    return {"utc": s}


def unspecified(s):
    return {"unspecified": s}


def utc_to_local(s):
    return {"utc_to_local": s}


def item(name, date, type_="Image", host=None, url=None, path="default", tags=None, **more):
    o = {"FileName": name, "DateTime": date, "Type": type_}
    if path == "default":
        path = None if name is None else "C:\\Shots\\2026-11\\" + name
    o["FilePath"] = path
    if host is not None:
        o["Host"] = host
    if url is not None:
        o["URL"] = url
    if tags is not None:
        o["Tags"] = tags
    o.update(more)
    return o


ODD = "q\"\\ \u0001\u001f\t\n\r\b\f \u0085\u2028\u2029 é ✓ 😀 <>&' \u007f"

write = [
    item("ShareX_abc.png", local("2026-07-01T10:00:00.1234567"),
         tags=[["WindowTitle", "Chrome - Page"], ["ProcessName", "chrome"]]),
    item("upload.png", local("2026-01-15T23:59:59.9999999"), host="Imgur", url="https://i.imgur.com/upload.png",
         ThumbnailURL="https://i.imgur.com/uploadt.png", DeletionURL="https://imgur.com/delete/x",
         ShortenedURL="https://bit.ly/x"),
    item("paste.txt", utc("2026-11-16T08:00:00.0000000"), type_="Text", host="Pastebin",
         url="https://pastebin.com/x", path=None),
    item("unspec.png", unspecified("2026-11-15T20:30:00.5000000")),
    item("ambiguous-first.png", utc_to_local("2025-10-26T00:30:00.0000000")),
    item("ambiguous-second.png", utc_to_local("2025-10-26T01:30:00.0000000")),
    item("gap.png", local("2025-03-30T01:30:00.0000000")),
    item("ambiguous-plain.png", local("2025-10-26T01:15:00.0000000")),
    item(None, local("2026-11-17T09:00:00.0000000"), path="C:\\x.png"),  # no file name: skipped
    item("min.png", None),  # MinValue: skipped
    item("nowhere.png", local("2026-11-17T09:00:00.0000000"), path=None),  # no URL or path: skipped
    item("empty-strings.png", local("2026-11-17T09:30:00.0000000"), type_="", host="", url="u", path=""),
    item("odd-tags.png", local("2026-11-17T10:00:00.0000000"),
         tags=[["Odd " + ODD, ODD], ["NullValue", None], ["Empty", ""], ["ProcessName", "Chrome"]]),
    item("no-tags-dict.png", local("2026-11-17T10:05:00.0000000"), tags=[]),
    item("スクリーンショット.png", local("2026-11-16T11:00:00.0000000"), type_="image", host="Amazon S3",
         url="https://s3.example/スクリーンショット.png", tags=[["ProcessName", " chrome "]]),
]

# A spread of items for the queries and stats.
names = ["shot.png", "rec.mp4", "rec.gif", "notes.txt", "archive.tar.gz", "no_ext", "weird.name)", "dot.at.end.",
         "file.abcdefg", "file.é1", "PIC.JPG", "clip.WEBM", "a+b(1).png", "data.json", "x.Png", "share_me.png"]
types = ["Image", "File", "Text", "URL", "image", "IMAGE"]
hosts = [None, None, "Imgur", "imgur", "Amazon-S3", "amazons3", "Custom image uploader", "File", "Ünicode host",
         "a host", "B host", "_underscore", "10host", "2host", "  "]
processes = [None, "chrome", "Chrome", "explorer", "Code", "code", " firefox ", "notepad++", "ShareX", "zsh", "Ärger"]
days = ["2026-11-17", "2026-11-16", "2026-11-15", "2026-11-13", "2026-11-12", "2026-11-10", "2026-11-09",
        "2026-11-04", "2026-11-03", "2026-10-31", "2026-03-29", "2025-12-31", "2025-06-01", "2024-02-29",
        "2019-07-04", "2026-11-18", "2027-01-01"]
for i in range(70):
    name = rnd.choice(names)
    day = rnd.choice(days)
    t = f"{day}T{rnd.randrange(24):02}:{rnd.randrange(60):02}:{rnd.randrange(60):02}.{rnd.randrange(10**7):07}"
    host = rnd.choice(hosts)
    proc = rnd.choice(processes)
    tags = None
    if proc is not None:
        tags = [["WindowTitle", f"Window {i}"], ["ProcessName", proc]]
    if rnd.random() < 0.1:
        tags = (tags or []) + [["OCR", rnd.choice(["receipt total", "hello world", "chrome settings"])]]
    url = None if host is None or not host.strip() else f"https://{host.strip().replace(' ', '')}.example/{i}/{name}"
    write.append(item(name, local(t), type_=rnd.choice(types), host=host, url=url, tags=tags))

ops = [
    {"op": "favorite", "id": 1},
    {"op": "favorite", "id": 2},
    {"op": "favorite", "id": 1},
    {"op": "tag", "id": 1, "value": "work"},
    {"op": "tag", "id": 3, "value": "first tag"},
    {"op": "tag", "id": 2, "value": "x"},
    {"op": "tag", "id": 2, "value": ""},
    {"op": "favorite", "id": 20},
    {"op": "favorite", "id": 21},
    {"op": "favorite", "id": 30},
    {"op": "set_tags", "id": 22, "tags": [["Zeta", "1"], ["alpha", None], ["Favorite", None]]},
    {"op": "edit", "id": 4, "FileName": "renamed.png", "URL": None, "Host": None},
    {"op": "delete", "ids": [5, 6]},
    {"op": "delete", "ids": [len(write) - 3]},
    {"op": "append", "items": [item("after-delete.png", local("2026-11-17T11:00:00.0000000"))]},
]

raw_columns = "(FileName, FilePath, DateTime, Type, Host, URL, ThumbnailURL, DeletionURL, ShortenedURL, Tags)"


def raw(values):
    return f"INSERT INTO History {raw_columns} VALUES ({values});"


raw_rows = [
    raw("'tokyo.png', 'C:\\t.png', '2026-07-01T12:00:00.0000000+09:00', 'Image', 'Imgur', NULL, NULL, NULL, NULL, NULL"),
    raw("'zulu.png', 'C:\\z.png', '2026-01-05T10:00:00Z', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'none.png', 'C:\\n.png', '2026-01-05T10:00:00', 'Image', NULL, NULL, NULL, NULL, NULL, '{}'"),
    raw("'dateonly.png', 'C:\\d.png', '2026-01-05', 'Image', NULL, NULL, NULL, NULL, NULL, 'null'"),
    raw("'ms.png', 'C:\\m.png', '2026-07-05T10:00:00.123+02:00', 'Image', NULL, NULL, NULL, NULL, NULL, ''"),
    raw("'eight.png', 'C:\\e.png', '2026-07-05T10:00:00.12345675', 'Image', NULL, NULL, NULL, NULL, NULL, '  '"),
    raw("'eight2.png', 'C:\\e2.png', '2026-07-05T10:00:00.12345665', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'space.png', 'C:\\s.png', '2026-07-05 10:00:00', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'nocolon.png', 'C:\\c.png', '2026-07-05T10:00:00+0530', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'houronly.png', 'C:\\h.png', '2026-07-05T10:00:00-05', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'minutes.png', 'C:\\mi.png', '2026-07-05T10:00', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'ambig.png', 'C:\\a.png', '2025-10-26T01:30:00.0000000+01:00', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("'ambig2.png', 'C:\\a2.png', '2025-10-26T01:30:00.0000000+00:00', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("NULL, NULL, '2026-02-02T02:02:02.0000000Z', NULL, NULL, NULL, NULL, NULL, NULL, NULL"),
    raw("12345, 1.5, '2026-02-02T02:02:02.0000000Z', x'00ff', 1e20, NULL, NULL, NULL, NULL, NULL"),
    raw("'dup.png', 'C:\\dup.png', '2026-02-03T00:00:00.0000000Z', 'IMAGE', 'imgur', NULL, NULL, NULL, NULL,"
        " '{\"a\":\"1\",\"b\":\"2\",\"a\":\"3\"}'"),
    raw("'num.png', 'C:\\num.png', '2026-02-03T00:00:00.0000000Z', 'Image', 'IMGUR', NULL, NULL, NULL, NULL,"
        " '{\"n\":5,\"f\":1.5,\"t\":true,\"z\":null}'"),
    raw("'esc.png', 'C:\\esc.png', '2026-02-03T00:00:00.0000000Z', 'Image', NULL, NULL, NULL, NULL, NULL,"
        " '{\"k\":\"\\u00e9\\n\\\"\",\"ProcessName\":\"Ärger\"}'"),
]

raw_errors = {
    "null-date": raw("'x.png', 'C:\\x.png', NULL, 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    "bad-date": raw("'x.png', 'C:\\x.png', 'yesterday', 'Image', NULL, NULL, NULL, NULL, NULL, NULL"),
    "bad-tags": raw("'x.png', 'C:\\x.png', '2026-01-01T00:00:00Z', 'Image', NULL, NULL, NULL, NULL, NULL, '{'"),
}

json_items = [
    item("json1.png", local("2016-05-10T13:33:56.5556426"), host="Imgur", url="https://i.imgur.com/a.png",
         tags=[["ProcessName", "chrome"], ["Favorite", None]]),
    item("json2.txt", utc("2016-05-11T00:00:00.0000000"), type_="Text", url="https://p/x", path=None),
    item("json3.png", unspecified("2016-05-12T01:02:03.4000000"), tags=[["Odd", ODD]]),
    item("json4.png", utc_to_local("2015-10-25T00:30:00.0000000")),
]
json_items_2 = [
    item("json5.png", local("2016-06-01T12:00:00.0000000"), tags=[]),
    item("json6.png", None),  # invalid: skipped by AppendHistoryItems
]

json_raw = "\r\n".join([
    '{',
    '  "filename": "old1.png",',
    '  "FILEPATH": "C:\\\\Old\\\\old1.png",',
    '  "DateTime": "/Date(1462880036555)/",',
    '  "Type": "Image",',
    '  "Id": 99,',
    '  "Unknown": [1, 2],',
    '  "Tags": {"ProcessName": "explorer", "n": 5, "b": true, "z": null}',
    '},',
    '{',
    '  "FileName": "old2.png", "FilePath": "C:\\\\Old\\\\old2.png",',
    '  "DateTime": "/Date(1462880036555+0300)/", "URL": null, "Host": "Imgur"',
    '},',
    '{',
    '  "FileName": "old3.png", "FilePath": "C:\\\\Old\\\\old3.png",',
    '  "DateTime": "2016-05-10T13:33:56.5556426+03:00", "Type": "Image", "Type": "File"',
    '},',
    '{',
    '  "FileName": "old4.png", "FilePath": "C:\\\\Old\\\\old4.png",',
    '  "DateTime": "2016-05-10T13:33:56.123456789Z"',
    '},',
    '{',
    '  "FileName": "old5.png", "URL": "https://x/old5.png"',
    '}',
])

xml_raw = "\r\n".join([
    '<HistoryItem>',
    '    <Filename>xml1.png</Filename>',
    '    <Filepath>C:\\Old\\xml1.png</Filepath>',
    '    <DateTimeUtc>2012-01-02T03:04:05.0000000Z</DateTimeUtc>',
    '    <Type>Image</Type>',
    '    <Host>Imgur</Host>',
    '    <URL>http://i.imgur.com/a.png</URL>',
    '</HistoryItem>',
    '<HistoryItem><Filename>b &amp; c &#233;</Filename><DateTimeUtc>nope</DateTimeUtc><URL>   </URL>'
    '<Host>a<b>bold</b>c</Host><Type>Text</Type><Type>File</Type><Unknown>z</Unknown></HistoryItem>',
    '<Wrapper><HistoryItem><Filename>nested.png</Filename>'
    '<DateTimeUtc>2012-07-02T03:04:05.1234567+02:00</DateTimeUtc></HistoryItem></Wrapper>',
    '<HistoryItem><Filename> spaced </Filename><DateTimeUtc>2012-07-02T03:04:05</DateTimeUtc></HistoryItem>',
])


def date_range(f, t):
    return {"FromDate": local(f + "T00:00:00.0000000"), "ToDate": local(t + "T00:00:00.0000000")}


filters = [
    {},
    {"Filename": "*.png"},
    {"Filename": "share?_*"},
    {"Filename": "CHROME"},
    {"Filename": "CHROME", "SearchInTags": False},
    {"Filename": "a+b(1).png"},
    {"Filename": "é"},
    {"Filename": "スクリーン*"},
    {"Filename": "hello*"},
    {"Filename": "*"},
    {"URL": "IMGUR"},
    {"URL": "amazons3"},
    {"FilterType": True, "Type": "image"},
    {"FilterType": False, "Type": "Text"},
    {"FilterType": True, "Type": ""},
    {"FilterHost": True, "Host": "s3"},
    {"FilterHost": True, "Host": "amazons3"},
    {"FilterHost": True, "Host": "AMAZON-S3"},
    {"FilterHost": True, "Host": "ü"},
    {"FilterHost": True, "Host": "host"},
    dict(FilterDate=True, **date_range("2026-11-10", "2026-11-15")),
    dict(FilterDate=True, **date_range("2026-11-17", "2026-11-17")),
    dict(FilterDate=True, Filename="*.png", Host="imgur", FilterHost=True, **date_range("2020-01-01", "2026-12-31")),
    {"FilterFavorites": True, "Filename": "nothing-matches"},
    {"MaxItemCount": 5},
    {"MaxItemCount": 3, "Filename": "*.gif"},
]

image_filters = [
    {"search": "", "favorites": False, "media_only": True, "filter_missing": False},
    {"search": "", "favorites": False, "media_only": False, "filter_missing": False},
    {"search": " *.png ", "favorites": False, "media_only": True, "filter_missing": False},
    {"search": " chrome ", "favorites": False, "media_only": False, "filter_missing": False},
    {"search": "*", "favorites": False, "media_only": False, "filter_missing": False},
    {"search": "", "favorites": True, "media_only": True, "filter_missing": False},
    {"search": "", "favorites": False, "media_only": True, "filter_missing": True},
    {"search": "receipt", "favorites": False, "media_only": False, "filter_missing": False},
]

existing_files = ["C:\\Shots\\2026-11\\shot.png", "C:\\Shots\\2026-11\\rec.mp4", "C:\\Shots\\2026-11\\ShareX_abc.png"]

stats = [
    {"now": local("2026-11-17T12:00:00.0000000"), "first_day": 1},
    {"now": local("2026-11-17T12:00:00.0000000"), "first_day": 0},
    {"now": local("2026-11-14T23:59:59.9999999"), "first_day": 6},
    {"now": local("2030-01-01T00:00:00.0000000"), "first_day": 1},
    {"now": local("2020-01-01T00:00:00.0000000"), "first_day": 1},
]

formats = {
    "share": [[1, 8], [1, 40], [3, 8], [999, 1000], [1, 3], [0, 5], [5, 5], [1, 200], [1, 2000], [995, 1000],
              [9999, 10000], [1, 16], [7, 16], [199, 200], [1, 0]],
    "number": [0, 999, 1000, 1234567, -1234],
    "compact": [0, 999, 999.5, 1000, 1049, 1050, 1250, 1_250_000, 12.5, 0.5, 1.5, 2.5],
    "rate": [[0, 1], [1, 1], [3, 7], [100, 7], [1, 30], [26, 1], [105, 10], [25, 10], [1, 4], [15, 2], [99, 10],
             [995, 100], [5, 20], [1, 0]],
    "scale": [0, 1, 2, 3, 4, 7, 10, 19, 21, 99, 101, 1234],
    "level": [[0, [1, 2, 3]], [1, [1, 2, 3]], [3, [1, 2, 3]], [4, [1, 2, 3]], [2, [0, 0, 0]], [-1, [0, 0, 0]]],
    "week_change": [[5, 4], [3, 4], [4, 0], [0, 0], [7, 3], [1, 3], [5, 8], [3, 8]],
}

columns = [
    None,
    [],
    [{"Name": "URL", "Visible": False, "Width": 0}, {"Name": "Date", "Visible": False, "Width": 0},
     {"Name": "FileName", "Visible": False, "Width": 0}],
    [{"Name": "Date", "Visible": True, "Width": 10.5}, {"Name": "Icon", "Visible": False, "Width": 99},
     {"Name": "Bogus", "Visible": True, "Width": 99}, {"Name": "3", "Visible": True, "Width": 77},
     {"Name": "Tag", "Visible": True, "Width": 150.5}, {"Name": "Host", "Visible": True, "Width": 151.5},
     {"Name": "url", "Visible": False, "Width": 300}],
]

cases = {
    "write": write, "ops": ops, "raw": raw_rows, "raw_errors": raw_errors,
    "json_items": json_items, "json_items_2": json_items_2, "json_raw": json_raw, "xml_raw": xml_raw,
    "filters": filters, "image_filters": image_filters, "existing_files": existing_files,
    "stats": stats, "formats": formats, "columns": columns,
}

with open(sys.argv[1], "w", encoding="utf-8", newline="\n") as f:
    json.dump(cases, f, ensure_ascii=False, indent=1)
    f.write("\n")
