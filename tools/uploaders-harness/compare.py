"""Compares what ShareX and ShareXrs sent and made of the same responses.

Usage: python compare.py <results-sharex.json> <results-sharexrs.json> [--report report.md]

Requests are compared byte for byte after normalising only what has to
differ between two runs:
  - multipart boundaries (random GUIDs) become BOUNDARY,
  - the user agent's version (the .NET harness reports its own 1.0.0),
  - header name case (ureq writes lower case; HTTP names are
    case-insensitive). Header order is compared and reported separately.
Results: URL, thumbnail URL, deletion URL, shortened URL, success/error
flags, response, response URL, status, CheckConfig, the custom uploader's
request URL and .sxcu export, exceptions, and error texts up to their
"Stack trace:" line (a .NET stack trace can't match).
"""

import base64
import hashlib
import hmac
import json
import re
import sys

RESULT_KEYS = ["url", "thumbnail_url", "deletion_url", "shortened_url", "is_success", "is_error", "is_url_expected",
               "response", "response_url", "status_code"]


S3_SECRET = "made-up-secret-key"  # make_cases.py's


def verify_sigv4(req):
    """Checks an S3 request's AWS Signature V4 against the request as it
    arrived (the host comes from the rewritten target, as the client signed
    the real host). Returns an error text, or None."""
    h = {k.lower(): v for k, v in req["headers"]}
    m = re.match(r"AWS4-HMAC-SHA256 Credential=([^/]+)/(\d{8})/([^/]+)/s3/aws4_request,SignedHeaders=([^,]+),Signature=([0-9a-f]{64})$",
                 h.get("authorization", ""))
    if not m:
        return f"malformed Authorization {h.get('authorization')!r}"
    _, date, region, signed, signature = m.groups()
    target = req["target"]
    rest = target[len("/__/"):]
    scheme, rest = rest.split("/", 1)
    host, _, path_query = rest.partition("/")
    path, _, query = ("/" + path_query).partition("?")
    values = []
    for name in signed.split(";"):
        values.append(f"{name}:{(host if name == 'host' else h.get(name, '')).strip()}\n")
    body = base64.b64decode(req["body_base64"])
    payload = h.get("x-amz-content-sha256", "")
    if payload != "UNSIGNED-PAYLOAD" and payload != hashlib.sha256(body).hexdigest():
        return "x-amz-content-sha256 doesn't match the body"
    canonical = f"{req['method']}\n{path}\n{query}\n{''.join(values)}\n{signed}\n{payload}"
    amz_date = h.get("x-amz-date", "")
    scope = f"{date}/{region}/s3/aws4_request"
    to_sign = f"AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{hashlib.sha256(canonical.encode()).hexdigest()}"
    key = ("AWS4" + S3_SECRET).encode()
    for part in (date, region, "s3", "aws4_request"):
        key = hmac.new(key, part.encode(), hashlib.sha256).digest()
    expected = hmac.new(key, to_sign.encode(), hashlib.sha256).hexdigest()
    return None if expected == signature else f"bad signature (expected {expected})"


def norm_request(req):
    headers = [[k, v] for k, v in req["headers"]]
    if any(k.lower() == "x-amz-date" for k, _ in headers):
        error = verify_sigv4(req)
        if error:
            headers.append(["x-sigv4-check", error])
        headers = [[k, "DATE" if k.lower() == "x-amz-date" else
                    re.sub(r"/\d{8}/", "/DATE/", re.sub(r"Signature=[0-9a-f]+", "Signature=SIG", v)) if k.lower() == "authorization" else v]
                   for k, v in headers]
    body = base64.b64decode(req["body_base64"])
    for k, v in headers:
        m = re.search(r'boundary="?([^";]+)"?', v) if k.lower() == "content-type" else None
        if m:
            b = m.group(1)
            body = body.replace(b.encode(), b"BOUNDARY")
    # Immich sends a new GUID and the time with each upload.
    body = re.sub(rb"ShareX-[0-9a-f]{32}", b"ShareX-GUID", body)
    # Services that take the user agent as a form field (puush's "z").
    body = re.sub(rb"ShareX(rs)?/[0-9.]+", b"ShareX/VERSION", body)
    body = re.sub(rb"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{7}\+00:00", b"TIMESTAMP", body)
    norm = []
    for k, v in headers:
        v = re.sub(r'boundary="?[^";]+"?', 'boundary="BOUNDARY"', v) if k.lower() == "content-type" else v
        if k.lower() == "user-agent":
            # ShareXrs identifies as itself (ShareXrs/<version>) where ShareX
            # sends ShareX/<version>: the one intended difference.
            v = re.sub(r"^ShareX(rs)?/[0-9.]+", "ShareX/VERSION", v)
        if k.lower() == "content-length":
            # From the normalised body: the user agent in a body is longer
            # in ShareXrs ("ShareXrs/"), and nothing else may differ.
            v = str(len(body))
        norm.append((k.lower(), v))
    return {"method": req["method"], "target": req["target"], "version": req["version"], "chunked": req["chunked"],
            "headers": norm, "body": body}


def cut_stack(text):
    """An error text without its stack trace: ShareX's "Stack trace:" section
    and the "   at ..." lines of an exception's ToString()."""
    if text is None:
        return None
    text = text.replace("\r\n", "\n")
    i = text.find("Stack trace:")
    if i >= 0:
        text = text[:i]
    return "\n".join(line for line in text.split("\n") if not line.startswith("   at "))


def compare_case(a, b):
    diffs = []
    ra, rb = a.get("requests", []), b.get("requests", [])
    if len(ra) != len(rb):
        diffs.append(f"request count: sharex {len(ra)} vs sharexrs {len(rb)}")
    order_only = False
    for i, (qa, qb) in enumerate(zip(ra, rb)):
        na, nb = norm_request(qa), norm_request(qb)
        for k in ["method", "target", "version", "chunked"]:
            if na[k] != nb[k]:
                diffs.append(f"request {i} {k}: sharex {na[k]!r} vs sharexrs {nb[k]!r}")
        if sorted(na["headers"]) != sorted(nb["headers"]):
            sa, sb = set(na["headers"]), set(nb["headers"])
            diffs.append(f"request {i} headers: only sharex {sorted(sa - sb)}, only sharexrs {sorted(sb - sa)}")
        elif na["headers"] != nb["headers"]:
            order_only = True
            diffs.append(f"request {i} header order: sharex {[h[0] for h in na['headers']]} vs sharexrs {[h[0] for h in nb['headers']]}")
        if na["body"] != nb["body"]:
            j = next((k for k in range(min(len(na['body']), len(nb['body']))) if na['body'][k] != nb['body'][k]),
                     min(len(na['body']), len(nb['body'])))
            diffs.append(f"request {i} body differs at byte {j} (lengths {len(na['body'])}/{len(nb['body'])}): "
                         f"sharex {na['body'][max(0, j - 40):j + 40]!r} vs sharexrs {nb['body'][max(0, j - 40):j + 40]!r}")
    for k in ["check_config", "request_url"]:
        if a.get(k) != b.get(k):
            diffs.append(f"{k}: sharex {a.get(k)!r} vs sharexrs {b.get(k)!r}")
    if (a.get("sxcu_export") or "").replace("\r\n", "\n") != (b.get("sxcu_export") or "").replace("\r\n", "\n"):
        diffs.append(f"sxcu_export:\n--- sharex\n{a.get('sxcu_export')}\n--- sharexrs\n{b.get('sxcu_export')}")
    if a.get("exception") != b.get("exception"):
        diffs.append(f"exception: sharex {a.get('exception')!r} vs sharexrs {b.get('exception')!r}")
    resa, resb = a.get("result"), b.get("result")
    if (resa is None) != (resb is None):
        diffs.append(f"result: sharex {resa!r} vs sharexrs {resb!r}")
    elif resa is not None:
        for k in RESULT_KEYS:
            if resa.get(k) != resb.get(k):
                diffs.append(f"result.{k}: sharex {resa.get(k)!r} vs sharexrs {resb.get(k)!r}")
        ea = [cut_stack(e) for e in resa.get("errors", [])]
        eb = [cut_stack(e) for e in resb.get("errors", [])]
        if ea != eb:
            diffs.append(f"result.errors: sharex {ea!r} vs sharexrs {eb!r}")
    ea = [cut_stack(e) for e in a.get("uploader_errors", [])]
    eb = [cut_stack(e) for e in b.get("uploader_errors", [])]
    if ea != eb:
        diffs.append(f"uploader errors:\n  sharex   {ea!r}\n  sharexrs {eb!r}")
    return diffs, order_only


def main():
    sharex = {c["id"]: c for c in json.load(open(sys.argv[1], encoding="utf-8"))}
    ours = {c["id"]: c for c in json.load(open(sys.argv[2], encoding="utf-8"))}
    report = sys.argv[sys.argv.index("--report") + 1] if "--report" in sys.argv else None
    rows, details, failed = [], [], 0
    for id, a in sharex.items():
        b = ours.get(id)
        if b is None:
            rows.append((id, "missing in ShareXrs", 0))
            failed += 1
            continue
        diffs, _ = compare_case(a, b)
        nreq = len(a.get("requests", []))
        if diffs:
            failed += 1
            rows.append((id, f"DIFF ({len(diffs)})", nreq))
            details.append(f"### {id}\n" + "\n".join(f"- {d}" for d in diffs))
        else:
            rows.append((id, "identical", nreq))
    lines = ["| Case | Requests | Result |", "|---|---|---|"] + [f"| {i} | {n} | {s} |" for i, s, n in rows]
    summary = f"{len(rows) - failed}/{len(rows)} cases identical"
    out = "\n".join([summary, "", *lines, "", *details])
    print(out)
    if report:
        with open(report, "w", encoding="utf-8", newline="\n") as f:
            f.write(out + "\n")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
