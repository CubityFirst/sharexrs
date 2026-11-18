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
import os
import re
import subprocess
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


AZURE_KEY = base64.b64decode(base64.b64encode(b"made-up-azure-key"))  # make_cases.py's


def split_target(target):
    """The rewritten target's (scheme, host, path, query)."""
    rest = target[len("/__/"):]
    scheme, rest = rest.split("/", 1)
    host, _, path_query = rest.partition("/")
    path, _, query = ("/" + path_query).partition("?")
    return scheme, host, path, query


def verify_azure(req):
    """Checks an Azure SharedKey signature against the request as it arrived."""
    h = {k.lower(): v for k, v in req["headers"]}
    m = re.match(r"SharedKey ([^:]+):(.+)$", h.get("authorization", ""))
    if not m:
        return f"malformed Authorization {h.get('authorization')!r}"
    account, signature = m.groups()
    _, host, path, _ = split_target(req["target"])
    body = base64.b64decode(req["body_base64"])
    canonical_headers = "".join(f"{k}:{v}\n" for k, v in sorted(h.items()) if k.startswith("x-ms-"))
    to_sign = (f"PUT\n\n\n{len(body)}\n\n{h.get('content-type', '')}\n\n\n\n\n\n\n"
               f"{canonical_headers}/{host.split('.')[0]}{path}")
    expected = base64.b64encode(hmac.new(AZURE_KEY, to_sign.encode(), hashlib.sha256).digest()).decode()
    if account != host.split(".")[0]:
        return "account doesn't match the host"
    return None if expected == signature else f"bad signature (expected {expected})"


BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_decode(text):
    n = 0
    for c in text:
        n = n * 58 + BASE58.index(c)
    raw = n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
    return b"\0" * (len(text) - len(text.lstrip("1"))) + raw


def privatebin_plain(body, url, password):
    """Decrypts a PrivateBin paste with the key after # in its URL."""
    try:
        payload = json.loads(body)
        iv, salt = base64.b64decode(payload["adata"][0][0]), base64.b64decode(payload["adata"][0][1])
        key_material = base58_decode(url.split("#", 1)[1]) + (password or "").encode()
        key = hashlib.pbkdf2_hmac("sha256", key_material, salt, 100000, 32)
        out = subprocess.run(["openssl", "enc", "-d", "-aes-256-cbc", "-K", key.hex(), "-iv", iv.hex()],
                             input=base64.b64decode(payload["ct"]), capture_output=True, check=True).stdout
        return out.decode("utf-8")
    except Exception as e:  # noqa: BLE001
        return f"undecryptable: {e}"


def norm_privatebin(req, result, case):
    """PrivateBin's random IV, salt and key: the decrypted paste instead."""
    body = base64.b64decode(req["body_base64"])
    try:
        payload = json.loads(body)
    except ValueError:
        return req
    if not isinstance(payload, dict) or "ct" not in payload:
        return req
    url = (result or {}).get("url") or ""
    password = ((case or {}).get("config") or {}).get("PrivateBinSettings", {}).get("PastePassword")
    plain = privatebin_plain(body, url, password) if "#" in url else "no key"
    for v in [payload["ct"], payload["adata"][0][0], payload["adata"][0][1]]:
        body = body.replace(json.dumps(v).encode(), b'"RANDOM"')
    req = dict(req, body_base64=base64.b64encode(body).decode(), headers=req["headers"] + [["x-privatebin-plain", plain]])
    return req


OAUTH1_CONSUMER_SECRET = "made-up-consumer-secret"  # make_cases.py's
OAUTH1_TOKEN_SECRETS = ["", "made-up-user-secret", "made-up-request-secret"]


def multipart_fields(body):
    """The form fields (not files) of a multipart body."""
    fields = []
    for part in re.split(rb"\r\n--[^\r\n]*\r\n|^--[^\r\n]*\r\n", body):
        head, sep, value = part.partition(b"\r\n\r\n")
        if not sep or b"filename=" in head:
            continue
        m = re.search(rb'name="([^"]*)"', head)
        if m:
            fields.append((m.group(1).decode(), value.decode("utf-8", "replace")))
    return fields


def verify_oauth1(req):
    """Checks an OAuth 1.0a HMAC-SHA1 signature (in the query or the form
    fields) with the made-up secrets. Returns an error text, or None."""
    from urllib.parse import parse_qsl, quote
    scheme, host, path, query = split_target(req["target"])
    params = parse_qsl(query, keep_blank_values=True)
    if not any(k == "oauth_signature" for k, _ in params):
        params = multipart_fields(base64.b64decode(req["body_base64"]))
    signature = dict(params).get("oauth_signature")
    if signature is None:
        return "no signature"
    params = [(k, v) for k, v in params if k != "oauth_signature"]
    enc = lambda t: quote(t, safe="-_.~")  # noqa: E731
    normalized = "&".join(f"{k}={enc(v)}" for k, v in sorted(params))
    # Photobucket signs api.photobucket.com, then sends to the subdomain;
    # Flickr signs getSizes as POST and sends a GET.
    for h in [host, "api.photobucket.com"]:
        for method in [req["method"], "POST", "GET"]:
            base = f"{method}&{enc(f'{scheme}://{h}{path}')}&{enc(normalized)}"
            for secret in OAUTH1_TOKEN_SECRETS:
                key = f"{enc(OAUTH1_CONSUMER_SECRET)}&{enc(secret)}".encode()
                if base64.b64encode(hmac.new(key, base.encode(), hashlib.sha1).digest()).decode() == signature:
                    return None
    return "bad OAuth 1.0a signature"


def norm_oauth1(req):
    """The nonce, time and signature of an OAuth 1.0a request (checked first)."""
    body = base64.b64decode(req["body_base64"])
    if "oauth_signature" not in req["target"] and b'name="oauth_signature"' not in body:
        return req
    error = verify_oauth1(req)
    target = re.sub(r"(oauth_(?:nonce|timestamp|signature)=)[^&]*", r"\1X", req["target"])
    body = re.sub(rb'(name="oauth_(?:nonce|timestamp|signature)"\r\n\r\n)[^\r]*', rb"\1X", body)
    headers = req["headers"] + ([["x-oauth1-check", error]] if error else [])
    return dict(req, target=target, body_base64=base64.b64encode(body).decode(), headers=headers)


def norm_request(req):
    req = norm_oauth1(req)
    headers = [[k, v] for k, v in req["headers"]]
    if any(k.lower() == "x-ms-date" for k, _ in headers):
        error = verify_azure(req)
        if error:
            headers.append(["x-azure-check", error])
        headers = [[k, "DATE" if k.lower() == "x-ms-date" else
                    re.sub(r":.+$", ":SIG", v) if k.lower() == "authorization" else v] for k, v in headers]
    # B2's file time.
    headers = [[k, "MILLIS" if k.lower() == "x-bz-info-src_last_modified_millis" else v] for k, v in headers]
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


def norm_oauth_text(text):
    """OAuth 1.0a's nonce, time and signature in a URL or error text."""
    if not isinstance(text, str):
        return text
    return re.sub(r"(oauth_(?:nonce|timestamp|signature)=)[^&\s]*", r"\1X", text)


def cut_stack(text):
    """An error text without its stack trace: ShareX's "Stack trace:" section
    and the "   at ..." lines of an exception's ToString()."""
    if text is None:
        return None
    text = norm_oauth_text(text).replace("\r\n", "\n")
    i = text.find("Stack trace:")
    if i >= 0:
        text = text[:i]
    return "\n".join(line for line in text.split("\n") if not line.startswith("   at "))


def compare_case(a, b, case=None):
    diffs = []
    ra, rb = a.get("requests", []), b.get("requests", [])
    if (case or {}).get("service", "").endswith(":PrivateBin"):
        ra = [norm_privatebin(r, a.get("result"), case) for r in ra]
        rb = [norm_privatebin(r, b.get("result"), case) for r in rb]
        for side in (a, b):
            if side.get("result") and "#" in (side["result"].get("url") or ""):
                side["result"]["url"] = side["result"]["url"].split("#")[0] + "#KEY"
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
    for k in ["check_config", "request_url", "written_file_base64", "steps", "auth"]:
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
            if norm_oauth_text(resa.get(k)) != norm_oauth_text(resb.get(k)):
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
    here = os.path.dirname(os.path.abspath(__file__))
    try:
        cases = {c["id"]: c for c in json.load(open(os.path.join(here, "cases.json"), encoding="utf-8"))["cases"]}
    except OSError:
        cases = {}
    rows, details, failed = [], [], 0
    for id, a in sharex.items():
        b = ours.get(id)
        if b is None:
            rows.append((id, "missing in ShareXrs", 0))
            failed += 1
            continue
        diffs, _ = compare_case(a, b, cases.get(id))
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
