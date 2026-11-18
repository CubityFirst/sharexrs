// Runs SKPath.Op on the cases in a text file and writes the inputs and the
// results with their float bits, for sharex-skia's path ops tests.
//
// Case file: one case per line, `name | op | pathA | pathB`, '#' comments.
// op: union, intersect, difference, xor, reversedifference.
// path: tokens; `winding`/`evenodd` set the fill type; M x y, L x y,
// Q x1 y1 x y, K x1 y1 x y w (conic), C x1 y1 x2 y2 x y, Z,
// A rx ry rotation large(0/1) sweep(0/1, 1 = clockwise) x y (SKPath.ArcTo),
// oval l t r b, ovalccw l t r b, rect l t r b, circle cx cy r.
using System.Globalization;
using System.Reflection;
using System.Text;
using SkiaSharp;

if (args.Length < 2)
{
    Console.Error.WriteLine("usage: PathOpsHarness <cases.txt> <out.txt>");
    return 2;
}

var inv = CultureInfo.InvariantCulture;
var sb = new StringBuilder();
var skiaPath = Path.Combine(AppContext.BaseDirectory, "libSkiaSharp.dll");
Console.WriteLine($"SkiaSharp {typeof(SKPath).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion}");
Console.WriteLine($"libSkiaSharp {System.Diagnostics.FileVersionInfo.GetVersionInfo(skiaPath).FileVersion} ({new FileInfo(skiaPath).Length} bytes)");

int count = 0, failed = 0;
foreach (var raw in File.ReadAllLines(args[0]))
{
    var line = raw.Trim();
    if (line.Length == 0 || line.StartsWith('#')) continue;
    var parts = line.Split('|');
    var name = parts[0].Trim();
    var opName = parts[1].Trim();
    var op = opName switch
    {
        "union" => SKPathOp.Union,
        "intersect" => SKPathOp.Intersect,
        "difference" => SKPathOp.Difference,
        "xor" => SKPathOp.Xor,
        "reversedifference" => SKPathOp.ReverseDifference,
        _ => throw new Exception("op " + opName),
    };
    using var a = Parse(parts[2]);
    using var b = Parse(parts[3]);
    sb.Append("case ").Append(name).Append(' ').Append(opName).Append('\n');
    sb.Append("A ").Append(Dump(a)).Append('\n');
    sb.Append("B ").Append(Dump(b)).Append('\n');
    using var r = a.Op(b, op);
    if (r == null) { sb.Append("R FAIL\n"); failed++; }
    else sb.Append("R ").Append(Dump(r)).Append('\n');
    count++;
}
File.WriteAllText(args[1], sb.ToString());
Console.WriteLine($"{count} cases, {failed} failed ops");
return 0;

SKPath Parse(string text)
{
    var p = new SKPath();
    var t = text.Split(' ', StringSplitOptions.RemoveEmptyEntries);
    int i = 0;
    float F() => float.Parse(t[i++], inv);
    while (i < t.Length)
    {
        var cmd = t[i++];
        switch (cmd)
        {
            case "winding": p.FillType = SKPathFillType.Winding; break;
            case "evenodd": p.FillType = SKPathFillType.EvenOdd; break;
            case "M": p.MoveTo(F(), F()); break;
            case "L": p.LineTo(F(), F()); break;
            case "Q": p.QuadTo(F(), F(), F(), F()); break;
            case "K": p.ConicTo(F(), F(), F(), F(), F()); break;
            case "C": p.CubicTo(F(), F(), F(), F(), F(), F()); break;
            case "Z": p.Close(); break;
            case "A":
            {
                float rx = F(), ry = F(), rot = F();
                var large = t[i++] == "1" ? SKPathArcSize.Large : SKPathArcSize.Small;
                var sweep = t[i++] == "1" ? SKPathDirection.Clockwise : SKPathDirection.CounterClockwise;
                p.ArcTo(rx, ry, rot, large, sweep, F(), F());
                break;
            }
            case "oval": p.AddOval(new SKRect(F(), F(), F(), F()), SKPathDirection.Clockwise); break;
            case "ovalccw": p.AddOval(new SKRect(F(), F(), F(), F()), SKPathDirection.CounterClockwise); break;
            case "rect": p.AddRect(new SKRect(F(), F(), F(), F())); break;
            case "circle": p.AddCircle(F(), F(), F()); break;
            default: throw new Exception("token " + cmd);
        }
    }
    return p;
}

static string Bits(float f) => BitConverter.SingleToInt32Bits(f).ToString("x8");

static string Dump(SKPath p)
{
    var s = new StringBuilder();
    s.Append(p.FillType == SKPathFillType.EvenOdd ? "evenodd" : p.FillType.ToString().ToLowerInvariant());
    using var it = p.CreateRawIterator();
    var pts = new SKPoint[4];
    while (true)
    {
        var verb = it.Next(pts);
        void P(int k) { s.Append(' ').Append(Bits(pts[k].X)).Append(' ').Append(Bits(pts[k].Y)); }
        switch (verb)
        {
            case SKPathVerb.Move: s.Append(" M"); P(0); break;
            case SKPathVerb.Line: s.Append(" L"); P(1); break;
            case SKPathVerb.Quad: s.Append(" Q"); P(1); P(2); break;
            case SKPathVerb.Conic: s.Append(" K"); P(1); P(2); s.Append(' ').Append(Bits(it.ConicWeight())); break;
            case SKPathVerb.Cubic: s.Append(" C"); P(1); P(2); P(3); break;
            case SKPathVerb.Close: s.Append(" Z"); break;
            case SKPathVerb.Done: return s.ToString();
        }
    }
}
