// ShareXrs OCR harness: runs ShareX's own OCR (ShareX.OCR.dll from a ShareX build of
// REFERENCE_COMMIT) over synthetic images and writes what it produces, so
// crates/sharex-ocr can be compared with it (examples/validate.rs).
//
//   OcrHarness generate <cases dir>
//       Draws the test images (Generate.cs) and cases.json.
//   OcrHarness run <cases dir> <models dir> <work dir> [case prefix]
//       For each case, loads the PNG as ShareX does (SkiaImageHelpers.LoadImage) and
//       writes under <work dir>/sharex/<case>/:
//         result.json   PaddleOCREngine.Recognize: regions (corners, text, score),
//                       lines, GetText() three ways; the detection input size,
//                       probability map size, boxes, raw texts and scores. Floats
//                       are also written as their bit patterns ("bits") so the
//                       comparison is exact.
//         resized.bgra  the detection model input image (ImageTensor.Resize)
//         prob.f32      the detection model's probability map (little-endian floats)
//         crop_<i>.bgra each box's upright line image (ImageTensor.Crop)
//
// The intermediates come from ShareX's internal classes through reflection
// (TextDetector, TextRecognizer, ImageTensor); the final result is the public API.

using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.ML.OnnxRuntime;
using Microsoft.ML.OnnxRuntime.Tensors;
using ShareX.HelpersLib;
using ShareX.OCR;
using SkiaSharp;

static class Program
{
    const BindingFlags Any = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static;

    delegate void FillDelegate(Span<float> tensor, int batchIndex, int tensorWidth, int tensorHeight, SKBitmap bitmap, float[] mean, float[] std);

    static int Main(string[] args)
    {
        if (args.Length >= 2 && args[0] == "generate")
        {
            Generate.Run(args[1]);
            return 0;
        }
        if (args.Length >= 4 && args[0] == "run")
        {
            return RunAll(args[1], args[2], args[3], args.Length > 4 ? args[4] : null);
        }
        Console.Error.WriteLine("usage: OcrHarness generate <cases dir> | run <cases dir> <models dir> <work dir> [case prefix]");
        return 2;
    }

    static int RunAll(string casesDir, string modelsDir, string work, string only)
    {
        string outRoot = Path.Combine(work, "sharex");
        Directory.CreateDirectory(outRoot);
        JsonArray cases = JsonNode.Parse(File.ReadAllText(Path.Combine(casesDir, "cases.json"))).AsArray();
        using PaddleOCREngine engine = new(modelsDir);
        Console.WriteLine($"models: {engine.DetectionModelFolder} | {engine.RecognitionModelFolder}");
        int failures = 0;
        foreach (JsonNode c in cases)
        {
            string name = (string)c["name"];
            if (only != null && !name.StartsWith(only, StringComparison.Ordinal)) continue;
            string dir = Path.Combine(outRoot, name);
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
            Directory.CreateDirectory(dir);
            try
            {
                RunCase(engine, Path.Combine(casesDir, (string)c["file"]), dir);
                Console.WriteLine($"{name}: ok");
            }
            catch (Exception e)
            {
                failures++;
                File.WriteAllText(Path.Combine(dir, "error.txt"), e.ToString());
                Console.Error.WriteLine($"{name}: {e.GetBaseException().Message}");
            }
        }
        return failures == 0 ? 0 : 1;
    }

    static object Field(object o, string name) => o.GetType().GetField(name, Any).GetValue(o);

    static JsonArray FloatBits(IEnumerable<float> values) => new(values.Select(v => (JsonNode)BitConverter.SingleToUInt32Bits(v)).ToArray());

    static JsonArray Points(SKPoint[] points) => new(points.Select(p => (JsonNode)new JsonArray(p.X, p.Y)).ToArray());

    static JsonArray PointBits(SKPoint[] points) => FloatBits(points.SelectMany(p => new[] { p.X, p.Y }));

    static JsonObject Rect(SKRect r) => new() { ["left"] = r.Left, ["top"] = r.Top, ["right"] = r.Right, ["bottom"] = r.Bottom, ["bits"] = FloatBits([r.Left, r.Top, r.Right, r.Bottom]) };

    static void WriteBgra(string path, SKBitmap bitmap)
    {
        // Rows without padding.
        using FileStream f = File.Create(path);
        ReadOnlySpan<byte> pixels = bitmap.GetPixelSpan();
        for (int y = 0; y < bitmap.Height; y++) f.Write(pixels.Slice(y * bitmap.RowBytes, bitmap.Width * 4));
    }

    static void RunCase(PaddleOCREngine engine, string imagePath, string dir)
    {
        using SKBitmap bitmap = SkiaImageHelpers.LoadImage(imagePath) ?? throw new InvalidDataException("couldn't load " + imagePath);
        JsonObject json = new() { ["width"] = bitmap.Width, ["height"] = bitmap.Height, ["colorType"] = bitmap.ColorType.ToString(), ["alphaType"] = bitmap.AlphaType.ToString() };

        // The public result.
        OCRResult result = engine.Recognize(bitmap);
        json["regions"] = new JsonArray(result.Regions.Select(r => (JsonNode)new JsonObject
        {
            ["text"] = r.Text,
            ["score"] = r.Score,
            ["scoreBits"] = BitConverter.SingleToUInt32Bits(r.Score),
            ["points"] = Points(r.Points),
            ["pointBits"] = PointBits(r.Points),
            ["bounds"] = Rect(r.Bounds),
        }).ToArray());
        json["lines"] = new JsonArray(result.Lines.Select(l => (JsonNode)new JsonObject
        {
            ["text"] = l.Text,
            ["bounds"] = Rect(l.Bounds),
            ["height"] = l.Height,
            ["heightBits"] = BitConverter.SingleToUInt32Bits(l.Height),
            ["startsParagraph"] = l.StartsParagraph,
        }).ToArray());
        json["text"] = result.GetText();
        json["textSingleLine"] = result.GetText(singleLine: true);
        json["textNoParagraphs"] = result.GetText(separateParagraphs: false);

        // The intermediates, as PaddleOCREngine.Recognize computes them.
        Assembly ocr = typeof(PaddleOCREngine).Assembly;
        Type imageTensor = ocr.GetType("ShareX.OCR.ImageTensor", true);
        object detector = Field(engine, "_detector");
        object recognizer = Field(engine, "_recognizer");
        Type detectorType = detector.GetType(), recognizerType = recognizer.GetType();

        using SKBitmap source = (SKBitmap)imageTensor.GetMethod("ToOpaqueBgra", Any).Invoke(null, [bitmap]);
        using SKImage image = SKImage.FromBitmap(source);

        // Detection input and probability map.
        var size = ((int Width, int Height))detectorType.GetMethod("GetInputSize", Any).Invoke(null, [image.Width, image.Height]);
        json["detInputWidth"] = size.Width;
        json["detInputHeight"] = size.Height;
        float[] mean = (float[])Field(detector, "_mean"), std = (float[])Field(detector, "_std");
        DenseTensor<float> input = new([1, 3, size.Height, size.Width]);
        using (SKBitmap resized = (SKBitmap)imageTensor.GetMethod("Resize", Any).Invoke(null, [image, size.Width, size.Height]))
        {
            WriteBgra(Path.Combine(dir, "resized.bgra"), resized);
            FillDelegate fill = imageTensor.GetMethod("Fill", Any).CreateDelegate<FillDelegate>();
            fill(input.Buffer.Span, 0, size.Width, size.Height, resized, mean, std);
        }
        InferenceSession detSession = (InferenceSession)Field(detector, "_session");
        string detInput = (string)Field(detector, "_inputName");
        using (IDisposableReadOnlyCollection<DisposableNamedOnnxValue> outputs = detSession.Run([NamedOnnxValue.CreateFromTensor(detInput, input)]))
        {
            Tensor<float> output = outputs.First().AsTensor<float>();
            ReadOnlySpan<int> dims = output.Dimensions;
            json["mapWidth"] = dims[^1];
            json["mapHeight"] = dims[^2];
            float[] probabilities = output.ToArray();
            using FileStream f = File.Create(Path.Combine(dir, "prob.f32"));
            f.Write(System.Runtime.InteropServices.MemoryMarshal.AsBytes(probabilities.AsSpan()));
        }

        // Boxes, raw texts and scores.
        var boxes = (List<SKPoint[]>)detectorType.GetMethod("Detect", Any).Invoke(detector, [image]);
        json["boxes"] = new JsonArray(boxes.Select(b => (JsonNode)new JsonObject { ["points"] = Points(b), ["bits"] = PointBits(b) }).ToArray());
        if (boxes.Count > 0)
        {
            var texts = ((string Text, float Score)[])recognizerType.GetMethod("Recognize", Any).Invoke(recognizer, [image, boxes]);
            json["texts"] = new JsonArray(texts.Select(t => (JsonNode)new JsonObject { ["text"] = t.Text, ["score"] = t.Score, ["scoreBits"] = BitConverter.SingleToUInt32Bits(t.Score) }).ToArray());

            // The line crops, as RecognizeBatch draws them.
            int inputHeight = (int)Field(recognizer, "_inputHeight");
            int minInputWidth = (int)Field(recognizer, "_minInputWidth");
            MethodInfo getLine = recognizerType.GetMethod("GetLineImage", Any);
            object[] lines = boxes.Select(b => getLine.Invoke(null, [b])).ToArray();
            Type lineType = getLine.ReturnType;
            T Prop<T>(object line, string name) => (T)lineType.GetProperty(name, Any).GetValue(line);
            int[] order = [.. Enumerable.Range(0, lines.Length).OrderBy(i => Prop<float>(lines[i], "AspectRatio"))];
            JsonArray crops = new(new JsonNode[boxes.Count]);
            for (int start = 0; start < order.Length; start += 6)
            {
                int[] batch = order[start..Math.Min(start + 6, order.Length)];
                float maxAspect = Math.Max((float)minInputWidth / inputHeight, batch.Max(i => Prop<float>(lines[i], "AspectRatio")));
                int inputWidth = Math.Min((int)(inputHeight * maxAspect), 3200);
                for (int k = 0; k < batch.Length; k++)
                {
                    object line = lines[batch[k]];
                    float aspect = Prop<float>(line, "AspectRatio");
                    int width = Math.Clamp((int)Math.Ceiling(inputHeight * aspect), 1, inputWidth);
                    using SKBitmap crop = (SKBitmap)imageTensor.GetMethod("Crop", Any).Invoke(null,
                        [image, Prop<SKPoint>(line, "Origin"), Prop<SKPoint>(line, "XEdge"), Prop<SKPoint>(line, "YEdge"), width, inputHeight]);
                    WriteBgra(Path.Combine(dir, $"crop_{batch[k]}.bgra"), crop);
                    crops[batch[k]] = new JsonObject { ["batch"] = start + k, ["inputWidth"] = inputWidth, ["width"] = width, ["height"] = inputHeight, ["aspectBits"] = BitConverter.SingleToUInt32Bits(aspect) };
                }
            }
            json["crops"] = crops;
        }

        File.WriteAllText(Path.Combine(dir, "result.json"), json.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }));
    }
}
