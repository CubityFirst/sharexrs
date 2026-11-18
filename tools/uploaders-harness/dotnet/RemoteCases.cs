// "remote" cases: ShareX's remote storage browser providers (ShareX.dll's
// FTPRemoteStorageProvider, SFTPRemoteStorageProvider and
// AmazonS3RemoteStorageProvider, internal, made by reflection) driven step
// by step against the mock servers, as the browser window calls them.
// "listing" cases: the destination pickers' lists (Imgur's albums and an
// album's images, MEGA's folders).

using System.Reflection;
using System.Text;
using System.Text.Json.Nodes;
using Newtonsoft.Json;
using ShareX.Tools;
using ShareX.UploadersLib;
using ShareX.UploadersLib.FileUploaders;
using ShareX.UploadersLib.ImageUploaders;

static class RemoteCases
{
    static readonly Assembly ShareXAssembly = typeof(ShareX.TaskInfo).Assembly;

    static IRemoteStorageProvider Create(JsonObject c)
    {
        string provider = (string)c["provider"];
        bool allowUpload = (bool?)c["allow_upload"] ?? true;
        object settings;
        string typeName;
        switch (provider)
        {
            case "ftp":
                typeName = "ShareX.FTPRemoteStorageProvider";
                settings = JsonConvert.DeserializeObject<FTPAccount>(c["account"]!.ToJsonString());
                break;
            case "sftp":
                typeName = "ShareX.SFTPRemoteStorageProvider";
                settings = JsonConvert.DeserializeObject<FTPAccount>(c["account"]!.ToJsonString());
                break;
            case "s3":
                typeName = "ShareX.AmazonS3RemoteStorageProvider";
                settings = JsonConvert.DeserializeObject<AmazonS3Settings>(c["s3"]!.ToJsonString());
                break;
            default:
                throw new Exception("unknown provider " + provider);
        }
        Type type = ShareXAssembly.GetType(typeName, true);
        try
        {
            return (IRemoteStorageProvider)Activator.CreateInstance(type, BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance,
                null, new object[] { settings, allowUpload }, null);
        }
        catch (TargetInvocationException e)
        {
            throw e.InnerException;
        }
    }

    static RemoteStorageItem Item(JsonNode n) =>
        new RemoteStorageItem((string)n["name"], (string)n["path"], (bool)n["is_folder"], (long?)n["size"], null);

    static JsonObject ItemJson(RemoteStorageItem i) => new JsonObject
    {
        ["name"] = i.Name,
        ["path"] = i.Path,
        ["is_folder"] = i.IsFolder,
        ["size"] = i.Size,
        ["modified"] = i.Modified?.ToString("o"),
    };

    static JsonObject Error(Exception e)
    {
        Exception inner = e is AggregateException ae && ae.InnerException != null ? ae.InnerException : e;
        if (inner is TargetInvocationException t && t.InnerException != null) inner = t.InnerException;
        return new JsonObject { ["exception"] = inner.Message, ["exception_type"] = inner.GetType().FullName };
    }

    public static void Run(JsonObject c, JsonObject output)
    {
        JsonArray steps = new JsonArray();
        output["steps"] = steps;
        IRemoteStorageProvider provider;
        try
        {
            provider = Create(c);
        }
        catch (Exception e)
        {
            output["create"] = Error(e);
            return;
        }
        output["display_name"] = provider.DisplayName;
        output["root_display_name"] = provider.RootDisplayName;
        output["root_path"] = provider.RootPath;
        output["capabilities"] = (int)provider.Capabilities;
        try
        {
            foreach (JsonNode step in c["steps"]!.AsArray())
            {
                steps.Add(RunStep(provider, step!.AsObject()));
            }
        }
        finally
        {
            (provider as IDisposable)?.Dispose();
        }
    }

    static JsonNode RunStep(IRemoteStorageProvider p, JsonObject step)
    {
        string op = (string)step["op"];
        try
        {
            switch (op)
            {
                case "display_path":
                    return p.GetDisplayPath((string)step["path"]);
                case "parent_path":
                    return p.GetParentPath((string)step["path"]);
                case "url":
                    return p.GetUrl(Item(step["item"]));
                case "list":
                {
                    JsonArray items = new JsonArray();
                    foreach (RemoteStorageItem i in p.GetItemsAsync((string)step["path"]).GetAwaiter().GetResult()) items.Add(ItemJson(i));
                    return items;
                }
                case "has_children":
                    return p.HasChildrenAsync(Item(step["item"])).GetAwaiter().GetResult();
                case "download":
                {
                    using MemoryStream ms = new MemoryStream();
                    p.DownloadAsync(Item(step["item"]), ms).GetAwaiter().GetResult();
                    return Convert.ToBase64String(ms.ToArray());
                }
                case "upload":
                {
                    using MemoryStream ms = new MemoryStream(Convert.FromBase64String((string)step["content_base64"]));
                    p.UploadAsync((string)step["dir"], (string)step["name"], ms).GetAwaiter().GetResult();
                    return "ok";
                }
                case "create_folder":
                    p.CreateFolderAsync((string)step["dir"], (string)step["name"]).GetAwaiter().GetResult();
                    return "ok";
                case "rename":
                    p.RenameAsync(Item(step["item"]), (string)step["name"]).GetAwaiter().GetResult();
                    return "ok";
                case "delete":
                    p.DeleteAsync(Item(step["item"])).GetAwaiter().GetResult();
                    return "ok";
                default:
                    throw new Exception("unknown op " + op);
            }
        }
        catch (Exception e)
        {
            return Error(e);
        }
    }

    // ---------------------------------------------------------------- listings

    public static void RunListing(JsonObject c, JsonObject output, ref Uploader uploader)
    {
        string service = (string)c["service"];
        JsonArray steps = new JsonArray();
        output["steps"] = steps;
        if (service == "imgur")
        {
            OAuth2Info info = JsonConvert.DeserializeObject<OAuth2Info>(c["oauth2"]!.ToJsonString());
            Imgur imgur = new Imgur(info);
            uploader = imgur;
            foreach (JsonNode step in c["steps"]!.AsArray())
            {
                string op = (string)step!["op"];
                try
                {
                    object r = op switch
                    {
                        "albums" => step["max_page"] != null
                            ? imgur.GetAlbumsAsync((int)step["max_page"], (int)step["per_page"]).GetAwaiter().GetResult()
                            : imgur.GetAlbumsAsync().GetAwaiter().GetResult(),
                        "album_images" => imgur.GetAlbumImagesAsync((string)step["album_id"]).GetAwaiter().GetResult(),
                        _ => throw new Exception("unknown op " + op),
                    };
                    steps.Add(JsonConvert.SerializeObject(r));
                }
                catch (Exception e)
                {
                    steps.Add(Error(e));
                }
            }
            output["token"] = OAuthCases.Token(info);
            return;
        }
        if (service == "mega")
        {
            byte[] masterKey = (byte[])typeof(Mega).GetMethod("FromBase64URL", BindingFlags.NonPublic | BindingFlags.Static)
                .Invoke(null, new object[] { (string)c["master_key"] });
            Mega mega = (Mega)typeof(Mega).GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, null, new[] { typeof(string), typeof(byte[]) }, null)
                .Invoke(new object[] { (string)c["session_id"], masterKey });
            uploader = mega;
            MethodInfo getFolders = typeof(Mega).GetMethod("GetFoldersAsync", BindingFlags.NonPublic | BindingFlags.Instance);
            foreach (JsonNode step in c["steps"]!.AsArray())
            {
                try
                {
                    MegaFolderInfo parent = step!["parent"] == null ? Mega.RootFolder
                        : new MegaFolderInfo { ID = (string)step["parent"]["id"], Name = (string)step["parent"]["name"] };
                    Task task = (Task)getFolders.Invoke(mega, new object[] { parent, CancellationToken.None });
                    task.GetAwaiter().GetResult();
                    var folders = (IReadOnlyList<MegaFolderInfo>)task.GetType().GetProperty("Result").GetValue(task);
                    JsonArray a = new JsonArray();
                    foreach (MegaFolderInfo f in folders) a.Add(new JsonObject { ["id"] = f.ID, ["name"] = f.Name });
                    steps.Add(a);
                }
                catch (Exception e)
                {
                    steps.Add(Error(e));
                }
            }
            return;
        }
        throw new Exception("unknown listing service " + service);
    }
}
