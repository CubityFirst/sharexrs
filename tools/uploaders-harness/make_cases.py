"""Writes cases.json: what both harnesses upload, with which settings, and the
canned responses the mock server answers with.

Every value is made up: no real keys, accounts or personal data. URLs point
at real services' hosts only so the requests match ShareX's; the harnesses
rewrite them to the local mock server before anything is sent.

Usage: python make_cases.py  (writes cases.json beside it)
"""

import base64
import json
import os

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

here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "cases.json"), "w", encoding="utf-8", newline="\n") as f:
    json.dump({"cases": cases}, f, ensure_ascii=False, indent=1)
print(f"{len(cases)} cases")
