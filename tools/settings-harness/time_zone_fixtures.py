"""Writes the time zone fixtures (crates/sharex-helpers/tests/fixtures/time_zones/)
from SettingsHarness --time-zones runs: the machine's list as .NET has it
(ids, names, ShareX's JSON, UTC offsets), and for each other local zone only
what loading the JSON as ShareX does changes (the local zone's JSON, which
list item a reloaded zone selects, its offsets where they differ).

usage: time_zone_fixtures.py <out dir> <base run> [<local run>...]
"""
import json
import sys


def main():
    out, base, *locals_ = sys.argv[1:]
    d = json.load(open(base, encoding="utf-8"))
    zones = [{"id": z["id"], "displayName": z["displayName"], "json": z["json"], "transitions": z["transitions"]} for z in d["zones"]]
    write(f"{out}/time-zones-sharex.json", {"machine": d["machine"], "local": d["local"], "utc": d["utc"], "zones": zones})
    for path in locals_:
        v = json.load(open(path, encoding="utf-8"))
        loaded = []
        for z in v["zones"]:
            L = z["loaded"]
            item = {"id": z["id"], "selects": L.get("selects"), "sameJson": L.get("sameJson")}
            if not L.get("sameJson"):
                item["json"] = L["json"]
            if L.get("transitions") is not None:
                item["transitions"] = L["transitions"]
            if "error" in L:
                item["error"] = L["error"] + ": " + L["message"]
            loaded.append(item)
        name = json.loads(v["local"])["Id"].replace(" ", "-").lower()
        write(f"{out}/time-zones-loaded-{name}.json", {"machine": v["machine"], "local": v["local"], "zones": loaded})


def write(path, obj):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(obj, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")
    print(path)


main()
