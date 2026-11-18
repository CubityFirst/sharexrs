"""Writes the fixtures ShareXrs's cargo tests compare against, from
ShareX's results (target/uploaders-harness/results-sharex.json):
crates/sharex-uploaders/tests/fixtures/servers.sharex.json holds, for each
case run against the FTP/SFTP/SMTP mocks, what ShareX's own code did there
(the server's log and files) and its result.

Usage: python extract_fixtures.py <results-sharex.json>
"""

import json
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
results = json.load(open(sys.argv[1], encoding="utf-8"))
out = {}
for r in results:
    if "servers" not in r:
        continue
    res = r.get("result") or {}
    out[r["id"]] = {"servers": r["servers"], "url": res.get("url"), "is_url_expected": res.get("is_url_expected"),
                    "exception": r.get("exception"), "uploader_errors": r.get("uploader_errors", [])}
path = os.path.join(here, "..", "..", "crates", "sharex-uploaders", "tests", "fixtures", "servers.sharex.json")
with open(path, "w", encoding="utf-8", newline="\n") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
    f.write("\n")
print(f"{len(out)} cases -> {os.path.normpath(path)}")
