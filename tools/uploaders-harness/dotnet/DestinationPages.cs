// Dumps what DestinationSettingsPageBuilder builds for each Destination
// settings page, by calling ShareX's own (private) helpers through
// reflection, so ShareXrs draws the same rows with the same labels, editor
// kinds, enum texts and order:
//   UploadersHarness --destination-pages <out.json>
//
// The generic part of BuildPage (the "Settings" card of simple members,
// the nested object cards, the list editors) is described member by
// member; the hand-built cards (accounts, Imgur albums, Amazon S3, img.fish,
// remote folders...) are named by kind, as ShareXrs builds those itself.

using System.Collections;
using System.Reflection;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.UploadersLib;

static class DestinationPages
{
    const BindingFlags Static = BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Static;
    const BindingFlags Instance = BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Instance;

    static readonly Assembly Lib = typeof(UploadersConfig).Assembly;
    static readonly Type Builder = Lib.GetType("ShareX.UploadersLib.DestinationSettingsPageBuilder", true);
    static readonly Type Member = Lib.GetType("ShareX.UploadersLib.DestinationMember", true);
    static readonly Type ViewModel = Lib.GetType("ShareX.UploadersLib.DestinationSettingsViewModel", true);

    static object Call(string name, params object[] args) => Builder.GetMethod(name, Static).Invoke(null, args);

    static object NewMember(MemberInfo m) => m is PropertyInfo p
        ? Activator.CreateInstance(Member, Instance, null, [p], null)
        : Activator.CreateInstance(Member, Instance, null, [(FieldInfo)m], null);

    static string Name(object member) => (string)Member.GetProperty("Name").GetValue(member);
    static Type ValueType(object member) => (Type)Member.GetProperty("ValueType").GetValue(member);

    static readonly System.Resources.ResourceManager Resources =
        (System.Resources.ResourceManager)Lib.GetType("ShareX.UploadersLib.Localization.Strings", true)
            .GetProperty("ResourceManager", Static).GetValue(null);

    /// FormatLabel's resource key for a label, when the resource exists.
    static string LabelKey(string label)
    {
        string key = "DestinationSettings_Field_" + System.Text.RegularExpressions.Regex.Replace(label, "[^A-Za-z0-9]+", "_");
        return Resources.GetString(key) != null ? key : null;
    }

    /// One editor row (EditorRow/CreateEditor), or null when ShareX shows none.
    static JObject Row(object member, string label, string[] path)
    {
        Type type = Nullable.GetUnderlyingType(ValueType(member)) ?? ValueType(member);
        JObject row = new() { ["label"] = label, ["path"] = new JArray(path) };
        if (LabelKey(label) is { } labelKey) row["label_key"] = labelKey;
        if (type == typeof(bool)) row["kind"] = "bool";
        else if (type == typeof(string))
        {
            row["kind"] = "string";
            row["secret"] = (bool)Call("IsSecret", member);
            // `[JsonEncrypt]`: written as `$DPAPIEncrypted$...`.
            if (((IEnumerable<Attribute>)Member.GetMethod("GetAttributes").Invoke(member, null)).Any(x => x.GetType().Name == "JsonEncryptAttribute"))
                row["encrypt"] = true;
        }
        else if (type.IsEnum)
        {
            row["kind"] = "enum";
            JArray values = new();
            foreach (Enum v in Enum.GetValues(type))
            {
                JObject value = new() { ["name"] = v.ToString(), ["text"] = v.GetLocalizedDescription(Resources) };
                string key = type.Name + "_" + v;
                if (Resources.GetString(key) != null) value["key"] = key;
                values.Add(value);
            }
            row["values"] = values;
        }
        else if ((bool)Call("IsStringList", type)) row["kind"] = "stringlist";
        else if ((bool)Call("IsSimpleType", type))
        {
            // Number(): the type's range (others ±1e9); "selection" for the
            // account pickers TryCreateSelectionEditor makes.
            string n = path[^1];
            if (n.StartsWith("FTPSelected") || n.StartsWith("LocalhostSelected") || n == "SelectedDevice") row["kind"] = "selection";
            else row["kind"] = "number";
            row["type"] = type.Name;
        }
        else row["kind"] = "nested";
        string description = (Member.GetMethod("GetAttribute").MakeGenericMethod(typeof(System.ComponentModel.DescriptionAttribute))
            .Invoke(member, null) as System.ComponentModel.DescriptionAttribute)?.Description;
        if (!string.IsNullOrWhiteSpace(description)) row["tooltip"] = description;
        return row;
    }

    static JArray EditableRows(Type type, string[] prefix)
    {
        JArray rows = new();
        foreach (object child in (IEnumerable)Call("GetEditableMembers", type))
        {
            rows.Add(Row(child, (string)Call("FormatLabel", Name(child)), [.. prefix, Name(child)]));
        }
        return rows;
    }

    public static int Run(string outPath)
    {
        UploadersConfig config = new();
        PropertyInfo[] properties = typeof(UploadersConfig).GetProperties(BindingFlags.Instance | BindingFlags.Public);
        JArray categories = new();
        foreach (object category in (IEnumerable)ViewModel.GetProperty("Categories", Static).GetValue(null))
        {
            Type ct = category.GetType();
            JArray pages = new();
            foreach (object page in (IEnumerable)ct.GetProperty("Pages").GetValue(category))
            {
                Type pt = page.GetType();
                string id = (string)pt.GetProperty("Id").GetValue(page);
                string[] prefixes = (string[])pt.GetProperty("Prefixes").GetValue(page);
                JArray cards = new();
                // Accounts, then the page's own cards (BuildPage's order).
                object accounts = Activator.CreateInstance(Lib.GetType("ShareX.UploadersLib.DestinationSettingsAccounts", true), Instance, null, [config], null);
                if (accounts.GetType().GetMethod("Build", Instance).Invoke(accounts, [id]) != null) cards.Add(new JObject { ["special"] = "account" });
                string special = id switch
                {
                    "imgur" => "imgur-albums",
                    "photobucket" => "photobucket-albums",
                    "onedrive" => "onedrive-folder",
                    "box" => "box-folder",
                    "mega" => "mega-folders",
                    "img-fish" => "img-fish",
                    _ => null
                };
                if (special != null) cards.Add(new JObject { ["special"] = special });
                if (id == "amazon-s3") cards.Add(new JObject { ["special"] = "amazon-s3" });
                JArray simple = new();
                foreach (PropertyInfo property in properties.Where(p => prefixes.Any(x => p.Name.StartsWith(x, StringComparison.OrdinalIgnoreCase))))
                {
                    if (!(bool)Call("IsEditableConfigProperty", property)) continue;
                    object member = NewMember(property);
                    if ((bool)Call("ShouldSkipMember", id, member)) continue;
                    object value = property.GetValue(config);
                    Type type = Nullable.GetUnderlyingType(property.PropertyType) ?? property.PropertyType;
                    string label = (string)Call("GetTopLevelLabel", property.Name, (IEnumerable<string>)prefixes);
                    if ((bool)Call("IsSimpleType", type) || (bool)Call("IsStringList", type))
                    {
                        simple.Add(Row(member, label, [property.Name]));
                    }
                    else if (value is IList)
                    {
                        Type itemType = property.PropertyType.IsGenericType ? property.PropertyType.GetGenericArguments()[0] : typeof(object);
                        JObject list = new() { ["title"] = label, ["list"] = property.Name, ["item_type"] = itemType.Name };
                        if (itemType == typeof(string)) list["strings"] = true;
                        else
                        {
                            list["rows"] = EditableRows(itemType, []);
                            // What Add makes (`Activator.CreateInstance`), as the config stores it.
                            list["default_item"] = JObject.Parse(JsonConvert.SerializeObject(Activator.CreateInstance(itemType)));
                        }
                        cards.Add(list);
                    }
                    else if (value != null)
                    {
                        JArray rows = EditableRows(value.GetType(), [property.Name]);
                        if (rows.Count > 0)
                        {
                            string title = label is "Settings" or "Uploader" ? Resources.GetString("DestinationSettings_Settings") : label;
                            cards.Add(new JObject { ["title"] = title, ["rows"] = rows });
                        }
                    }
                }
                if (simple.Count > 0) cards.Add(new JObject { ["title"] = Resources.GetString("DestinationSettings_Settings"), ["rows"] = simple });
                if (cards.Count == 0)
                {
                    cards.Add(new JObject { ["title"] = Resources.GetString("DestinationSettings_Settings"), ["hint"] = Resources.GetString("DestinationSettings_No_additional_configuration_required") });
                }
                if (id is "ftp" or "amazon-s3") cards.Add(new JObject { ["special"] = "remote-storage-browser" });
                pages.Add(new JObject
                {
                    ["id"] = id,
                    ["title"] = (string)pt.GetProperty("Title").GetValue(page),
                    ["prefixes"] = new JArray(prefixes),
                    ["aliases"] = new JArray((string[])pt.GetProperty("Aliases").GetValue(page)),
                    ["cards"] = cards,
                });
            }
            categories.Add(new JObject
            {
                ["id"] = (string)ct.GetProperty("Id").GetValue(category),
                ["title"] = (string)ct.GetProperty("Title").GetValue(category),
                ["icon"] = (string)ct.GetProperty("Icon").GetValue(category),
                ["pages"] = pages,
            });
        }
        File.WriteAllText(outPath, categories.ToString(Formatting.Indented));
        return 0;
    }
}
