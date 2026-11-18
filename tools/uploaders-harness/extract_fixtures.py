"""Writes the fixtures ShareXrs's cargo tests compare against, from
ShareX's results (target/uploaders-harness/results-sharex.json):
crates/sharex-uploaders/tests/fixtures/servers.sharex.json holds, for each
case run against the FTP/SFTP/SMTP mocks, what ShareX's own code did there
(the server's log and files) and its result (for remote storage browser
cases, each step's value).

Usage: python extract_fixtures.py <results-sharex.json>
"""

import json
import os
import sys



def strip_types(v):
    """Without .NET's exception types (only the messages compare)."""
    if isinstance(v, dict):
        return {k: strip_types(x) for k, x in v.items() if k != "exception_type"}
    if isinstance(v, list):
        return [strip_types(x) for x in v]
    return v


here = os.path.dirname(os.path.abspath(__file__))
results = json.load(open(sys.argv[1], encoding="utf-8"))
out = {}
for r in results:
    if "servers" not in r:
        continue
    res = r.get("result") or {}
    entry = {"servers": r["servers"], "url": res.get("url"), "is_url_expected": res.get("is_url_expected"),
             "exception": r.get("exception"), "uploader_errors": r.get("uploader_errors", [])}
    # Remote storage browser cases: the provider's names and each step.
    for k in ("steps", "create", "display_name", "root_display_name", "root_path", "capabilities"):
        if k in r:
            entry[k] = strip_types(r[k])
    out[r["id"]] = entry
path = os.path.join(here, "..", "..", "crates", "sharex-uploaders", "tests", "fixtures", "servers.sharex.json")
with open(path, "w", encoding="utf-8", newline="\n") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)
    f.write("\n")
print(f"{len(out)} cases -> {os.path.normpath(path)}")
