"""Writes cases.json for the update harness (README.md).

Each "check" case is one GitHubUpdateChecker run against canned GitHub API
responses; "download" cases run FileDownloader; "version" cases parse and
compare versions. Everything is made up; the URLs are GitHub's shapes, and
both harnesses rewrite them to the local mock server before sending.
"""

import base64
import json
import sys

OWNER, REPO = "CubityFirst", "sharexrs"
API = f"https://api.github.com/repos/{OWNER}/{REPO}"

ALL_ASSETS = [
    "ShareXrs-{v}-setup-x64.exe",
    "ShareXrs-{v}-setup-arm64.exe",
    "ShareXrs-{v}-portable-x64.zip",
    "ShareXrs-{v}-portable-arm64.zip",
    "ShareXrs-macos-aarch64.dmg",
    "ShareXrs-macos-aarch64.app.zip",
    "ShareXrs-{v}-linux-x64.deb",
    "ShareXrs-{v}-linux-x64.tar.gz",
    "SHA256SUMS",
]


def asset(i, name, tag):
    return {
        "url": f"{API}/releases/assets/{1000 + i}",
        "id": 1000 + i,
        "node_id": f"RA_{i}",
        "name": name,
        "label": None,
        "uploader": {"login": "someone"},
        "content_type": "application/octet-stream",
        "state": "uploaded",
        "size": 1234 + i,
        "download_count": 7,
        "created_at": "2026-11-01T10:00:00Z",
        "updated_at": "2026-11-01T10:00:05Z",
        "browser_download_url": f"https://github.com/{OWNER}/{REPO}/releases/download/{tag}/{name}",
    }


def release(tag, prerelease=False, published="2026-11-01T10:00:00Z", assets=None, rid=1):
    version = tag[1:] if tag.startswith("v") else tag
    names = [n.format(v=version) for n in (ALL_ASSETS if assets is None else assets)]
    return {
        "url": f"{API}/releases/{rid}",
        "assets_url": f"{API}/releases/{rid}/assets",
        "upload_url": f"https://uploads.github.com/repos/{OWNER}/{REPO}/releases/{rid}/assets{{?name,label}}",
        "html_url": f"https://github.com/{OWNER}/{REPO}/releases/tag/{tag}",
        "id": rid,
        "author": {"login": "someone"},
        "node_id": f"RE_{rid}",
        "tag_name": tag,
        "target_commitish": "master",
        "name": f"ShareXrs {version}",
        "draft": False,
        "prerelease": prerelease,
        "created_at": published,
        "published_at": published,
        "assets": [asset(i, n, tag) for i, n in enumerate(names)],
        "tarball_url": f"{API}/tarball/{tag}",
        "zipball_url": f"{API}/zipball/{tag}",
        "body": "Changes",
    }


def ok(body, content_type="application/json; charset=utf-8"):
    text = body if isinstance(body, str) else json.dumps(body)
    return {"status": 200, "reason": "OK", "headers": [["Content-Type", content_type]], "body": text}


def status(code, reason, body=""):
    return {"status": code, "reason": reason, "headers": [["Content-Type", "application/json; charset=utf-8"]], "body": body}


cases = []


def check(id, response, portable=False, pre=False, ignore_revision=True, dev=False, current="22.4.0.0", addition=None):
    case = {
        "id": id,
        "kind": "check",
        "portable": portable,
        "include_pre_release": pre,
        "ignore_revision": ignore_revision,
        "is_dev": dev,
        "current": current,
        "responses": [response],
    }
    if addition:
        case["addition"] = addition
    cases.append(case)


# Up to date, newer, older, revisions.
check("latest_same", ok(release("v22.4.0")))
check("latest_newer_installed", ok(release("v22.5.0")))
check("latest_newer_portable", ok(release("v22.5.0")), portable=True)
check("latest_older", ok(release("v22.3.9")))
check("latest_minor_10", ok(release("v22.10.0")))
check("latest_two_parts", ok(release("v23.0")))
check("latest_four_parts_revision_ignored", ok(release("v22.4.0.3")))
check("latest_four_parts_revision_counted", ok(release("v22.4.0.3")), ignore_revision=False)
check("current_two_parts", ok(release("v22.4.1")), current="22.4")

# Channels.
unsorted = [
    release("v22.5.0", published="2026-11-01T10:00:00Z", rid=3),
    release("v22.6.0", prerelease=True, published="2026-11-10T10:00:00Z", rid=4),
    release("v22.4.0", published="2026-10-01T10:00:00Z", rid=2),
]
check("prerelease_channel_newest_published", ok(unsorted), pre=True)
check("prerelease_channel_portable", ok(unsorted), pre=True, portable=True)
check("release_channel_latest", ok(release("v22.5.0")), pre=False)
check(
    "prerelease_channel_semver_tag",
    ok([release("v22.6.0-beta.1", prerelease=True, published="2026-11-10T10:00:00Z"), release("v22.5.0")]),
    pre=True,
    addition="ShareXrs reads SemVer pre-release tags (v22.6.0-beta.1), which new Version rejects",
)
check(
    "latest_semver_build_metadata",
    ok(release("v22.5.0+build.7")),
    addition="ShareXrs drops SemVer build metadata, which new Version rejects",
)
# Many releases, several published at the same moment: List.Sort's introsort
# order decides between them (more than 16 runs past insertion sort).
many = []
for i in range(40):
    day = 1 + (i * 7) % 12
    many.append(release(f"v22.{i}.0", published=f"2026-10-{day:02d}T10:00:00Z", rid=100 + i))
check("prerelease_channel_ties_40", ok(many), pre=True)
few = [release(f"v22.{i}.0", published="2026-10-05T10:00:00Z", rid=200 + i) for i in range(5)]
check("prerelease_channel_ties_5", ok(few), pre=True)
check("prerelease_channel_offsets", ok([
    release("v22.5.0", published="2026-11-01T12:00:00+02:00", rid=5),
    release("v22.6.0", published="2026-11-01T11:00:00Z", rid=6),
]), pre=True)
check("prerelease_channel_empty_list", ok([]), pre=True)
check("prerelease_channel_null_element", ok("[null," + json.dumps(release("v22.5.0")) + "]"), pre=True)
check("prerelease_channel_single_null", ok("[null]"), pre=True)
check("prerelease_channel_object", ok(release("v22.5.0")), pre=True)

# Dev builds (TaskHelpers.DownloadDevBuild: IsDev, revision kept).
check("dev_newer_revision", ok(release("v22.4.0.1")), dev=True, ignore_revision=False)
check("dev_same", ok(release("v22.4.0.0")), dev=True, ignore_revision=False)

# Assets.
check("missing_asset", ok(release("v22.5.0", assets=["ShareXrs-macos-aarch64.dmg", "SHA256SUMS"])))
check("missing_asset_portable", ok(release("v22.5.0", assets=["ShareXrs-{v}-setup-x64.exe"])), portable=True)
check("fallback_setup_exe", ok(release("v22.5.0", assets=["notes.txt", "ShareXrs-setup.exe"])))
check("fallback_portable_zip", ok(release("v22.5.0", assets=["ShareXrs-portable.zip", "x.zip"])), portable=True)
check("asset_order_reversed", ok(release("v22.5.0", assets=list(reversed(ALL_ASSETS)))), portable=True)
check("asset_name_upper_case", ok(release("v22.5.0", assets=["SHAREXRS-22.5.0-SETUP-X64.EXE"])))
check("asset_x64_suffix_only", ok(release("v22.5.0", assets=["x64.exe", "setup-x64.exe"])))
r = release("v22.5.0")
r["assets"] = None
check("assets_null", ok(r))
r = release("v22.5.0")
r["assets"].insert(0, None)
check("asset_null_element", ok(r))
r = release("v22.5.0")
for a in r["assets"]:
    del a["browser_download_url"]
check("portable_no_browser_url", ok(r), portable=True)
r = release("v22.5.0")
for a in r["assets"]:
    a["url"] = ""
check("installed_empty_api_url", ok(r))

# Tags.
for i, tag in enumerate(["22.5.0", "v", "vNext", "v22", "v 22.5.0", "v+22.5.0", "v22.5.0.0.1", "v22.-1.0", "v22.5.99999999999", "V22.5.0", "v022.05.000"]):
    check(f"tag_{i}_" + "".join(c if c.isalnum() else "_" for c in tag), ok(release(tag)))
r = release("v22.5.0")
r["tag_name"] = None
check("tag_null", ok(r))
r = release("v22.5.0")
r["tag_name"] = 225
check("tag_number", ok(r))

# Newtonsoft's reading.
r = release("v22.5.0")
check("property_names_any_case", ok(json.dumps(r).replace('"tag_name"', '"TAG_NAME"').replace('"assets"', '"Assets"')))
r = release("v22.5.0", prerelease=True)
check("prerelease_flag_latest", ok(r))
r = release("v22.5.0")
r["prerelease"] = "true"
check("prerelease_as_string", ok(r))
r = release("v22.5.0")
r["prerelease"] = None
check("prerelease_null", ok(r))
r = release("v22.5.0")
r["published_at"] = None
check("published_at_null", ok(r))
r = release("v22.5.0")
r["id"] = "5"
check("id_as_string", ok(r))
r = release("v22.5.0")
r["assets"][0]["size"] = None
check("asset_size_null", ok(r))
r = release("v22.5.0")
r["draft"] = 1
check("draft_as_number", ok(r))
check("utf8_bom", ok("\ufeff" + json.dumps(release("v22.5.0"))))
check("malformed_json", ok('{"tag_name": "v22.5.0", '))
check("html_error_page", ok("<html>busy</html>", "text/html"))
check("trailing_garbage", ok(json.dumps(release("v22.5.0")) + " x"))
check("latest_is_array", ok([release("v22.5.0")]))
check("latest_null", ok("null"))
check("latest_empty_body", ok(""))
check("latest_number", ok("42"))

# HTTP errors.
check("http_404", status(404, "Not Found", '{"message":"Not Found"}'))
check("http_403_rate_limit", status(403, "Forbidden", '{"message":"API rate limit exceeded"}'))
check("http_500", status(500, "Internal Server Error"))
check("http_404_prerelease", status(404, "Not Found", "{}"), pre=True)

# Downloads (FileDownloader, as the downloader window runs it).
payload = bytes((i * 7 + 3) % 256 for i in range(150_000))
b64 = base64.b64encode(payload).decode()


def download(id, response, accept="application/octet-stream", url=f"{API}/releases/assets/1000"):
    cases.append({"id": id, "kind": "download", "url": url, "accept": accept, "responses": [response]})


download("download_ok", {"status": 200, "reason": "OK", "headers": [["Content-Type", "application/octet-stream"]], "body_base64": b64})
download("download_browser_url", {"status": 200, "reason": "OK", "headers": [], "body_base64": b64}, accept="",
         url=f"https://github.com/{OWNER}/{REPO}/releases/download/v22.5.0/ShareXrs-22.5.0-portable-x64.zip")
download("download_small", {"status": 200, "reason": "OK", "headers": [], "body_base64": base64.b64encode(b"abc").decode()})
download("download_empty", {"status": 200, "reason": "OK", "headers": [], "body": ""})
download("download_404", status(404, "Not Found", '{"message":"Not Found"}'))
download("download_500", status(500, "Internal Server Error"))

# Versions (System.Version and Helpers.CompareVersion).
versions = ["22.4.0", "22.4.0.0", "1.2", "1", "", "1.2.3.4.5", " 1 . 2 ", "+1.2", "-0.1", "-1.2", "1..2", "a.b",
            "1.2.3.4", "2147483647.0", "2147483648.0", "01.002.0003", "1.2-beta", "1.2.", "\t1.2"]
pairs = [("22.4.0.0", "22.4.0", True), ("22.4.0.0", "22.4.0.1", True), ("22.4.0.0", "22.4.0.1", False),
         ("1.2", "1.2.0", False), ("1.2", "1.2.0.0", True), ("22.10.0", "22.9.9", True), ("3.0", "22.0", True)]
cases.append({"id": "versions", "kind": "version", "parse": versions, "compare": [list(p) for p in pairs]})

assert len({c["id"] for c in cases}) == len(cases), "duplicate case ids"
json.dump({"cases": cases}, open(sys.argv[1] if len(sys.argv) > 1 else "cases.json", "w", encoding="utf-8"), indent=1)
print(f"{len(cases)} cases")
