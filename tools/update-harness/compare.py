"""Compares the update harness results of ShareX and ShareXrs.

  python compare.py <cases.json> <results-sharex.json> <results-sharexrs.json> [--report report.md]

Every field is compared (the user agent was already left out of the
recorded requests). ShareXrs's "error" (why a check failed) has no ShareX
counterpart and is ignored. Cases marked "addition" in cases.json are where
ShareXrs reads more than ShareX on purpose; they're listed apart.
"""

import json
import sys


def main():
    args = sys.argv[1:]
    report = None
    if "--report" in args:
        i = args.index("--report")
        report = args[i + 1]
        del args[i:i + 2]
    cases = {c["id"]: c for c in json.load(open(args[0], encoding="utf-8"))["cases"]}
    sharex = {r["id"]: r for r in json.load(open(args[1], encoding="utf-8"))}
    sharexrs = {r["id"]: r for r in json.load(open(args[2], encoding="utf-8"))}
    same, differ, additions = [], [], []
    for id, case in cases.items():
        a, b = sharex.get(id), sharexrs.get(id)
        if a is None or b is None:
            differ.append((id, "missing on one side"))
            continue
        b = {k: v for k, v in b.items() if k != "error"}
        diffs = [k for k in sorted(set(a) | set(b)) if a.get(k) != b.get(k)]
        if not diffs:
            same.append(id)
        elif case.get("addition"):
            additions.append((id, case["addition"], {k: (a.get(k), b.get(k)) for k in diffs}))
        else:
            differ.append((id, {k: (a.get(k), b.get(k)) for k in diffs}))
    lines = [f"# Update harness: {len(same)} identical, {len(differ)} differ, {len(additions)} additions", ""]
    for id, d in differ:
        lines.append(f"- DIFFER `{id}`: {json.dumps(d)[:2000]}")
    for id, why, d in additions:
        lines.append(f"- addition `{id}` ({why}): {json.dumps(d)[:600]}")
    text = "\n".join(lines) + "\n"
    print(text)
    if report:
        open(report, "w", encoding="utf-8").write(text)
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
