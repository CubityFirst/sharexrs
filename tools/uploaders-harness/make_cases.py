"""Writes cases.json: what both harnesses upload, with which settings, and the
canned responses the mock server answers with.

Every value is made up: no real keys, accounts or personal data. URLs point
at real services' hosts only so the requests match ShareX's; the harnesses
rewrite them to the local mock server before anything is sent.

Usage: python make_cases.py  (writes cases.json beside it)
"""

import base64
import hashlib
import json
import os
import subprocess

# Every byte value, so multipart bodies show any encoding slip.
FILE_BYTES = bytes(range(256)) * 3
FILE_B64 = base64.b64encode(FILE_BYTES).decode()
TEXT = 'Hello "world" & <friends>\nline 2 \\ tab\t é ü 🙂  '
URL = "https://example.com/some/long/path?a=1&b=two words"


def resp(body="", status=200, headers=None, reason=None):
    r = {"status": status, "headers": headers or [["Content-Type", "application/json; charset=utf-8"]], "body": body}
    if reason:
        r["reason"] = reason
    return r


def text_resp(body, status=200):
    return resp(body, status, [["Content-Type", "text/plain; charset=utf-8"]])


def image_input(name="test image.png"):
    return {"file_name": name, "content_base64": FILE_B64}


def text_input(name="notes.txt", text=TEXT):
    return {"file_name": name, "text": text}


def url_input(url=URL):
    return {"url": url}


cases = []


def custom(id, dest, sxcu, inp, responses):
    cases.append({"id": id, "kind": "custom", "dest": dest, "sxcu": sxcu, "input": inp, "responses": responses})


def builtin(id, service, config, inp, responses):
    cases.append({"id": id, "kind": "builtin", "service": service, "config": config, "input": inp, "responses": responses})


JSON_OK = json.dumps({"data": {"link": "https://i.example.com/abc.png", "thumb": "https://i.example.com/abc_t.png",
                               "del": "https://example.com/delete/xyz", "n": 3, "f": 1.5, "b": True}})

# ---------------------------------------------------------------------------
# Custom uploaders: one case per body type and per syntax function.

custom("custom-image-multipart", "image", {
    "Version": "17.0.0",
    "Name": "Mock image host",
    "DestinationType": "ImageUploader, FileUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/api/{filename}/upload",
    "Parameters": {"key": "value with space", "fn": "{filename}"},
    "Headers": {"Authorization": "Bearer made-up-token", "X-Custom": "{filename}"},
    "Body": "MultipartFormData",
    "Arguments": {"name": "{filename}", "literal": "a&b=c", "empty": ""},
    "FileFormName": "file",
    "URL": "{json:data.link}",
    "ThumbnailURL": "{json:$.data.thumb}",
    "DeletionURL": "{json:data.del}",
    "ErrorMessage": "{json:error}",
}, image_input(), [resp(JSON_OK)])

custom("custom-file-binary-put", "file", {
    "Version": "17.0.0",
    "DestinationType": "FileUploader",
    "RequestMethod": "PUT",
    "RequestURL": "https://files.example.com/put/{filename}",
    "Headers": {"X-Api-Key": "made-up"},
    "Body": "Binary",
    "URL": "https://files.example.com/{json:id}",
}, {"file_name": "archive.zip", "content_base64": FILE_B64}, [resp('{"id":"f-123"}')])

custom("custom-text-json", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/api",
    "Body": "JSON",
    "Data": "{\"content\":\"{input}\",\"name\":\"{filename}\",\"lang\":\"text\"}",
    "URL": "{response}",
}, text_input(), [text_resp("https://paste.example.com/p/1")])

custom("custom-text-xml", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/xml",
    "Body": "XML",
    "Data": "<paste><content>{input}</content><name>{filename}</name></paste>",
    "URL": "{xml:/result/url}",
    "DeletionURL": "{xml:{response}|//delete}",
}, text_input(), [resp("<result><url>https://paste.example.com/x/2</url><delete>https://paste.example.com/d/2</delete></result>",
                       headers=[["Content-Type", "application/xml"]])])

custom("custom-text-form-urlencoded", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/form",
    "Body": "FormURLEncoded",
    "Arguments": {"text": "{input}", "title": "{filename}", "flag": ""},
    "URL": "{regex:href=\"(.+?)\"|1}",
}, text_input(), [resp('<a href="https://paste.example.com/r/3">link</a>', headers=[["Content-Type", "text/html"]])])

custom("custom-text-multipart-args", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/multi",
    "Body": "MultipartFormData",
    "Arguments": {"content": "{input}", "file name": "{filename}"},
    "URL": "{json:url}",
}, text_input(), [resp('{"url":"https://paste.example.com/m/4"}')])

custom("custom-text-multipart-file", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/multifile",
    "Body": "MultipartFormData",
    "Arguments": {"expires": "1d"},
    "FileFormName": "upload",
    "URL": "{json:url}",
}, text_input("script.py"), [resp('{"url":"https://paste.example.com/m/5"}')])

custom("custom-text-binary", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/raw",
    "Body": "Binary",
}, text_input("page.html"), [text_resp("https://paste.example.com/raw/6")])

custom("custom-text-get-no-body", "text", {
    "Version": "17.0.0",
    "DestinationType": "TextUploader",
    "RequestMethod": "GET",
    "RequestURL": "https://paste.example.com/get",
    "Parameters": {"text": "{input}", "name": "{filename}"},
    "Body": "None",
}, text_input(), [text_resp("https://paste.example.com/g/7")])

custom("custom-url-shortener", "url_shortener", {
    "Version": "17.0.0",
    "DestinationType": "URLShortener",
    "RequestMethod": "GET",
    "RequestURL": "https://short.example.com/api",
    "Parameters": {"url": "{input}", "format": "json"},
    "Body": "None",
    "URL": "{json:short}",
}, url_input(), [resp('{"short":"https://s.example/abc"}')])

custom("custom-url-shortener-json-patch", "url_shortener", {
    "Version": "17.0.0",
    "DestinationType": "URLShortener",
    "RequestMethod": "PATCH",
    "RequestURL": "https://short.example.com/v2/links",
    "Headers": {"Content-Type": "application/vnd.example+json"},
    "Body": "JSON",
    "Data": "{\"target\":\"{input}\"}",
    "URL": "{json:link}",
}, url_input(), [resp('{"link":"https://s.example/def"}')])

custom("custom-url-sharing", "url_sharing", {
    "Version": "17.0.0",
    "DestinationType": "URLSharingService",
    "RequestMethod": "POST",
    "RequestURL": "https://share.example.com/share",
    "Body": "MultipartFormData",
    "Arguments": {"url": "{input}"},
}, url_input(), [resp('{"ok":true}')])

custom("custom-functions", "image", {
    "Version": "17.0.0",
    "DestinationType": "ImageUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/fn",
    "Headers": {"Authorization": "Basic {base64:user:pa:ss}", "X-Pick": "{random:same|same}"},
    "Body": "MultipartFormData",
    "FileFormName": "f",
    "URL": "{header:location}",
    "ThumbnailURL": "{responseurl}|{regex:(?<=href=\")[^\"]+(?=\")}",
    "DeletionURL": "{json:{response}|$.nested.items[1].id}/{json:nested.n}/{json:nested.f}/{json:nested.b}/{json:nested.missing}",
}, image_input("fn.jpg"), [resp(json.dumps({"nested": {"items": [{"id": "a"}, {"id": "b"}], "n": 42, "f": 2.5, "b": False},
                                         "html": "<a href=\"https://x.example/1\">"}),
                                headers=[["Content-Type", "application/json"], ["Location", "https://upload.example.com/i/loc"],
                                         ["X-Multi", "one"], ["X-Multi", "two"]])])

custom("custom-escapes-and-unicode", "file", {
    "Version": "17.0.0",
    "DestinationType": "FileUploader",
    "RequestMethod": "POST",
    "RequestURL": "upload.example.com/u/\\{literal\\}/{filename}",
    "Parameters": {"q": "ä ö {filename}"},
    "Body": "MultipartFormData",
    "Arguments": {"ünïcödé": "välüé \"quoted\"", "pipe": "a\\|b"},
    "FileFormName": "fïle",
    "URL": "{json:url}",
}, {"file_name": "fïlé \"quoted\" näme.txt", "content_base64": FILE_B64}, [resp('{"url":"https://u.example/ü"}')])

custom("custom-error-message", "image", {
    "Version": "17.0.0",
    "DestinationType": "ImageUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/err",
    "Body": "MultipartFormData",
    "FileFormName": "file",
    "URL": "{json:url}",
    "ErrorMessage": "{json:error.message}",
}, image_input(), [resp('{"error":{"message":"File too large"}}', 413, reason="Payload Too Large")])

custom("custom-invalid-json", "image", {
    "Version": "17.0.0",
    "DestinationType": "ImageUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/bad",
    "Body": "MultipartFormData",
    "FileFormName": "file",
    "URL": "{json:url}",
}, image_input(), [resp("<html>not json</html>", headers=[["Content-Type", "text/html"]])])

custom("custom-empty-response", "image", {
    "Version": "17.0.0",
    "DestinationType": "ImageUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/empty",
    "Body": "MultipartFormData",
    "FileFormName": "file",
}, image_input(), [resp("", headers=[])])

custom("custom-redirect", "url_shortener", {
    "Version": "17.0.0",
    "DestinationType": "URLShortener",
    "RequestMethod": "GET",
    "RequestURL": "https://short.example.com/redir",
    "Parameters": {"u": "{input}"},
    "Body": "None",
    "URL": "{responseurl}",
}, url_input(), [resp("", 302, [["Location", "{base}/__/https/short.example.com/final?x=1"]]), text_resp("done")])

custom("custom-old-syntax-13", "image", {
    "Version": "13.1.0",
    "DestinationType": "ImageUploader",
    "RequestMethod": "POST",
    "RequestURL": "https://upload.example.com/old?key=abc&flag",
    "Body": "MultipartFormData",
    "Arguments": {"name": "$filename$"},
    "FileFormName": "file",
    "URL": "$json:data.link$",
    "ThumbnailURL": "{literal braces}",
}, image_input(), [resp(JSON_OK)])

custom("custom-unsupported-old", "image", {
    "Version": "12.3.1",
    "RequestURL": "https://upload.example.com/x",
    "Body": "MultipartFormData",
    "FileFormName": "file",
}, image_input(), [resp(JSON_OK)])

custom("custom-missing-file-form-name", "image", {
    "Version": "17.0.0",
    "RequestURL": "https://upload.example.com/x",
    "Body": "MultipartFormData",
}, image_input(), [resp(JSON_OK)])

custom("custom-bad-function", "image", {
    "Version": "17.0.0",
    "RequestURL": "https://upload.example.com/{nosuch}",
    "Body": "MultipartFormData",
    "FileFormName": "file",
}, image_input(), [resp(JSON_OK)])

custom("custom-cookie-header", "text", {
    "Version": "17.0.0",
    "RequestMethod": "POST",
    "RequestURL": "https://paste.example.com/cookie",
    "Headers": {"Cookie": "session=made-up; theme=dark", "Accept": "application/json"},
    "Body": "JSON",
    "Data": "{\"t\":\"{input}\"}",
    "URL": "{json:u}",
}, text_input(text="short"), [resp('{"u":"https://paste.example.com/c"}')])

custom("custom-delete-json", "text", {
    "Version": "17.0.0",
    "RequestMethod": "DELETE",
    "RequestURL": "https://paste.example.com/del",
    "Body": "JSON",
    "Data": "{\"t\":\"{input}\"}",
}, text_input(text="x"), [text_resp("https://paste.example.com/d")])

custom("custom-dates-in-json", "image", {
    "Version": "17.0.0",
    "RequestURL": "https://upload.example.com/date",
    "Body": "MultipartFormData",
    "FileFormName": "file",
    "URL": "{json:created}",
    "ThumbnailURL": "{json:big}",
    "DeletionURL": "{json:small}",
}, image_input(), [resp('{"created":"2024-03-04T05:06:07Z","big":1e20,"small":0.00001}')])

custom("custom-server-error-text", "file", {
    "Version": "17.0.0",
    "RequestURL": "https://files.example.com/500",
    "Body": "Binary",
}, {"file_name": "a.bin", "content_base64": FILE_B64}, [text_resp("internal error", 500)])

# ---------------------------------------------------------------------------
# Built-in destinations.

FUTURE = "2099-01-01T00:00:00Z"


def oauth2(token="made-up-access", refresh="made-up-refresh", client="made-up-client", secret="made-up-secret"):
    return {"Client_ID": client, "Client_Secret": secret, "Token": {"access_token": token, "refresh_token": refresh,
            "expires_in": 3600, "token_type": "bearer", "scope": None, "ExpireDate": FUTURE}, "Proof": None}


IMGUR_OK = json.dumps({"data": {"id": "AbC123", "deletehash": "del456", "link": "https://i.imgur.com/AbC123.png",
                                "gifv": None, "type": "image/png"}, "success": True, "status": 200})
IMGUR_ERR = json.dumps({"data": {"error": "Imgur is temporarily over capacity. Please try again later.",
                                 "request": "/3/upload", "method": "POST"}, "success": False, "status": 500})
IMGUR_ERR_OBJ = json.dumps({"data": {"error": {"code": 1003, "message": "File type invalid (1)", "type": "ImgurException"},
                                     "request": "/3/upload", "method": "POST"}, "success": False, "status": 400})

builtin("imgur-anonymous", "image:Imgur", {}, image_input(), [resp(IMGUR_OK)])
builtin("imgur-anonymous-not-direct", "image:Imgur", {"ImgurDirectLink": False, "ImgurThumbnailType": "Big_Square"},
        image_input(), [resp(IMGUR_OK)])
builtin("imgur-video", "image:Imgur", {}, {"file_name": "clip.mp4", "content_base64": FILE_B64},
        [resp(json.dumps({"data": {"id": "V1d", "deletehash": "d", "link": "https://i.imgur.com/V1d.mp4.", "gifv": "https://i.imgur.com/V1d.gifv"},
                          "success": True, "status": 200}))])
builtin("imgur-error-string", "image:Imgur", {}, image_input(), [resp(IMGUR_ERR, 500)])
builtin("imgur-error-object", "image:Imgur", {}, image_input(), [resp(IMGUR_ERR_OBJ, 400)])
builtin("imgur-user-album", "image:Imgur", {"ImgurAccountType": "User", "ImgurOAuth2Info": oauth2(), "ImgurUploadSelectedAlbum": True,
                                            "ImgurSelectedAlbum": {"id": "alb1", "title": "x"}}, image_input(), [resp(IMGUR_OK)])
builtin("imgur-user-not-signed-in", "image:Imgur", {"ImgurAccountType": "User"}, image_input(), [resp(IMGUR_OK)])

builtin("chevereto", "image:Chevereto", {"CheveretoUploader": {"UploadURL": "images.example.com/api/1/upload", "APIKey": "made-up"},
                                        "CheveretoDirectURL": False},
        image_input(), [resp(json.dumps({"image": {"url": "https://images.example.com/i/a.png", "url_viewer": "https://images.example.com/v/a",
                                                   "thumb": {"url": "https://images.example.com/t/a.png"}}}))])
builtin("vgyme", "image:Vgyme", {"VgymeUserKey": "made-up-key"}, image_input(),
        [resp(json.dumps({"error": False, "url": "https://vgy.me/u/x", "image": "https://i.vgy.me/x.png", "delete": "https://vgy.me/delete/y"}))])

builtin("pastebin", "text:Pastebin", {"PastebinSettings": {"Title": "My paste", "TextFormat": "python", "Exposure": "Public",
                                                          "Expiration": "D1", "RawURL": True, "UserKey": "made-up-user-key"}},
        text_input(), [text_resp("https://pastebin.com/AbCdEf12")])
builtin("pastebin-bad-request", "text:Pastebin", {}, text_input(), [text_resp("Bad API request, invalid api_dev_key")])
builtin("paste-ee", "text:Paste_ee", {"Paste_eeUserKey": "made-up", "Paste_eeEncryptPaste": True}, text_input(),
        [resp('{"id":"Xy","link":"https://paste.ee/p/Xy"}')])
builtin("paste-ee-no-key", "text:Paste_ee", {}, text_input(), [resp('{}')])
builtin("hastebin", "text:Hastebin", {"HastebinCustomDomain": "https://haste.example.com/"}, text_input("code.RS"),
        [resp('{"key":"abcdef"}')])
builtin("hastebin-txt", "text:Hastebin", {}, text_input("plain.txt"), [resp('{"key":"qwerty"}')])
builtin("upaste", "text:Upaste", {"UpasteUserKey": "made-up", "UpasteIsPublic": True}, text_input(),
        [resp('{"paste":{"link":"https://upaste.me/abc"},"status":"success"}')])
builtin("upaste-error", "text:Upaste", {}, text_input(), [resp('{"status":"error","error":"Paste too short"}')])
builtin("onetimesecret", "text:OneTimeSecret", {"OneTimeSecretAPIUsername": "someone@example.com", "OneTimeSecretAPIKey": "made-up"},
        text_input(), [resp('{"secret_key":"s3cr3t"}')])
builtin("pastie", "text:Pastie", {"PastieIsPublic": True}, text_input(), [text_resp("ok")])
builtin("paste2", "text:Paste2", {}, text_input(), [text_resp("ok")])
builtin("slexy", "text:Slexy", {}, text_input(), [text_resp("ok")])

builtin("pomf", "file:Pomf", {"PomfUploader": {"UploadURL": "https://pomf.example.com/upload.php", "ResultURL": "files.example.com"}},
        {"file_name": "a.bin", "content_base64": FILE_B64},
        [resp('{"success":true,"files":[{"hash":"h","name":"a.bin","url":"abc.bin","size":"768"}]}')])
builtin("uguu", "file:Uguu", {}, {"file_name": "a.bin", "content_base64": FILE_B64}, [text_resp("https://a.uguu.se/xyz.bin\n")])
builtin("transfersh", "file:Transfersh", {}, {"file_name": "a.bin", "content_base64": FILE_B64}, [text_resp("https://transfer.sh/abc/a.bin\n")])
builtin("sul", "file:Sul", {"SulAPIKey": "made-up"}, {"file_name": "a.bin", "content_base64": FILE_B64},
        [resp('{"protocol":"https://","domain":"s-ul.eu","filename":"AbC","extension":".bin"}')])
builtin("sul-error", "file:Sul", {"SulAPIKey": "made-up"}, {"file_name": "a.bin", "content_base64": FILE_B64},
        [resp('{"error":"Invalid key"}')])
builtin("lambda", "file:Lambda", {"LambdaSettings": {"UserAPIKey": "made-up", "UploadURL": "https://λ.pw/"}},
        {"file_name": "a.bin", "content_base64": FILE_B64}, [resp('{"url":"abc.bin","errors":[]}')])
builtin("lambda-error", "file:Lambda", {"LambdaSettings": {"UserAPIKey": "made-up"}},
        {"file_name": "a.bin", "content_base64": FILE_B64}, [resp('{"url":null,"errors":["bad key","second"]}', 401)])

builtin("isgd", "shortener:ISGD", {}, url_input(), [text_resp("https://is.gd/abc")])
builtin("isgd-error", "shortener:ISGD", {}, url_input(), [text_resp("Error: Please enter a valid URL")])
builtin("vgd", "shortener:VGD", {}, url_input(), [text_resp("https://v.gd/abc")])
builtin("tinyurl", "shortener:TINYURL", {}, url_input(), [text_resp("https://tinyurl.com/abc")])
builtin("turl", "shortener:TURL", {}, url_input(), [text_resp("SUCCESS:abc")])
builtin("turl-error", "shortener:TURL", {}, url_input(), [text_resp("ERROR:Invalid")])
builtin("twogp", "shortener:TwoGP", {}, url_input(), [resp('{"url":"https://2.gp/abc"}')])
builtin("qrnet", "shortener:QRnet", {}, url_input(), [resp('{"url":"https://qr.net/abc"}')])
builtin("vurl", "shortener:VURL", {}, url_input(), [text_resp("https://vurl.com/abc")])
builtin("yourls", "shortener:YOURLS", {"YourlsAPIURL": "https://yourls.example.com/yourls-api.php", "YourlsSignature": "made-up"},
        url_input(), [text_resp("https://y.example/abc")])
builtin("yourls-userpass", "shortener:YOURLS", {"YourlsAPIURL": "https://yourls.example.com/yourls-api.php", "YourlsUsername": "u",
                                                "YourlsPassword": "p"}, url_input(), [text_resp("https://y.example/def")])
builtin("polr", "shortener:Polr", {"PolrAPIHostname": "polr.example.com/api/v2/action/shorten", "PolrAPIKey": "made-up", "PolrIsSecret": True},
        url_input(), [text_resp("https://polr.example.com/abc")])
builtin("polr-v1", "shortener:Polr", {"PolrAPIHostname": "https://polr.example.com/api.php", "PolrAPIKey": "made-up", "PolrUseAPIv1": True},
        url_input(), [text_resp("https://polr.example.com/def")])
builtin("kutt", "shortener:Kutt", {"KuttSettings": {"Host": "kutt.example.com", "APIKey": "made-up", "Password": None, "Reuse": True,
                                                   "Domain": "k.example"}}, url_input(), [resp('{"id":"i","link":"https://k.example/abc"}')])
builtin("zws", "shortener:ZeroWidthShortener", {"ZeroWidthShortenerToken": "made-up"}, url_input(), [resp('{"Short":"​‌"}')])
builtin("zws-url", "shortener:ZeroWidthShortener", {"ZeroWidthShortenerURL": "https://zws.example.com/"}, url_input(),
        [resp('{"URL":"https://zws.example.com/x"}')])
builtin("firebase", "shortener:FirebaseDynamicLinks", {"FirebaseWebAPIKey": "made-up", "FirebaseDynamicLinkDomain": "https://ex.page.link",
                                                       "FirebaseIsShort": True}, url_input(), [resp('{"shortLink":"https://ex.page.link/abc"}')])
builtin("bitly", "shortener:BITLY", {"BitlyOAuth2Info": oauth2(), "BitlyDomain": "bit.example"}, url_input(),
        [resp('{"created_at":"2024-01-01T00:00:00+0000","id":"bit.example/abc","link":"https://bit.example/abc","long_url":"x"}')])

# Amazon S3 (SigV4). The secret is made up; compare.py checks each side's
# signature with it, then normalises the timestamps and signatures.
S3_SECRET = "made-up-secret-key"
S3 = {"AccessKeyID": "AKIAMADEUPEXAMPLE", "SecretAccessKey": S3_SECRET, "Endpoint": "s3.eu-west-2.amazonaws.com",
      "Bucket": "made-up-bucket", "ObjectPrefix": "shots/sub"}
XML = [["Content-Type", "application/xml"]]
builtin("s3-put", "file:AmazonS3", {"AmazonS3Settings": S3}, image_input("shot 1.png"), [resp("", headers=[])])
builtin("s3-put-signed-path-style", "file:AmazonS3", {"AmazonS3Settings": dict(S3, Bucket="made.up.bucket", SignedPayload=True,
        SetPublicACL=False, StorageClass="STANDARD_IA", Region="eu-central-1", Endpoint="https://minio.example.com/")},
        {"file_name": "notes.txt", "content_base64": FILE_B64}, [resp("", headers=[])])
builtin("s3-custom-domain-no-extension", "file:AmazonS3", {"AmazonS3Settings": dict(S3, UseCustomCNAME=True, CustomDomain="cdn.example.com/{random:x|x}",
        RemoveExtensionImage=True, UsePathStyle=True)}, image_input("pic.png"), [resp("", headers=[])])
builtin("s3-error", "file:AmazonS3", {"AmazonS3Settings": S3}, image_input(),
        [resp("<?xml version=\"1.0\"?><Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>", 403, XML)])
builtin("s3-multipart", "file:AmazonS3", {"AmazonS3Settings": dict(S3, UseMultipartUpload=True, SignedPayload=True)}, image_input("big.png"), [
    resp("<InitiateMultipartUploadResult><Bucket>made-up-bucket</Bucket><Key>k</Key><UploadId>up/load+id=1</UploadId></InitiateMultipartUploadResult>", headers=XML),
    resp("", headers=[["ETag", "\"etag-part-1\""]]),
    resp("<CompleteMultipartUploadResult><Location>x</Location></CompleteMultipartUploadResult>", headers=XML)])
builtin("s3-multipart-complete-error", "file:AmazonS3", {"AmazonS3Settings": dict(S3, UseMultipartUpload=True)}, image_input("big.png"), [
    resp("<InitiateMultipartUploadResult><UploadId>id2</UploadId></InitiateMultipartUploadResult>", headers=XML),
    resp("", headers=[["ETag", "\"e2\""]]),
    resp("<Error><Code>InternalError</Code></Error>", headers=XML),
    resp("", 204, headers=[])])

BIN = {"file_name": "clip.mp4", "content_base64": FILE_B64}
builtin("puush", "file:Puush", {"PuushAPIKey": "made-up"}, BIN, [text_resp("0,https://puu.sh/abc.mp4,123,456")])
builtin("puush-auth-error", "file:Puush", {"PuushAPIKey": "made-up"}, BIN, [text_resp("-1")])
builtin("puush-garbage", "file:Puush", {"PuushAPIKey": "made-up"}, BIN, [text_resp("nope")])
builtin("streamable", "file:Streamable", {"StreamableUsername": "someone@example.com", "StreamablePassword": "made-up",
                                         "StreamableUseDirectURL": True}, BIN, [
    resp('{"shortcode":"abc12","status":1}'),
    resp('{"status":1,"percent":40}'),
    resp('{"status":2,"url":"streamable.com/abc12","files":{"mp4":{"url":"//cdn.example.com/abc12.mp4"}}}')])
builtin("streamable-failed", "file:Streamable", {"StreamableUsername": "u", "StreamablePassword": "p"}, BIN, [
    resp('{"shortcode":"x"}'), resp('{"status":3,"message":"Transcoding failed"}')])
builtin("streamable-no-shortcode", "file:Streamable", {"StreamableUsername": "u", "StreamablePassword": "p"}, BIN, [resp('{"status":0}')])
builtin("lobfile", "file:Lithiio", {"LithiioSettings": {"UserAPIKey": "made-up"}}, BIN, [resp('{"success":true,"url":"https://lobfile.com/f/x"}')])
builtin("lobfile-error", "file:Lithiio", {"LithiioSettings": {"UserAPIKey": "made-up"}}, BIN, [resp('{"success":false,"error":"Bad key"}')])
builtin("imgfish", "file:ImgFish", {"ImgFishSettings": {"APIKey": "made-up", "FileIDLength": 12}}, image_input(),
        [resp('{"link":"https://img.fish/x.png","destroy":"https://img.fish/d/x"}')])
builtin("imgfish-error", "file:ImgFish", {}, image_input(), [resp('{"error":"Too big"}')])
builtin("imgfish-not-json", "file:ImgFish", {}, image_input(), [text_resp("oops")])

OC = {"OwnCloudHost": "cloud.example.com", "OwnCloudUsername": "someone", "OwnCloudPassword": "made-up", "OwnCloudPath": "/ShareX/shots"}
OCS_OK = json.dumps({"ocs": {"meta": {"status": "ok", "statuscode": 100, "message": None}, "data": {"id": 7, "url": "https://cloud.example.com/s/AbC", "token": "AbC"}}})
builtin("owncloud-share-direct", "file:OwnCloud", dict(OC, OwnCloudDirectLink=True, OwnCloudAppendFileNameToURL=True), image_input("my shot.png"),
        [resp("", 201, []), resp(OCS_OK)])
builtin("owncloud-preview-expire", "file:OwnCloud", dict(OC, OwnCloudUsePreviewLinks=True, OwnCloudAutoExpire=True, OwnCloudExpiryTime=3,
        OwnCloud81Compatibility=False), image_input(), [resp("", 201, []), resp(OCS_OK)])
builtin("owncloud-no-share", "file:OwnCloud", dict(OC, OwnCloudCreateShare=False), image_input(), [text_resp("created", 201)])
builtin("owncloud-share-error", "file:OwnCloud", OC, image_input(),
        [resp("", 201, []), resp(json.dumps({"ocs": {"meta": {"status": "failure", "statuscode": 404, "message": "Wrong path"}, "data": []}}))])
builtin("owncloud-put-error", "file:OwnCloud", OC, image_input(), [text_resp("denied", 401)])
IM = {"ImmichURL": "https://photos.example.com/api/", "ImmichAPIKey": "made-up"}
builtin("immich-share", "file:Immich", IM, image_input(), [resp('{"id":"asset-1","status":"created"}', 201), resp('{"key":"ShArE"}', 201)])
builtin("immich-no-share", "file:Immich", dict(IM, ImmichAutoCreateShareableLink=False, ImmichShowMetadata=True), image_input(),
        [resp('{"id":"asset-2","status":"duplicate","duplicate":true}')])
builtin("immich-no-id", "file:Immich", IM, image_input(), [resp('{"status":"created"}')])

# ---------------------------------------------------------------------------
# Signed storage: Azure (SharedKey; compare.py checks each side's signature
# with the made-up key, then normalises the date and signature) and B2.

AZURE_KEY = base64.b64encode(b"made-up-azure-key").decode()
AZ = {"AzureStorageAccountName": "madeupacct", "AzureStorageAccountAccessKey": AZURE_KEY, "AzureStorageContainer": "shots",
      "AzureStorageUploadPath": "/ShareX/sub dir/", "AzureStorageCacheControl": "max-age=60"}
builtin("azure", "file:AzureStorage", AZ, image_input("my shot.png"), [resp("", 201, [])])
builtin("azure-root-custom-domain", "file:AzureStorage", dict(AZ, AzureStorageContainer="$root", AzureStorageCustomDomain="cdn.example.com",
        AzureStorageUploadPath="", AzureStorageCacheControl="", AzureStorageEnvironment="blob.core.chinacloudapi.cn"), image_input(), [resp("", 201, [])])
builtin("azure-error", "file:AzureStorage", AZ, image_input(),
        [resp("<?xml version=\"1.0\"?><Error><Code>AuthenticationFailed</Code></Error>", 403, XML)])
builtin("azure-missing-account", "file:AzureStorage", dict(AZ, AzureStorageAccountName=""), image_input(), [])

B2 = {"B2ApplicationKeyId": "made-up-key-id", "B2ApplicationKey": "made-up-key", "B2BucketName": "made-up-bucket", "B2UploadPath": "shots/sub"}
B2_AUTH = {"accountId": "acc1", "apiUrl": "https://api000.backblazeb2.com", "authorizationToken": "auth-tok", "downloadUrl": "https://f000.backblazeb2.com",
           "minimumPartSize": 100, "allowed": {"capabilities": ["listBuckets", "writeFiles"], "bucketId": None, "namePrefix": None}}
B2_UPLOAD_URL = json.dumps({"bucketId": "b1", "uploadUrl": "https://pod-000.backblaze.com/b2api/v1/b2_upload_file/b1/c000", "authorizationToken": "up-tok"})
builtin("b2", "file:BackblazeB2", B2, image_input("my shot.png"), [
    resp(json.dumps(B2_AUTH)),
    resp(json.dumps({"buckets": [{"bucketName": "other", "bucketId": "b0"}, {"bucketName": "made-up-bucket", "bucketId": "b1"}]})),
    resp(B2_UPLOAD_URL),
    resp(json.dumps({"fileId": "f1", "fileName": "shots/sub/my shot.png", "contentLength": 768}))])
builtin("b2-retry-custom-url", "file:BackblazeB2", dict(B2, B2UseCustomUrl=True, B2CustomUrl="cdn.example.com/b2"), image_input(), [
    resp(json.dumps(dict(B2_AUTH, allowed={"capabilities": ["writeFiles"], "bucketId": "b1", "namePrefix": "shots/"}))),
    resp(B2_UPLOAD_URL),
    resp(json.dumps({"status": 503, "code": "service_unavailable", "message": "busy"}), 503),
    resp(B2_UPLOAD_URL),
    resp(json.dumps({"fileId": "f2", "fileName": "shots/sub/test image.png"}))])
builtin("b2-auth-error", "file:BackblazeB2", B2, image_input(), [resp(json.dumps({"status": 401, "code": "bad_auth_token", "message": "Invalid"}), 401)])
builtin("b2-auth-error-text", "file:BackblazeB2", B2, image_input(), [text_resp("nope", 500)])
builtin("b2-prefix-not-allowed", "file:BackblazeB2", B2, image_input(),
        [resp(json.dumps(dict(B2_AUTH, allowed={"capabilities": ["writeFiles"], "bucketId": "b1", "namePrefix": "other/"})))])
builtin("b2-no-bucket", "file:BackblazeB2", B2, image_input(), [resp(json.dumps(B2_AUTH)), resp('{"buckets":[]}')])
builtin("b2-upload-error", "file:BackblazeB2", B2, image_input(), [
    resp(json.dumps(dict(B2_AUTH, allowed={"capabilities": ["writeFiles"], "bucketId": "b1", "namePrefix": None}))),
    resp(B2_UPLOAD_URL), resp(json.dumps({"status": 400, "code": "bad_request", "message": "no"}), 400)])

# ---------------------------------------------------------------------------
# Account services.

BIN_PNG = image_input("my shot.png")
builtin("hostr-direct", "file:Localhostr", {"LocalhostrEmail": "someone@example.com", "LocalhostrPassword": "made-up", "LocalhostrDirectURL": True},
        BIN_PNG, [resp(json.dumps({"added": "x", "name": "my shot.png", "href": "https://hostr.co/abc", "size": 768, "type": "image",
                                   "direct": {"150x": "https://hostr.co/file/150/abc/my shot.png", "930x": "y"}, "id": "abc"}))])
builtin("hostr-href", "file:Localhostr", {"LocalhostrEmail": "someone@example.com", "LocalhostrPassword": "made-up", "LocalhostrDirectURL": True},
        BIN_PNG, [resp(json.dumps({"name": "a", "href": "https://hostr.co/def", "id": "def", "direct": None}))])
PLIK = {"URL": "https://plik.example.com", "APIKey": "made-up", "IsSecured": True, "Login": "user", "Password": "pass", "Removable": True,
        "OneShot": False, "TTLUnit": 1, "TTL": 2.5, "HasComment": True, "Comment": "a \"comment\""}
builtin("plik", "file:Plik", {"PlikSettings": PLIK}, BIN_PNG, [
    resp(json.dumps({"id": "up1", "files": {"f1": {"id": "f1", "fileName": "my shot.png"}}, "uploadToken": "utok"})),
    resp(json.dumps({"id": "f1", "status": "uploaded"}))])
builtin("plik-forever", "file:Plik", {"PlikSettings": dict(PLIK, TTLUnit=3, IsSecured=False, HasComment=False)}, BIN_PNG, [
    resp(json.dumps({"id": "up2", "files": {"f9": {"id": "f9", "fileName": "a&b.png"}}, "uploadToken": "u2"})), resp("{}")])
builtin("plik-error", "file:Plik", {"PlikSettings": PLIK}, BIN_PNG, [text_resp("bad", 500)])
SEA = {"SeafileAPIURL": "sea.example.com/api2/", "SeafileAuthToken": "made-up-token", "SeafileRepoID": "repo-1", "SeafilePath": "/ShareX",
       "SeafileCreateShareableURLRaw": True, "SeafileSharePassword": "secret", "SeafileShareDaysToExpire": 7}
builtin("seafile-share-raw", "file:Seafile", SEA, BIN_PNG, [
    text_resp('"https://sea.example.com/seafhttp/upload-api/abc"'), text_resp("fileid"),
    resp("", 201, [["Location", "https://sea.example.com/f/abc123/?dl=0"]])])
builtin("seafile-encrypted", "file:Seafile", dict(SEA, SeafileIsLibraryEncrypted=True, SeafilePath=""), BIN_PNG, [
    text_resp('"https://sea.example.com/up"'), text_resp("fileid")])
builtin("seafile-link-error", "file:Seafile", SEA, BIN_PNG, [text_resp("denied", 403)])
PB = {"PushbulletSettings": {"UserAPIKey": "made-up", "DeviceList": [{"Key": "dev0", "Name": "Tablet"}, {"Key": "dev1", "Name": "Phone"}], "SelectedDevice": 1}}
builtin("pushbullet", "file:Pushbullet", PB, BIN_PNG, [
    resp(json.dumps({"file_type": "image/png", "file_name": "my shot.png", "file_url": "https://dl.pushbulletusercontent.com/x/my shot.png",
                     "upload_url": "https://s3.amazonaws.com/pushbullet-uploads",
                     "data": {"awsaccesskeyid": "AK", "acl": "public-read", "key": "k/my shot.png", "signature": "sig", "policy": "pol",
                              "content-type": "image/png"}})),
    resp("", 204, []), resp(json.dumps({"iden": "push1", "created": 1}))])
builtin("pushbullet-no-device", "file:Pushbullet", {"PushbulletSettings": dict(PB["PushbulletSettings"], SelectedDevice=5)}, BIN_PNG, [])
builtin("pushbullet-share", "sharing:Pushbullet", PB, url_input(), [resp(json.dumps({"iden": "push2"}))])
SS_INFO = '<result method="anonymous.uploadGetInfo" status="ok"><upload url="https://fs00.sendspace.com/upload?SPEED_LIMIT=0" ' \
          'progress_url="https://fs00.sendspace.com/progress" max_file_size="314572800" upload_identifier="ui-1" extra_info="ei-1"/></result>'
builtin("sendspace-anonymous", "file:SendSpace", {}, BIN_PNG, [
    resp(SS_INFO, headers=XML),
    resp("<upload_done><status>ok</status><download_url>https://www.sendspace.com/file/abc</download_url>"
         "<delete_url>https://www.sendspace.com/delete/abc/x</delete_url><file_id>abc</file_id></upload_done>", headers=XML)])
builtin("sendspace-anonymous-fail", "file:SendSpace", {}, BIN_PNG, [
    resp('<result method="anonymous.uploadGetInfo" status="fail"><error code="6" text="API key invalid"/></result>', headers=XML)])
builtin("sendspace-user", "file:SendSpace", {"SendSpaceAccountType": "User", "SendSpaceUsername": "someone", "SendSpacePassword": "made-up"}, BIN_PNG, [
    resp('<result method="auth.createToken" status="ok"><token>tok123</token></result>', headers=XML),
    resp('<result method="auth.login" status="ok"><session_key>sess-1</session_key><user_name>someone</user_name></result>', headers=XML),
    resp(SS_INFO.replace("anonymous.uploadGetInfo", "upload.getInfo"), headers=XML),
    text_resp("upload_status=ok\nfile_id=xyz9\nfile_name=my shot.png")])
builtin("sendspace-user-again", "file:SendSpace", {"SendSpaceAccountType": "User", "SendSpaceUsername": "someone", "SendSpacePassword": "made-up"},
        BIN_PNG, [resp(SS_INFO.replace("anonymous.uploadGetInfo", "upload.getInfo"), headers=XML), text_resp("upload_status=ok\nfile_id=q1")])
MF = {"MediaFireUsername": "someone@example.com", "MediaFirePassword": "made-up", "MediaFirePath": "ShareX/shots", "MediaFireUseLongLink": True}
builtin("mediafire", "file:MediaFire", MF, BIN_PNG, [
    resp(json.dumps({"response": {"action": "user/get_session_token", "session_token": "st-1", "secret_key": 123456789,
                                  "time": "1700000000.1234", "result": "Success", "new_key": "no"}})),
    resp(json.dumps({"response": {"doupload": {"result": "0", "key": "upkey"}, "result": "Success", "new_key": "yes"}})),
    resp(json.dumps({"response": {"doupload": {"result": "0", "status": "17", "description": "Upload is in progress"},
                                  "result": "Success", "new_key": "yes"}})),
    resp(json.dumps({"response": {"doupload": {"result": "0", "status": "99", "quickkey": "qk1", "filename": "my shot.png"},
                                  "result": "Success", "new_key": "no"}}))])
builtin("mediafire-login-error", "file:MediaFire", MF, BIN_PNG,
        [resp(json.dumps({"response": {"result": "Error", "error": 107, "message": "Invalid credentials"}}))])
builtin("mediafire-poll-error", "file:MediaFire", MF, BIN_PNG, [
    resp(json.dumps({"response": {"session_token": "st", "secret_key": 7, "time": "1.5", "result": "Success"}})),
    resp(json.dumps({"response": {"doupload": {"result": 0, "key": "k"}, "result": "Success"}})),
    resp(json.dumps({"response": {"doupload": {"result": 0, "status": 99, "fileerror": 4, "description": "File too big"}, "result": "Success"}}))])
ISH = {"ImageShackSettings": {"Username": None, "Password": None, "IsPublic": True, "Auth_token": "made-up-token", "ThumbnailWidth": 200,
                              "ThumbnailHeight": 150}}
builtin("imageshack", "image:ImageShack", ISH, image_input(),
        [resp(json.dumps({"success": True, "process_time": 1, "result": {"images": [{"server": 123, "bucket": 456, "filename": "abc.png"}]}}))])
builtin("imageshack-error", "image:ImageShack", ISH, image_input(),
        [resp(json.dumps({"success": False, "error": {"error_code": 3, "error_message": "Bad image"}}))])

# PrivateBin encrypts with a random key: compare.py decrypts each side's
# paste with the key from its result URL, then normalises the cipher text.
PRIVATEBIN = {"Username": "user", "Password": "pass", "BurnAfterReading": True, "Expiration": "D1", "Format": "Markdown",
              "CustomUrl": "https://bin.example.com/pb/", "PastePassword": "pw"}
builtin("privatebin", "text:PrivateBin", {"PrivateBinSettings": PRIVATEBIN}, text_input(),
        [resp(json.dumps({"status": 0, "id": "abc", "url": "/?abc", "deletetoken": "dt+1"}))])
builtin("privatebin-defaults", "text:PrivateBin", {}, text_input(text="plain"),
        [resp(json.dumps({"status": 0, "id": "d", "url": "?d", "deletetoken": "t"}))])
builtin("privatebin-error", "text:PrivateBin", {"PrivateBinSettings": PRIVATEBIN}, text_input(),
        [resp(json.dumps({"status": 1, "message": "Invalid data."}))])
builtin("privatebin-401", "text:PrivateBin", {"PrivateBinSettings": PRIVATEBIN}, text_input(), [text_resp("auth", 401)])

# Shared folder: no HTTP; both harnesses read back the file written
# (relative to the working directory) and delete it.
SHARED_ROOT = "target/uploaders-harness/shared"
cases.append({"id": "shared-folder", "kind": "builtin", "service": "file:SharedFolder",
              "config": {"LocalhostAccountList": [{"Name": "x", "LocalhostRoot": SHARED_ROOT, "Port": 8080, "SubFolderPath": "shots",
                                                   "HttpHomePath": "@https://files.example.com/share/", "RemoteProtocol": "https"}]},
              "input": dict(image_input("my shot.png"), check_file=SHARED_ROOT + "/shots/my shot.png"), "responses": []})

# ---------------------------------------------------------------------------
# OAuth destinations (tokens made up; an expired one makes the uploader
# refresh it first).

PAST = "2001-01-01T00:00:00Z"
TOKEN_OK = json.dumps({"access_token": "new-access", "refresh_token": "new-refresh", "expires_in": 3600, "token_type": "Bearer", "scope": "x"})
VIDEO = {"file_name": "clip.mp4", "content_base64": FILE_B64}

builtin("gist-raw", "text:Gist", {"GistOAuth2Info": oauth2(), "GistPublishPublic": True, "GistRawURL": True}, text_input("notes.md"),
        [resp(json.dumps({"html_url": "https://gist.github.com/abc", "files": {"notes.md": {"filename": "notes.md", "raw_url": "https://gist.githubusercontent.com/raw/abc/notes.md"}}}), 201)])
builtin("gist-custom-api", "text:Gist", {"GistOAuth2Info": oauth2(), "GistCustomURL": "https://github.example.com/api/v3"}, text_input(),
        [resp(json.dumps({"html_url": "https://github.example.com/gist/abc", "files": {}}), 201)])
builtin("gist-error", "text:Gist", {"GistOAuth2Info": oauth2()}, text_input(), [resp('{"message":"Bad credentials"}', 401)])

DBX = {"DropboxOAuth2Info": oauth2(), "DropboxUploadPath": "ShareX\\shots/", "DropboxAutoCreateShareableLink": True, "DropboxUseDirectLink": True}
DBX_META = json.dumps({".tag": "file", "name": "my shot.png", "path_display": "/ShareX/shots/my shot.png", "id": "id:1", "size": 768})
builtin("dropbox-direct-link", "file:Dropbox", DBX, BIN_PNG, [
    resp(DBX_META), resp(json.dumps({".tag": "file", "url": "https://www.dropbox.com/scl/fi/abc/my%20shot.png?rlkey=k&dl=0"}))])
builtin("dropbox-link-exists", "file:Dropbox", dict(DBX, DropboxUseDirectLink=False), BIN_PNG, [
    resp(DBX_META), resp(json.dumps({"error_summary": "shared_link_already_exists/..", "error": {".tag": "shared_link_already_exists"}}), 409),
    resp(json.dumps({"links": [{".tag": "file", "url": "https://www.dropbox.com/scl/fi/old/my%20shot.png?dl=0"}], "has_more": False}))])
builtin("dropbox-no-link", "file:Dropbox", dict(DBX, DropboxAutoCreateShareableLink=False), BIN_PNG, [resp(DBX_META)])
builtin("dropbox-error", "file:Dropbox", DBX, BIN_PNG, [resp('{"error_summary":"path/insufficient_space/"}', 409)])

OD = {"OneDriveV2OAuth2Info": oauth2(), "OneDriveV2SelectedFolder": {"id": "FOLDER1", "name": "Shots", "webUrl": None}}
OD_SESSION = json.dumps({"uploadUrl": "https://api.onedrive.com/rup/abc123", "nextExpectedRanges": ["0-"]})
OD_ITEM = json.dumps({"id": "ITEM1", "name": "my shot.png", "webUrl": "https://onedrive.live.com/?id=ITEM1"})
builtin("onedrive-link", "file:OneDrive", OD, BIN_PNG, [
    resp(OD_SESSION), resp(OD_ITEM, 201), resp(json.dumps({"link": {"webUrl": "https://1drv.ms/i/s!abc", "webHtml": None}}), 201)])
builtin("onedrive-web-url-root", "file:OneDrive", dict(OD, OneDriveAutoCreateShareableLink=False, OneDriveV2SelectedFolder={"id": "", "name": "Root"}),
        image_input("a b#c.png"), [resp(OD_SESSION), resp(OD_ITEM, 201)])
builtin("onedrive-chunk-error", "file:OneDrive", OD, BIN_PNG, [resp(OD_SESSION), text_resp("fail", 500), resp("", 204, [])])
builtin("onedrive-refresh", "file:OneDrive", {"OneDriveV2OAuth2Info": dict(oauth2(), Token=dict(oauth2()["Token"], ExpireDate=PAST)),
                                             "OneDriveUseDirectLink": True}, BIN_PNG, [
    resp(TOKEN_OK), resp(OD_SESSION), resp(OD_ITEM, 201), resp(json.dumps({"link": {"webUrl": "https://onedrive.live.com/embed?x"}}))])
builtin("onedrive-refresh-fails", "file:OneDrive", {"OneDriveV2OAuth2Info": dict(oauth2(), Token=dict(oauth2()["Token"], ExpireDate=PAST))},
        BIN_PNG, [resp('{"error":"invalid_grant"}', 400)])
builtin("onedrive-not-signed-in", "file:OneDrive", {"OneDriveV2OAuth2Info": dict(oauth2(), Token=None)}, BIN_PNG, [])

BOX = {"BoxOAuth2Info": oauth2(), "BoxSelectedFolder": {"type": "folder", "id": "123", "name": "Shots"}, "BoxShareAccessLevel": "Company"}
BOX_FILE = json.dumps({"total_count": 1, "entries": [{"type": "file", "id": "999", "name": "my shot.png", "parent": {"type": "folder", "id": "123"}}]})
builtin("box-share", "file:Box", BOX, BIN_PNG, [resp(BOX_FILE, 201), resp(json.dumps({"type": "file", "id": "999", "shared_link": {"url": "https://app.box.com/s/abc"}}))])
builtin("box-no-share", "file:Box", dict(BOX, BoxShare=False), BIN_PNG, [resp(BOX_FILE, 201)])
builtin("box-refresh-fails", "file:Box", {"BoxOAuth2Info": dict(oauth2(), Token=dict(oauth2()["Token"], ExpireDate=PAST))}, BIN_PNG,
        [text_resp("no", 400)])
builtin("box-not-signed-in", "file:Box", {"BoxOAuth2Info": dict(oauth2(), Client_Secret="")}, BIN_PNG, [])

GD = {"GoogleDriveOAuth2Info": oauth2(), "GoogleDriveIsPublic": True, "GoogleDriveDirectLink": True}
GD_FILE = json.dumps({"id": "FILE1", "webViewLink": "https://drive.google.com/file/d/FILE1/view?usp=drivesdk",
                      "webContentLink": "https://drive.google.com/uc?id=FILE1&export=download"})
builtin("google-drive-public-direct", "file:GoogleDrive", GD, BIN_PNG, [resp(GD_FILE), resp('{"kind":"drive#permission","id":"anyone"}')])
builtin("google-drive-folder", "file:GoogleDrive", dict(GD, GoogleDriveIsPublic=False, GoogleDriveDirectLink=False, GoogleDriveUseFolder=True,
                                                        GoogleDriveFolderID="FOLDER9", GoogleDriveSelectedDrive={"id": "DRIVE2", "name": "Team"}),
        BIN_PNG, [resp(GD_FILE)])
builtin("google-drive-shared-drive-root", "file:GoogleDrive", dict(GD, GoogleDriveIsPublic=False, GoogleDriveDirectLink=False,
                                                                   GoogleDriveSelectedDrive={"id": "DRIVE2", "name": "Team"}), BIN_PNG, [resp(GD_FILE)])
builtin("google-drive-refresh", "file:GoogleDrive", {"GoogleDriveOAuth2Info": dict(oauth2(), Token=dict(oauth2()["Token"], ExpireDate=PAST)),
                                                     "GoogleDriveIsPublic": False}, BIN_PNG,
        [resp(json.dumps({"access_token": "fresh", "expires_in": 3599, "token_type": "Bearer"})), resp(GD_FILE)])
builtin("google-drive-error", "file:GoogleDrive", GD, BIN_PNG, [resp('{"error":{"code":403,"message":"no"}}', 403)])
GCS = {"GoogleCloudStorageOAuth2Info": oauth2(), "GoogleCloudStorageBucket": "made-up-bucket", "GoogleCloudStorageObjectPrefix": "/shots/sub/",
       "GoogleCloudStorageRemoveExtensionImage": True}
builtin("gcs-acl", "file:GoogleCloudStorage", GCS, BIN_PNG, [resp('{"name":"shots/sub/my shot"}')])
builtin("gcs-domain-no-acl", "file:GoogleCloudStorage", dict(GCS, GoogleCloudStorageDomain="cdn.example.com", GoogleCloudStorageSetPublicACL=False,
                                                             GoogleCloudStorageRemoveExtensionImage=False), BIN_PNG, [resp('{"name":"shots/sub/my shot.png"}')])
builtin("gcs-error", "file:GoogleCloudStorage", GCS, BIN_PNG, [resp('{"error":{"code":403}}', 403)])
YT = {"YouTubeOAuth2Info": oauth2(), "YouTubePrivacyType": "Unlisted"}
builtin("youtube", "file:YouTube", YT, VIDEO,
        [resp(json.dumps({"id": "vid123", "snippet": {"title": "clip"}, "status": {"uploadStatus": "uploaded", "privacyStatus": "unlisted"}}))])
builtin("youtube-short-rejected", "file:YouTube", dict(YT, YouTubeUseShortenedLink=True, YouTubePrivacyType="Private"), VIDEO,
        [resp(json.dumps({"id": "vid9", "status": {"uploadStatus": "rejected", "rejectionReason": "duplicate", "privacyStatus": "private"}}))])

# OAuth 1.0a (made-up consumer and token secrets; compare.py checks each
# side's HMAC-SHA1 signature, then normalises the nonce, time and signature).
OAUTH1 = {"Description": "x", "OAuthVersion": "1.0", "ConsumerKey": "made-up-consumer-key", "ConsumerSecret": "made-up-consumer-secret",
          "SignatureMethod": "HMAC_SHA1", "UserToken": "made-up-user-token", "UserSecret": "made-up-user-secret"}
FLICKR = {"FlickrOAuthInfo": OAUTH1, "FlickrSettings": {"DirectLink": False, "Title": "A title", "Tags": "a b", "IsPublic": "1", "Hidden": "2"}}
builtin("flickr", "image:Flickr", FLICKR, BIN_PNG, [
    resp('<?xml version="1.0" encoding="utf-8" ?>\n<rsp stat="ok">\n<photoid>51234</photoid>\n</rsp>', headers=XML),
    resp(json.dumps({"sizes": {"canblog": 0, "size": [{"label": "Square", "source": "https://live.staticflickr.com/s.png", "url": "https://flickr.com/s"},
                                                       {"label": "Original", "source": "https://live.staticflickr.com/o.png",
                                                        "url": "https://www.flickr.com/photos/me/51234/sizes/o/"}]}, "stat": "ok"}))])
builtin("flickr-fail", "image:Flickr", dict(FLICKR, FlickrSettings={"DirectLink": True}), BIN_PNG,
        [resp('<rsp stat="fail"><err code="5" msg="Filetype was not recognised" /></rsp>', headers=XML)])
PB_ACCOUNT = {"Subdomain": "api123.photobucket.com", "AlbumID": "me", "AlbumList": ["me", "me/shots"], "ActiveAlbumID": 1}
builtin("photobucket", "image:Photobucket", {"PhotobucketOAuthInfo": OAUTH1, "PhotobucketAccountInfo": PB_ACCOUNT}, BIN_PNG, [
    resp('<?xml version="1.0"?><response><status>OK</status><content><url>https://i.photobucket.com/me/shots/a.png</url>'
         '<thumb>https://i.photobucket.com/me/shots/th_a.png</thumb></content></response>', headers=XML)])
builtin("photobucket-bad-album", "image:Photobucket", {"PhotobucketOAuthInfo": OAUTH1, "PhotobucketAccountInfo": dict(PB_ACCOUNT, ActiveAlbumID=5)},
        BIN_PNG, [])

# ---------------------------------------------------------------------------
# Sign-in flows: the authorization URL each service builds, the code (PIN)
# exchange, refresh and Google's user info, against a mock token endpoint.


def oauth_case(id, service, steps, responses, oauth2_info=None, oauth1_info=None, proof=None, redirect=None):
    c = {"id": id, "kind": "oauth", "service": service, "steps": steps, "responses": responses}
    if oauth2_info is not None:
        c["oauth2"] = oauth2_info
    if oauth1_info is not None:
        c["oauth1"] = oauth1_info
    if proof is not None:
        c["proof"] = proof
    if redirect is not None:
        c["redirect"] = redirect
    cases.append(c)


CLIENT = {"Client_ID": "made-up-client-id", "Client_Secret": "made-up-client-secret", "Token": None, "Proof": None}
SIGNED_IN = dict(CLIENT, Token={"access_token": "old-access", "refresh_token": "old-refresh", "expires_in": 3600, "token_type": "bearer",
                                "scope": None, "ExpireDate": PAST})
URL, CODE, REFRESH = {"op": "authorization_url"}, {"op": "access_token", "code": "made-up code/1"}, {"op": "refresh"}
oauth_case("signin-imgur", "imgur", [URL, CODE], [resp(TOKEN_OK)], CLIENT)
oauth_case("signin-imgur-refresh", "imgur", [REFRESH, {"op": "check"}], [resp(TOKEN_OK)], SIGNED_IN)
oauth_case("signin-gist", "gist", [URL, CODE], [resp('{"access_token":"gho_x","token_type":"bearer","scope":"gist"}')], CLIENT)
oauth_case("signin-dropbox", "dropbox", [URL, CODE], [resp(TOKEN_OK)], CLIENT)
oauth_case("signin-dropbox-refused", "dropbox", [CODE], [resp('{"error":"invalid_grant"}', 400)], CLIENT)
oauth_case("signin-onedrive-pkce", "onedrive", [URL, CODE, REFRESH], [resp(TOKEN_OK), resp(json.dumps({"access_token": "a2", "expires_in": 3600}))],
           CLIENT, proof={"verifier": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", "challenge": "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"})
oauth_case("signin-box", "box", [URL, CODE, REFRESH], [resp(TOKEN_OK), resp(TOKEN_OK)], CLIENT)
oauth_case("signin-box-no-refresh", "box", [REFRESH], [], CLIENT)
oauth_case("signin-bitly", "bitly", [URL, CODE], [resp('{"access_token":"bitly-token"}')], CLIENT)
REDIRECT = {"uri": "http://127.0.0.1:49152/", "state": "made-up-state-0123456789abcdefghij"}
oauth_case("signin-google-drive", "google-drive", [URL, CODE, {"op": "user_info"}],
           [resp(TOKEN_OK), resp(json.dumps({"sub": "1", "name": "Some One", "given_name": "Some", "picture": "https://x/p.png", "locale": "en"}))],
           CLIENT, redirect=REDIRECT)
oauth_case("signin-youtube-refresh", "youtube", [{"op": "check"}, URL], [resp(json.dumps({"access_token": "a3", "expires_in": 60}))], SIGNED_IN,
           redirect=REDIRECT)
oauth_case("signin-gcs-refresh-fails", "google-cloud-storage", [{"op": "check"}], [resp("{}", 400)], SIGNED_IN, redirect=REDIRECT)
OAUTH1_CLIENT = {"Description": "x", "OAuthVersion": "1.0", "ConsumerKey": "made-up-consumer-key", "ConsumerSecret": "made-up-consumer-secret",
                 "SignatureMethod": "HMAC_SHA1"}
FORM = [["Content-Type", "application/x-www-form-urlencoded"]]
oauth_case("signin-flickr", "flickr", [URL, {"op": "access_token", "code": "123-456-789"}], [
    resp("oauth_callback_confirmed=true&oauth_token=made-up-request-token&oauth_token_secret=made-up-request-secret", headers=FORM),
    resp("fullname=Some%20One&oauth_token=made-up-user-token&oauth_token_secret=made-up-user-secret&user_nsid=1%40N0&username=someone",
         headers=FORM)], oauth1_info=OAUTH1_CLIENT)
oauth_case("signin-photobucket", "photobucket", [URL, {"op": "access_token", "code": "verifier"}], [
    resp("oauth_token=made-up-request-token&oauth_token_secret=made-up-request-secret&next_step=x", headers=FORM),
    resp("oauth_token=made-up-user-token&oauth_token_secret=made-up-user-secret&username=someone&subdomain=api123.photobucket.com",
         headers=FORM)], oauth1_info=OAUTH1_CLIENT)
oauth_case("signin-flickr-no-token", "flickr", [URL], [text_resp("oauth_problem=signature_invalid", 401)], oauth1_info=OAUTH1_CLIENT)

# ---------------------------------------------------------------------------
# FTP / FTPS / SFTP against the mock servers (sharex_uploaders_mock_servers,
# fixed ports): every command and reply is compared.

MOCK = 18820
FTP_ACCOUNT = {"Name": "mock", "Protocol": "FTP", "Host": "127.0.0.1", "Port": MOCK + 1, "Username": "user", "Password": "pass",
               "IsActive": False, "SubFolderPath": "shots/sub", "BrowserProtocol": "https", "HttpHomePath": "files.example.com/~me",
               "HttpHomePathAutoAddSubFolderPath": True, "HttpHomePathNoExtension": False}


def ftp_case(id, accounts, inp, servers=None, extra=None, selected=0):
    config = {"FTPAccountList": accounts, "FTPSelectedImage": selected, "FTPSelectedText": selected, "FTPSelectedFile": selected}
    config.update(extra or {})
    cases.append({"id": id, "kind": "builtin", "service": "file:FTP", "config": config, "input": inp, "responses": [],
                  "servers": servers or {"user": "user", "password": "pass", "dirs": []}})


ftp_case("ftp-passive-mkdir", [FTP_ACCOUNT], BIN_PNG)
ftp_case("ftp-existing-dir", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots/sub"]})
ftp_case("ftp-parent-exists", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots"]})
ftp_case("ftp-active", [dict(FTP_ACCOUNT, IsActive=True)], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots/sub"]})
ftp_case("ftp-no-epsv", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots/sub"],
                                                 "fail_once": [["EPSV", "502 Command not implemented."]]})
ftp_case("ftp-active-no-eprt", [dict(FTP_ACCOUNT, IsActive=True)], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots/sub"],
                                                                         "fail_once": [["EPRT", "502 Command not implemented."]]})
ftp_case("ftp-login-failed", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "other", "dirs": []})
ftp_case("ftp-absolute-root", [dict(FTP_ACCOUNT, SubFolderPath="/up/sub", HttpHomePath="", HttpHomePathNoExtension=True, BrowserProtocol="http")],
         image_input("a+b c.png"), {"user": "user", "password": "pass", "dirs": []})
ftp_case("ftp-stor-error", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "pass", "dirs": ["/shots/sub"],
                                                    "fail_once": [["STOR", "451 Requested action aborted. Local error in processing."]]})
ftp_case("ftp-mkd-denied", [FTP_ACCOUNT], BIN_PNG, {"user": "user", "password": "pass", "dirs": [],
                                                    "fail_once": [["MKD shots", "550 Permission denied."]]})
ftp_case("ftps-explicit", [dict(FTP_ACCOUNT, Protocol="FTPS", Port=MOCK + 2, FTPSEncryption="Explicit")], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": []})
ftp_case("ftps-implicit", [dict(FTP_ACCOUNT, Protocol="FTPS", Port=MOCK + 3, FTPSEncryption="Implicit")], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/shots/sub"]})
ftp_case("ftps-explicit-active", [dict(FTP_ACCOUNT, Protocol="FTPS", Port=MOCK + 2, IsActive=True)], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/shots/sub"]})
ftp_case("ftp-second-account", [FTP_ACCOUNT, dict(FTP_ACCOUNT, Name="two", SubFolderPath="", HttpHomePath="cdn.example.com:8080/x?f=")],
         BIN_PNG, {"user": "user", "password": "pass", "dirs": []}, selected=1)
ftp_case("ftp-connection-refused", [dict(FTP_ACCOUNT, Port=MOCK + 9)], BIN_PNG)

SFTP_ACCOUNT = dict(FTP_ACCOUNT, Protocol="SFTP", Port=MOCK + 4, Name="sftp")
ftp_case("sftp-password-mkdir", [SFTP_ACCOUNT], BIN_PNG)
ftp_case("sftp-existing-dir", [SFTP_ACCOUNT], {"file_name": "big.bin", "content_base64": base64.b64encode(bytes(range(256)) * 200).decode()},
         {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"]})
KEYS = "crates/sharex-uploaders/tests/fixtures/"


def pub(name):
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", KEYS + name + ".pub"), encoding="utf-8") as f:
        return f.read().strip()


ftp_case("sftp-key-ed25519", [dict(SFTP_ACCOUNT, Password="", Keypath=KEYS + "sftp_test_ed25519")], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "authorized_key": pub("sftp_test_ed25519")})
ftp_case("sftp-key-rsa-passphrase", [dict(SFTP_ACCOUNT, Password="", Keypath=KEYS + "sftp_test_rsa", Passphrase="made-up-passphrase")], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "authorized_key": pub("sftp_test_rsa")})
ftp_case("sftp-key-pem", [dict(SFTP_ACCOUNT, Password="", Keypath=KEYS + "sftp_test_rsa_pem")], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "authorized_key": pub("sftp_test_rsa_pem")})
ftp_case("sftp-wrong-password", [dict(SFTP_ACCOUNT, Password="nope")], BIN_PNG)
ftp_case("sftp-key-missing", [dict(SFTP_ACCOUNT, Keypath=KEYS + "no_such_key")], BIN_PNG)
ftp_case("sftp-no-credentials", [dict(SFTP_ACCOUNT, Password="")], BIN_PNG)
ftp_case("sftp-mkdir-denied", [SFTP_ACCOUNT], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": [], "sftp_fail_once": [["MKDIR /home/user/shots", 3, "Permission denied"]]})
ftp_case("sftp-open-failure", [SFTP_ACCOUNT], BIN_PNG,
         {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "sftp_fail_once": [["OPEN", 4, "Failure"]]})
for kex in ["ecdh-sha2-nistp256", "ecdh-sha2-nistp384", "ecdh-sha2-nistp521", "diffie-hellman-group-exchange-sha256",
            "diffie-hellman-group16-sha512", "diffie-hellman-group18-sha512", "diffie-hellman-group14-sha256",
            "diffie-hellman-group-exchange-sha1", "diffie-hellman-group14-sha1", "diffie-hellman-group1-sha1"]:
    ftp_case("sftp-kex-" + kex.replace("diffie-hellman-", "dh-"), [SFTP_ACCOUNT], BIN_PNG,
             {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "sftp_kex": [kex]})
for cipher, mac in [("aes128-cbc", "hmac-sha1"), ("aes256-cbc", "hmac-sha2-512"), ("3des-cbc", "hmac-sha1"),
                    ("aes256-ctr", "hmac-sha2-256-etm@openssh.com"), ("aes192-cbc", "hmac-sha1-etm@openssh.com"),
                    ("aes128-ctr", "hmac-sha2-512-etm@openssh.com")]:
    ftp_case(f"sftp-cipher-{cipher}-{mac.replace('@openssh.com', '')}", [SFTP_ACCOUNT], BIN_PNG,
             {"user": "user", "password": "pass", "dirs": ["/home/user/shots/sub"], "sftp_kex": ["diffie-hellman-group14-sha256"],
              "sftp_ciphers": [cipher], "sftp_macs": [mac]})
ftp_case("sftp-absolute",[dict(SFTP_ACCOUNT, SubFolderPath="/srv/up", HttpHomePath="", BrowserProtocol="sftp")], image_input("é b.png"),
         {"user": "user", "password": "pass", "dirs": ["/srv"]})

# Email (SMTP with STARTTLS, automatic send: no window).
EMAIL = {"EmailSmtpServer": "127.0.0.1", "EmailSmtpPort": MOCK + 5, "EmailFrom": "sender@example.com", "EmailPassword": "pass",
         "EmailAutomaticSend": True, "EmailAutomaticSendTo": "someone@example.com", "EmailDefaultSubject": "Sending email from ShareX",
         "EmailDefaultBody": "Screenshot is attached."}
SMTP = {"user": "sender@example.com", "password": "pass"}
cases.append({"id": "email", "kind": "builtin", "service": "file:Email", "config": EMAIL, "input": BIN_PNG, "responses": [], "servers": SMTP})
cases.append({"id": "email-unicode", "kind": "builtin", "service": "file:Email",
              "config": dict(EMAIL, EmailDefaultSubject="Écran 🙂 capturé", EmailDefaultBody="Line 1\nLíne 2 — done"),
              "input": image_input("café shot.png"), "responses": [], "servers": SMTP})
cases.append({"id": "email-auth-failed", "kind": "builtin", "service": "file:Email", "config": dict(EMAIL, EmailPassword="nope"),
              "input": BIN_PNG, "responses": [], "servers": SMTP})
cases.append({"id": "email-invalid-from", "kind": "builtin", "service": "file:Email", "config": dict(EMAIL, EmailFrom="user"),
              "input": BIN_PNG, "responses": [], "servers": SMTP})
cases.append({"id": "email-long-subject", "kind": "builtin", "service": "file:Email",
              "config": dict(EMAIL, EmailDefaultSubject="Écran capturé — " * 9 + "fin 🙂🙂🙂 " + "x" * 30,
                             EmailDefaultBody="Body"), "input": BIN_PNG, "responses": [], "servers": SMTP})
cases.append({"id": "email-long-ascii-subject", "kind": "builtin", "service": "file:Email",
              "config": dict(EMAIL, EmailDefaultSubject="A long plain subject " * 8 + "end", EmailDefaultBody="Body"),
              "input": BIN_PNG, "responses": [], "servers": SMTP})
cases.append({"id": "email-share-url", "kind": "builtin", "service": "sharing:Email", "config": EMAIL, "input": url_input(), "responses": [],
              "servers": SMTP})

# MEGA: random file keys and request ids; compare.py decrypts each side's
# chunk, name and key with the made-up master key and checks the MAC.


def b64url(b):
    return base64.urlsafe_b64encode(b).decode().rstrip("=")


def aes_ecb(data, key):
    return subprocess.run(["openssl", "enc", "-aes-128-ecb", "-nopad", "-K", key.hex()], input=data, capture_output=True, check=True).stdout


MEGA_MASTER = bytes(range(16))
MEGA = {"MegaSessionID": "made-up-session-id", "MegaMasterKey": b64url(MEGA_MASTER)}
MEGA_TOKEN = {"status": 200, "headers": [["Content-Type", "application/octet-stream"]], "body_base64": base64.b64encode(bytes(range(36))).decode()}
MEGA_UPLOAD = [resp('[{"p":"https://gfs270n123.userstorage.mega.co.nz/ul/made-up-upload"}]'), MEGA_TOKEN,
               resp('[{"f":[{"h":"NODE1","t":0,"a":"x","k":"y"}]}]'), resp('["PUBHANDLE"]')]
builtin("mega-root", "file:Mega", MEGA, BIN_PNG, [resp('[{"f":[{"h":"INBOX","t":3},{"h":"ROOT1","t":2}]}]')] + MEGA_UPLOAD)
builtin("mega-folder", "file:Mega", dict(MEGA, MegaSelectedFolder={"ID": "FOLDER7", "Name": "Shots"}), image_input("é b.png"), MEGA_UPLOAD)
builtin("mega-session-expired", "file:Mega", MEGA, BIN_PNG, [resp("[-15]")])
builtin("mega-not-logged-in", "file:Mega", {"MegaSessionID": "", "MegaMasterKey": b64url(MEGA_MASTER)}, BIN_PNG, [])
builtin("mega-hashcash", "file:Mega", dict(MEGA, MegaSelectedFolder={"ID": "F", "Name": "x"}), BIN_PNG, [
    resp("", 402, [["X-Hashcash", "1:255:1700000000:" + b64url(bytes(range(48)))]])] + MEGA_UPLOAD)
MEGA_SALT = bytes(range(100, 132))
MEGA_PASSWORD = "made-up password"
MEGA_PWKEY = hashlib.pbkdf2_hmac("sha512", MEGA_PASSWORD.encode("latin-1") + b"\0" * ((-len(MEGA_PASSWORD)) % 4), MEGA_SALT, 100000, 32)[:16]
MEGA_TSID = bytes(range(200, 216))
oauth_case("signin-mega", "mega", [{"op": "login", "email": " Someone@Example.com ", "password": MEGA_PASSWORD}], [
    resp(json.dumps([{"v": 2, "s": b64url(MEGA_SALT)}])),
    resp(json.dumps([{"k": b64url(aes_ecb(MEGA_MASTER, MEGA_PWKEY)), "tsid": b64url(MEGA_TSID + aes_ecb(MEGA_TSID, MEGA_MASTER))}]))])
oauth_case("signin-mega-2fa", "mega", [{"op": "login", "email": "someone@example.com", "password": MEGA_PASSWORD}], [
    resp(json.dumps([{"v": 2, "s": b64url(MEGA_SALT)}])), resp("[-26]")])

# ---------------------------------------------------------------------------
# The remote storage browser (Tools › Remote storage browser): ShareX's own
# FTP/SFTP/Amazon S3 providers, step by step, against the mock servers.


def remote_case(id, provider, steps, servers=None, account=None, s3=None, responses=None, allow_upload=True):
    c = {"id": id, "kind": "remote", "provider": provider, "steps": steps, "allow_upload": allow_upload, "responses": responses or []}
    if account is not None:
        c["account"] = account
    if s3 is not None:
        c["s3"] = s3
    if servers is not None:
        c["servers"] = servers
    cases.append(c)


def item(path, is_folder=False, size=None):
    return {"name": path.rstrip("/").rsplit("/", 1)[-1], "path": path, "is_folder": is_folder, "size": size}


def b64(b):
    return base64.b64encode(b).decode()


FTP_FILES = {"/docs/a.txt": b64(b"hello"), "/docs/sub/deep.bin": b64(bytes(range(64))), "/top.png": BIN_PNG["content_base64"],
             "/Zeta.txt": b64(b"z"), "/alpha.txt": b64(b"")}
FTP_BROWSE = [
    {"op": "display_path", "path": ""}, {"op": "display_path", "path": "/docs/sub/"}, {"op": "display_path", "path": "a\\b"},
    {"op": "parent_path", "path": ""}, {"op": "parent_path", "path": "docs"}, {"op": "parent_path", "path": "/docs/sub/"},
    {"op": "list", "path": ""}, {"op": "list", "path": "docs"}, {"op": "list", "path": "missing"},
    {"op": "has_children", "item": item("docs", True)}, {"op": "has_children", "item": item("empty", True)},
    {"op": "has_children", "item": item("top.png")},
    {"op": "download", "item": item("docs/a.txt")}, {"op": "download", "item": item("docs")},
    {"op": "url", "item": item("docs/a b.txt")}, {"op": "url", "item": item("top.png")}, {"op": "url", "item": item("docs", True)},
    {"op": "upload", "dir": "docs", "name": "new file.bin", "content_base64": b64(bytes(range(200)))},
    {"op": "upload", "dir": "newdir/deeper", "name": "x.txt", "content_base64": b64(b"x")},
    {"op": "create_folder", "dir": "", "name": "made"}, {"op": "create_folder", "dir": "docs", "name": "a/b"},
    {"op": "create_folder", "dir": "", "name": ".."}, {"op": "create_folder", "dir": "made", "name": "inner"},
    {"op": "rename", "item": item("docs/a.txt"), "name": "b.txt"}, {"op": "rename", "item": item("missing.txt"), "name": "c.txt"},
    {"op": "delete", "item": item("docs/b.txt")}, {"op": "delete", "item": item("docs", True)},
    {"op": "delete", "item": item("nothing.txt")}, {"op": "list", "path": ""},
]
FTP_SERVERS = {"user": "user", "password": "pass", "dirs": ["/empty"], "ftp_files": FTP_FILES}
FTP_REMOTE = dict(FTP_ACCOUNT, SubFolderPath="", HttpHomePath="files.example.com/~me")
remote_case("remote-ftp-browse", "ftp", FTP_BROWSE, FTP_SERVERS, FTP_REMOTE)
remote_case("remote-ftp-mlsd", "ftp", FTP_BROWSE, dict(FTP_SERVERS, ftp_mlsd=True, ftp_greeting="220---------- Welcome to Pure-FTPd [privsep] [TLS] ----------\r\n"
                                                                    "220 You will be disconnected after 15 minutes of inactivity."),
            dict(FTP_REMOTE, Name=""))
remote_case("remote-ftp-iis", "ftp", [{"op": "list", "path": ""}, {"op": "list", "path": "docs"}, {"op": "has_children", "item": item("docs", True)}],
            dict(FTP_SERVERS, ftp_windows_list=True, ftp_greeting="220 Microsoft FTP Service", ftp_syst="215 Windows_NT"), FTP_REMOTE)
remote_case("remote-ftp-listing-formats", "ftp", [{"op": "list", "path": ""}, {"op": "list", "path": "win"}, {"op": "list", "path": "odd"}],
            dict(FTP_SERVERS, ftp_listings={
                "/": ["total 12",
                      "drwxr-xr-x    2 1000     1000         4096 Mar 04  2025 dir one",
                      "-rw-r--r--    1 1000     1000       123456 Jan 15 10:30 recent.txt",
                      "-rw-r--r--    1 owner    group         7 Dec  1  1999 old.txt",
                      "lrwxrwxrwx    1 1000     1000           11 Feb 02  2024 link -> target/file",
                      "-rw-r--r--    1 ftp      ftp      1.234.567 Jul 07 07:07 dotted size.bin",
                      "drwxr-xr-x    3 user     group        4096 May 31 23:59 .",
                      "drwxr-xr-x    3 user     group        4096 May 31 23:59 ..",
                      "-rw-r--r--   1 user  group  9  Aug  9  2021  spaced   name .txt",
                      "crw-rw-rw-    1 0        0          1, 3 Mar 04  2025 null",
                      "-rw-r--r-- 1 0 0 5 2025-03-04 05:06 iso.txt"],
                "/win": ["03-04-25  05:06AM       <DIR>          Folder A",
                         "12-31-1999  11:59PM              1,234 old file.txt",
                         "2025-03-04  17:06            5 iso time.txt"],
                "/odd": ["été.txt", "garbage line", "-rwxr-xr-x 1 u g 10 Mar 4 2025 x"]}), FTP_REMOTE)
remote_case("remote-ftp-upload-disabled", "ftp", [{"op": "upload", "dir": "", "name": "x.txt", "content_base64": b64(b"x")},
                                                  {"op": "list", "path": "docs"}], FTP_SERVERS, FTP_REMOTE, allow_upload=False)
remote_case("remote-ftps-explicit", "ftp", [{"op": "list", "path": "docs"}, {"op": "download", "item": item("docs/a.txt")},
                                            {"op": "upload", "dir": "docs", "name": "up.bin", "content_base64": b64(b"up")}],
            FTP_SERVERS, dict(FTP_REMOTE, Protocol="FTPS", Port=MOCK + 2, FTPSEncryption="Explicit"))
remote_case("remote-ftp-active", "ftp", [{"op": "list", "path": "docs"}, {"op": "download", "item": item("docs/sub/deep.bin")}],
            FTP_SERVERS, dict(FTP_REMOTE, IsActive=True))
remote_case("remote-ftp-login-failed", "ftp", [{"op": "list", "path": ""}, {"op": "download", "item": item("top.png")}],
            FTP_SERVERS, dict(FTP_REMOTE, Password="wrong"))
remote_case("remote-ftp-sftp-account", "ftp", [], FTP_SERVERS, dict(FTP_REMOTE, Protocol="SFTP"))
DELETE_TREE = [{"op": "create_folder", "dir": "docs/sub", "name": "deeper"}, {"op": "delete", "item": item("docs", True)},
               {"op": "list", "path": ""}]
remote_case("remote-ftp-pureftpd-recursive", "ftp", DELETE_TREE,
            dict(FTP_SERVERS, ftp_greeting="220---------- Welcome to Pure-FTPd [privsep] [TLS] ----------\r\n220 Local time is now 10:00."),
            FTP_REMOTE)
remote_case("remote-ftp-proftpd-site", "ftp", DELETE_TREE + [{"op": "create_folder", "dir": "new/deep", "name": "x"}],
            dict(FTP_SERVERS, ftp_greeting="220 ProFTPD Server (ProFTPD Default Installation) [127.0.0.1]",
                 ftp_features=["SITE MKDIR", "SITE RMDIR"]), FTP_REMOTE)
remote_case("remote-ftp-servu-rmda", "ftp", DELETE_TREE,
            dict(FTP_SERVERS, ftp_greeting="220 Serv-U FTP Server v15.4 ready...", ftp_features=["RMDA directoryname"]), FTP_REMOTE)
remote_case("remote-ftp-no-utf8", "ftp", [{"op": "list", "path": ""}, {"op": "list", "path": "docs"}],
            dict(FTP_SERVERS, ftp_utf8=False, ftp_files=dict(FTP_FILES, **{"/docs/été.txt": b64(b"e")})), FTP_REMOTE)
remote_case("remote-ftp-vms", "ftp", [{"op": "list", "path": ""}],
            dict(FTP_SERVERS, ftp_syst="215 VMS OpenVMS-like", ftp_listings={"/": [
                "Directory DISK$USER:[USER]", "",
                "FILE.TXT;1           2/4      14-JUN-2004 10:27:08  [GROUP,OWNER]  (RWED,RWED,RE,)",
                "SUBDIR.DIR;1         1/1       1-JAN-2025 00:00:00  [GROUP,OWNER]  (RWE,RWE,RE,E)",
                "BIG.DAT;3          123      31-DEC-1999 23:59:59  [OWNER]        (RWED,RWED,,)",
                "Total of 3 files, 126/129 blocks."]}), FTP_REMOTE)
remote_case("remote-ftp-nonstop", "ftp", [{"op": "list", "path": ""}],
            dict(FTP_SERVERS, ftp_syst="215 NONSTOP Type: L8", ftp_listings={"/": [
                "File         Code             EOF  Last Modification    Owner  RWEP",
                "TEST        101             807 11-Aug-25 13:56:09 255,255 \"nnnn\"",
                "OTHER       0             12345 1-Jan-99 00:00:00 1,2 \"oooo\""]}), FTP_REMOTE)
remote_case("remote-ftp-os400", "ftp", [{"op": "list", "path": ""}],
            dict(FTP_SERVERS, ftp_syst="215 UNKNOWN", ftp_listings={"/": [
                "QSYS            77824 02/23/00 15:09:55 *DIR       QDOC/",
                "QSYS            12288 23/02/00 15:09:55 *STMF      a file.txt",
                "QPGMR               0 01/01/24 00:00:00 *FILE      MYLIB.FILE/",
                "QPGMR                                   *MEM       MYLIB.FILE/MBR.MBR"]}), FTP_REMOTE)
SFTP_FILES = {"/home/user" + k: v for k, v in FTP_FILES.items()}
SFTP_SERVERS = {"user": "user", "password": "pass", "dirs": ["/home/user/empty"], "sftp_files": SFTP_FILES}
SFTP_REMOTE = dict(SFTP_ACCOUNT, SubFolderPath="", HttpHomePath="files.example.com/~me")
SFTP_BROWSE = [s for s in FTP_BROWSE if s["op"] != "upload" or s["dir"] == "docs"] + [
    {"op": "upload", "dir": "missing", "name": "x.txt", "content_base64": b64(b"x")},
    {"op": "upload", "dir": "", "name": "big.bin", "content_base64": b64(bytes(range(256)) * 300)},
    {"op": "download", "item": item("big.bin")}, {"op": "rename", "item": item("empty", True), "name": "made"},
    {"op": "rename", "item": item("made", True), "name": "renamed"}, {"op": "delete", "item": item("renamed", True)}]
remote_case("remote-sftp-browse", "sftp", SFTP_BROWSE, SFTP_SERVERS, SFTP_REMOTE)
remote_case("remote-sftp-no-credentials", "sftp", [{"op": "list", "path": ""}, {"op": "delete", "item": item("docs", True)}],
            SFTP_SERVERS, dict(SFTP_REMOTE, Password=""))
remote_case("remote-sftp-wrong-password", "sftp", [{"op": "list", "path": ""}, {"op": "list", "path": "docs"}],
            SFTP_SERVERS, dict(SFTP_REMOTE, Password="nope"))
remote_case("remote-sftp-key", "sftp", [{"op": "list", "path": "docs"}, {"op": "download", "item": item("docs/a.txt")}],
            dict(SFTP_SERVERS, authorized_key=pub("sftp_test_ed25519")), dict(SFTP_REMOTE, Password="", Keypath=KEYS + "sftp_test_ed25519"))
remote_case("remote-sftp-failures", "sftp", [{"op": "list", "path": "docs"}, {"op": "download", "item": item("docs/a.txt")},
                                             {"op": "delete", "item": item("docs/a.txt")}, {"op": "create_folder", "dir": "docs", "name": "n"}],
            dict(SFTP_SERVERS, sftp_fail_once=[["OPENDIR", 3, "Permission denied"], ["OPEN", 4, "Failure"], ["REMOVE", 3, "Permission denied"],
                                               ["MKDIR", 4, "Failure"]]), SFTP_REMOTE)


def s3_list(contents=(), prefixes=(), truncated=False, token=None):
    body = '<?xml version="1.0" encoding="UTF-8"?>\n<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>made-up-bucket</Name>'
    body += f"<IsTruncated>{'true' if truncated else 'false'}</IsTruncated>"
    if token:
        body += f"<NextContinuationToken>{token}</NextContinuationToken>"
    for key, size, date in contents:
        body += f"<Contents><Key>{key}</Key><LastModified>{date}</LastModified><ETag>&quot;e&quot;</ETag><Size>{size}</Size>" \
                "<StorageClass>STANDARD</StorageClass></Contents>"
    for p in prefixes:
        body += f"<CommonPrefixes><Prefix>{p}</Prefix></CommonPrefixes>"
    return resp(body + "</ListBucketResult>", headers=XML)


S3_DATE = "2025-03-04T05:06:07.000Z"
S3_NO_CONTENT = resp("", 204, headers=[])
S3_COPIED = resp('<?xml version="1.0" encoding="UTF-8"?><CopyObjectResult><LastModified>2025-03-04T05:06:07.000Z</LastModified>'
                 '<ETag>"e"</ETag></CopyObjectResult>', headers=XML)
S3_REMOTE = dict(S3, ObjectPrefix="")
remote_case("remote-s3-browse", "s3", [
    {"op": "display_path", "path": ""}, {"op": "display_path", "path": "a\\b/c"}, {"op": "parent_path", "path": ""},
    {"op": "parent_path", "path": "a/"}, {"op": "parent_path", "path": "/a/b/"}, {"op": "parent_path", "path": "a/b.txt"},
    {"op": "list", "path": ""}, {"op": "list", "path": "docs/"}, {"op": "has_children", "item": item("docs/", True)},
    {"op": "has_children", "item": item("empty/", True)}, {"op": "has_children", "item": item("top.png")},
    {"op": "download", "item": item("docs/a.txt")}, {"op": "download", "item": item("missing.txt")},
    {"op": "url", "item": item("docs/a b+c.txt")}, {"op": "url", "item": item("docs/", True)},
    {"op": "upload", "dir": "docs", "name": "new file.bin", "content_base64": b64(bytes(range(200)))},
    {"op": "create_folder", "dir": "", "name": "made"}, {"op": "create_folder", "dir": "docs", "name": "a/b"},
    {"op": "rename", "item": item("docs/a.txt"), "name": "b.txt"}, {"op": "rename", "item": item("docs/", True), "name": "renamed"},
    {"op": "delete", "item": item("top.png")}, {"op": "delete", "item": item("renamed/", True)},
    {"op": "list", "path": "denied"}, {"op": "rename", "item": item("x.txt"), "name": "y.txt"},
], s3=S3_REMOTE, responses=[
    s3_list([("top.png", 768, S3_DATE), ("Zeta.txt", 1, S3_DATE), ("folder/", 0, S3_DATE), ("my+doc.txt", 3, "2025-03-04T05:06:07Z")],
            ["docs/", "empty/", "folder/"], True, "tok/1+="),
    s3_list([("alpha.txt", 0, S3_DATE), ("%C3%A9t%C3%A9.txt", 4, S3_DATE)]),
    s3_list([("docs/", 0, S3_DATE), ("docs/a.txt", 5, S3_DATE), ("docs/a%20b.txt", 7, "not a date")], ["docs/sub/"]),
    s3_list([("docs/", 0, S3_DATE), ("docs/a.txt", 5, S3_DATE)]),
    s3_list([("empty/", 0, S3_DATE)]),
    resp("hello", headers=[["Content-Type", "text/plain"]]),
    resp('<?xml version="1.0" encoding="UTF-8"?><Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message></Error>',
         404, XML),
    resp("", headers=[]), resp("", headers=[]),
    S3_COPIED, S3_NO_CONTENT,
    s3_list([("docs/", 0, S3_DATE), ("docs/b.txt", 5, S3_DATE), ("docs/sub/x.bin", 9, S3_DATE)]),
    S3_COPIED, S3_COPIED, S3_COPIED, S3_NO_CONTENT, S3_NO_CONTENT, S3_NO_CONTENT,
    S3_NO_CONTENT,
    s3_list([("renamed/", 0, S3_DATE), ("renamed/b.txt", 5, S3_DATE)]), S3_NO_CONTENT, S3_NO_CONTENT,
    resp('<?xml version="1.0" encoding="UTF-8"?><Error><Code>AccessDenied</Code><Message>Access Denied</Message></Error>', 403, XML),
    resp('<?xml version="1.0" encoding="UTF-8"?><Error><Code>InternalError</Code></Error>', headers=XML),
])
remote_case("remote-s3-path-style-errors", "s3", [
    {"op": "list", "path": ""}, {"op": "rename", "item": item("a.txt"), "name": "b.txt"},
    {"op": "upload", "dir": "", "name": "c.txt", "content_base64": b64(b"c")}, {"op": "delete", "item": item("d/", True)},
    {"op": "list", "path": "truncated"},
], s3=dict(S3_REMOTE, Bucket="made.up.bucket", Endpoint="https://minio.example.com/", Region="eu-central-1", SignedPayload=True,
           UseCustomCNAME=True, CustomDomain="cdn.example.com"), responses=[
    resp("not xml", headers=XML),
    S3_COPIED, resp("", 500, headers=[]), S3_NO_CONTENT,
    resp("", 503, headers=[]),
    s3_list([("d/", 0, S3_DATE), ("d/x", 1, S3_DATE)]), S3_NO_CONTENT, resp("<Error/>", 403, XML),
    s3_list([("t/a", 1, S3_DATE)], truncated=True),
], allow_upload=True)
remote_case("remote-s3-upload-disabled", "s3", [{"op": "upload", "dir": "", "name": "c.txt", "content_base64": b64(b"c")}],
            s3=S3_REMOTE, allow_upload=False)
remote_case("remote-ftp-sanitize", "ftp", [{"op": "list", "path": "a%20b"}, {"op": "list", "path": "x;y"}, {"op": "list", "path": "../etc"},
                                           {"op": "download", "item": item("docs/../top.png")}], FTP_SERVERS, FTP_REMOTE)

# ---------------------------------------------------------------------------
# Destination settings' pickers: Imgur's albums ("Refresh albums") and an
# album's images, MEGA's folder tree.


def listing_case(id, service, steps, responses, **extra):
    c = {"id": id, "kind": "listing", "service": service, "steps": steps, "responses": responses}
    c.update(extra)
    cases.append(c)


def imgur_ok(data):
    return resp(json.dumps({"data": data, "success": True, "status": 200}))


ALBUM_A = {"id": "abc12", "title": "Shots", "description": None, "datetime": 1700000000, "cover": "cov1", "cover_width": 640,
           "cover_height": "480", "account_url": "someone", "account_id": 12345, "privacy": "hidden", "layout": "blog", "views": 7,
           "link": "https://imgur.com/a/abc12", "favorite": False, "nsfw": None, "section": None, "order": 0, "deletehash": "dh1",
           "images_count": 2, "in_gallery": False, "is_ad": False}
ALBUM_B = dict(ALBUM_A, id="def34", title="Café \"quoted\" <b>", datetime="1700000001", favorite="true", nsfw=True, order=3,
               images=[{"id": "img1", "title": None, "datetime": 1700000002, "type": "image/png", "animated": False, "width": 10,
                        "height": 20, "size": 300, "views": 0, "bandwidth": 12345678901, "link": "https://i.imgur.com/img1.png",
                        "nsfw": None, "vote": None}])
ALBUM_C = dict(ALBUM_A, id="ghi56", title="2024-01-02T03:04:05Z", account_id=None, description="x\ny")
listing_case("listing-imgur-albums", "imgur", [{"op": "albums", "max_page": 3, "per_page": 2}, {"op": "albums"},
                                               {"op": "album_images", "album_id": "def34"}],
             [imgur_ok([ALBUM_A, ALBUM_B]), imgur_ok([ALBUM_C]), imgur_ok([]), imgur_ok(ALBUM_B["images"] * 2)], oauth2=oauth2())
listing_case("listing-imgur-refresh-and-errors", "imgur", [{"op": "albums"}, {"op": "album_images", "album_id": "zz"},
                                                           {"op": "albums"}, {"op": "album_images", "album_id": "yy"}],
             [resp(json.dumps({"access_token": "new-access", "refresh_token": "new-refresh", "expires_in": 2419200,
                               "token_type": "bearer"})),
              resp(json.dumps({"data": {"error": "Unauthorized", "request": "/3/account/me/albums", "method": "GET"}, "success": False,
                               "status": 403})),
              resp(json.dumps({"data": {"error": {"code": 1003, "message": "Album not found", "type": "Exception"}, "request": "/3/album/zz/images",
                               "method": "GET"}, "success": False, "status": 404}), 404),
              imgur_ok({"not": "an array"}),
              resp("<html>bad gateway</html>", 502, [["Content-Type", "text/html"]])],
             oauth2=dict(oauth2(), Token=dict(oauth2()["Token"], ExpireDate=PAST)))
listing_case("listing-imgur-not-signed-in", "imgur", [{"op": "albums"}], [], oauth2=dict(oauth2(), Token=None))


def aes_cbc(data, key):
    return subprocess.run(["openssl", "enc", "-aes-128-cbc", "-nopad", "-K", key.hex(), "-iv", "00" * 16], input=data,
                          capture_output=True, check=True).stdout


def mega_folder(handle, parent, name, key_byte, owner="u1"):
    key = bytes([key_byte]) * 16
    attrs = ("MEGA" + json.dumps({"n": name}, ensure_ascii=False, separators=(",", ":"))).encode("utf-8")
    attrs += b"\0" * ((-len(attrs)) % 16)
    return {"h": handle, "p": parent, "t": 1, "a": b64url(aes_cbc(attrs, key)), "k": f"{owner}:{b64url(aes_ecb(key, MEGA_MASTER))}",
            "ts": 1700000000}


MEGA_NODES = {"f": [
    {"h": "ROOT1", "t": 2}, {"h": "INBOX", "t": 3}, {"h": "BIN", "t": 4},
    mega_folder("F1", "ROOT1", "zeta shots", 1), mega_folder("F2", "ROOT1", "Alpha", 2), mega_folder("F3", "ROOT1", "été \U0001F600", 3),
    dict(mega_folder("F4", "ROOT1", "bad key", 4), k="u1:notbase64!!/u2:" + b64url(aes_ecb(bytes([4]) * 16, MEGA_MASTER))),
    dict(mega_folder("F5", "ROOT1", "no attributes", 5), a=""), mega_folder("F6", "F1", "inside zeta", 6),
    dict(mega_folder("F7", "ROOT1", "wrong key", 7), k="u1:" + b64url(bytes(16))), mega_folder("", "ROOT1", "no handle", 8),
    {"h": "FILE1", "p": "ROOT1", "t": 0, "a": "x", "k": "y"},
]}
listing_case("listing-mega-folders", "mega", [{"parent": None}, {"parent": {"id": "F1", "name": "zeta shots"}}, {"parent": None},
                                              {"parent": {"id": "", "name": ""}}],
             [resp(json.dumps([MEGA_NODES])), resp(json.dumps([MEGA_NODES])), resp("[-15]"),
              resp(json.dumps([{"f": [{"h": "INBOX", "t": 3}]}]))],
             session_id="made-up-session-id", master_key=b64url(MEGA_MASTER))
listing_case("listing-mega-not-logged-in", "mega", [{"parent": None}], [], session_id="", master_key=b64url(MEGA_MASTER))

here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "cases.json"), "w", encoding="utf-8", newline="\n") as f:
    json.dump({"cases": cases}, f, ensure_ascii=False, indent=1)
print(f"{len(cases)} cases")
