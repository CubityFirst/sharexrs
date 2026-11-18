// "oauth" cases: ShareX's own sign-in code (authorization URLs, code and PIN
// exchange, refresh, Google's user info, OAuth 1.0a request and access
// tokens) against the mock token endpoints. The loopback listener itself
// needs a browser, so the redirect URI and state are given.

using System.Reflection;
using System.Text.Json.Nodes;
using Newtonsoft.Json;
using ShareX.UploadersLib;
using ShareX.UploadersLib.FileUploaders;
using ShareX.UploadersLib.ImageUploaders;
using ShareX.UploadersLib.TextUploaders;
using ShareX.UploadersLib.URLShorteners;

static class OAuthCases
{
    public static void Run(JsonObject c, JsonObject output, ref Uploader uploader)
    {
        string service = (string)c["service"];
        OAuth2Info info2 = c["oauth2"] != null ? JsonConvert.DeserializeObject<OAuth2Info>(c["oauth2"].ToJsonString()) : null;
        OAuthInfo info1 = c["oauth1"] != null ? JsonConvert.DeserializeObject<OAuthInfo>(c["oauth1"].ToJsonString()) : null;
        if (c["proof"] is JsonObject pj)
        {
            OAuth2ProofKey proof = new OAuth2ProofKey(OAuth2ChallengeMethod.SHA256);
            typeof(OAuth2ProofKey).GetProperty("CodeVerifier").SetValue(proof, (string)pj["verifier"]);
            typeof(OAuth2ProofKey).GetProperty("CodeChallenge").SetValue(proof, (string)pj["challenge"]);
            info2.Proof = proof;
        }

        if (service == "mega")
        {
            JsonArray megaSteps = new JsonArray();
            output["steps"] = megaSteps;
            foreach (JsonNode step in c["steps"].AsArray())
            {
                var mega = new Mega((string)step["email"], (string)step["password"]);
                uploader = mega;
                MethodInfo login = typeof(Mega).GetMethod("LoginAsync", BindingFlags.NonPublic | BindingFlags.Instance);
                try
                {
                    Task task = (Task)login.Invoke(mega, new object[] { null, CancellationToken.None });
                    task.GetAwaiter().GetResult();
                    object session = task.GetType().GetProperty("Result").GetValue(task);
                    megaSteps.Add((string)session.GetType().GetProperty("SessionID").GetValue(session) + "|" + (string)session.GetType().GetProperty("MasterKey").GetValue(session));
                }
                catch (TargetInvocationException e)
                {
                    throw e.InnerException;
                }
            }
            return;
        }
        object client;
        Photobucket photobucket = null;
        switch (service)
        {
            case "imgur": { var u = new Imgur(info2); client = u; uploader = u; break; }
            case "gist": { var u = new GitHubGist(info2); client = u; uploader = u; break; }
            case "dropbox": { var u = new Dropbox(info2); client = u; uploader = u; break; }
            case "onedrive": { var u = new OneDrive(info2); client = u; uploader = u; break; }
            case "box": { var u = new Box(info2); client = u; uploader = u; break; }
            case "bitly": { var u = new BitlyURLShortener(info2); client = u; uploader = u; break; }
            case "google-drive": { var u = new GoogleDrive(info2); client = u.OAuth2; uploader = u; break; }
            case "google-cloud-storage": { var u = new GoogleCloudStorage(info2); client = u.OAuth2; uploader = u; break; }
            case "youtube": { var u = new YouTube(info2); client = u.OAuth2; uploader = u; break; }
            case "flickr": { var u = new FlickrUploader(info1); client = u; uploader = u; break; }
            case "photobucket": { var u = new Photobucket(info1); client = u; uploader = u; photobucket = u; break; }
            default: throw new Exception("unknown service " + service);
        }
        if (client is IOAuth2Loopback loopback && c["redirect"] is JsonObject r)
        {
            loopback.RedirectURI = (string)r["uri"];
            loopback.State = (string)r["state"];
        }

        JsonArray steps = new JsonArray();
        output["steps"] = steps;
        try
        {
            foreach (JsonNode step in c["steps"].AsArray())
            {
                string op = (string)step["op"];
                switch (op)
                {
                    case "authorization_url":
                        steps.Add(((IOAuthBase)client).GetAuthorizationURLAsync().GetAwaiter().GetResult());
                        break;
                    case "access_token":
                        steps.Add(((IOAuthBase)client).GetAccessTokenAsync((string)step["code"]).GetAwaiter().GetResult());
                        break;
                    case "refresh":
                        steps.Add(((IOAuth2)client).RefreshAccessTokenAsync().GetAwaiter().GetResult());
                        break;
                    case "check":
                        steps.Add(((IOAuth2)client).CheckAuthorizationAsync().GetAwaiter().GetResult());
                        break;
                    case "user_info":
                        OAuthUserInfo user = ((IOAuth2Loopback)client).GetUserInfoAsync().GetAwaiter().GetResult();
                        steps.Add(user == null ? null : $"{user.sub}|{user.name}|{user.given_name}|{user.picture}|{user.locale}");
                        break;
                    default:
                        throw new Exception("unknown op " + op);
                }
            }
        }
        finally
        {
            output["auth"] = info2 != null ? Token(info2) : OAuth1(info1, photobucket);
        }
    }

    static JsonObject Token(OAuth2Info info)
    {
        OAuth2Token t = info.Token;
        return new JsonObject
        {
            ["access_token"] = t?.access_token,
            ["refresh_token"] = t?.refresh_token,
            ["expires_in"] = t?.expires_in,
            ["token_type"] = t?.token_type,
            ["scope"] = t?.scope,
            ["has_expire_date"] = t == null ? null : t.ExpireDate != DateTime.MinValue,
            ["expired"] = t?.IsExpired,
        };
    }

    static JsonObject OAuth1(OAuthInfo info, Photobucket photobucket)
    {
        JsonObject o = new JsonObject
        {
            ["AuthToken"] = info.AuthToken,
            ["AuthSecret"] = info.AuthSecret,
            ["AuthVerifier"] = info.AuthVerifier,
            ["UserToken"] = info.UserToken,
            ["UserSecret"] = info.UserSecret,
        };
        if (photobucket != null)
        {
            o["Subdomain"] = photobucket.AccountInfo.Subdomain;
            o["AlbumID"] = photobucket.AccountInfo.AlbumID;
        }
        return o;
    }
}
