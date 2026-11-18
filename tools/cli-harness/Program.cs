// The ShareX side of tools/cli-harness: ShareX's own CLIManager, the
// BinaryWriter message SingleInstanceManager sends, and the native messaging
// input JSON as TaskHelpers.HandleNativeMessagingInput reads it, over the
// cases ShareXrs's tests replay.
//
//   CliHarness --cli <cases.json> <results.json>
//   CliHarness --native <cases.json> <scratch> <results.json>
//
// No window opens; the native cases only write files in <scratch>.

using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX;
using ShareX.HelpersLib;

static class Program
{
    static int Main(string[] args)
    {
        if (args.Length == 3 && args[0] == "--cli")
        {
            return Cli(args[1], args[2]);
        }
        if (args.Length == 4 && args[0] == "--native")
        {
            return Native(args[1], args[2], args[3]);
        }
        Console.Error.WriteLine("usage: CliHarness --cli <cases> <out> | --native <cases> <scratch> <out>");
        return 2;
    }

    static void Write(string path, JsonNode node)
    {
        File.WriteAllText(path, node.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
    }

    static string[] Strings(JsonNode n) => n.AsArray().Select(x => x.GetValue<string>()).ToArray();

    static int Cli(string casesPath, string outPath)
    {
        JsonNode cases = JsonNode.Parse(File.ReadAllText(casesPath));
        string[][] queries = cases["queries"].AsArray().Select(Strings).ToArray();
        string[] parameters = Strings(cases["parameters"]);
        JsonObject results = new();

        JsonArray arrays = new();
        foreach (JsonNode c in cases["arrays"].AsArray())
        {
            string[] arguments = Strings(c);
            CLIManager cli = new(arguments);
            bool parsed = cli.ParseCommands();
            JsonArray commands = new();
            foreach (CLICommand command in cli.Commands)
            {
                commands.Add(new JsonObject { ["command"] = command.Command, ["parameter"] = command.Parameter, ["is_command"] = command.IsCommand });
            }
            JsonArray exists = new();
            foreach (string[] q in queries)
            {
                exists.Add(cli.IsCommandExist(q));
            }
            JsonArray values = new();
            foreach (string p in parameters)
            {
                values.Add(cli.GetParameter(p));
            }
            // SingleInstanceManager.RedirectArgumentsToFirstInstance's message.
            MemoryStream ms = new();
            using (BinaryWriter writer = new(ms, Encoding.UTF8, true))
            {
                writer.Write(arguments.Length);
                foreach (string argument in arguments)
                {
                    writer.Write(argument);
                }
            }
            arrays.Add(new JsonObject
            {
                ["parsed"] = parsed,
                ["commands"] = commands,
                ["exists"] = exists,
                ["parameters"] = values,
                ["text"] = cli.ToString(),
                ["message"] = Convert.ToHexString(ms.ToArray()),
            });
        }
        results["arrays"] = arrays;

        JsonArray strings = new();
        foreach (JsonNode c in cases["strings"].AsArray())
        {
            JsonArray split = new();
            foreach (string a in new CLIManager(c.GetValue<string>()).Arguments)
            {
                split.Add(a);
            }
            strings.Add(split);
        }
        results["strings"] = strings;
        Write(outPath, results);
        return 0;
    }

    static int Native(string casesPath, string scratch, string outPath)
    {
        JsonNode cases = JsonNode.Parse(File.ReadAllText(casesPath));
        Directory.CreateDirectory(scratch);
        JsonArray results = new();
        int i = 0;
        foreach (JsonNode c in cases["inputs"].AsArray())
        {
            // As ShareX_NativeMessagingHost writes it (File.WriteAllText with
            // UTF-8: no BOM) and HandleNativeMessagingInput reads it.
            string path = Path.Combine(scratch, $"native-{i++}.json");
            File.WriteAllText(path, c.GetValue<string>(), new UTF8Encoding(false));
            JsonNode result;
            try
            {
                NativeMessagingInput input = JsonHelpers.DeserializeFromFile<NativeMessagingInput>(path);
                result = input == null ? null : new JsonObject
                {
                    ["action"] = input.Action.ToString(),
                    ["url"] = input.URL,
                    ["text"] = input.Text,
                };
            }
            catch (Exception e)
            {
                result = new JsonObject { ["exception"] = e.GetType().Name };
            }
            results.Add(result);
        }
        Write(outPath, new JsonObject { ["results"] = results });
        return 0;
    }
}
