// The harness's synthetic input, generated exactly as
// crates/sharex-native-recorder/examples/synthetic/mod.rs generates it
// (integer maths, one float division per sample), so both sides feed
// identical data.

using Newtonsoft.Json.Linq;

static class Synthetic
{
    public const long TPS = 10_000_000;

    public static byte[] Frame(string pattern, long n, long x0, long y0, long w, long h)
    {
        byte[] o = new byte[w * h * 4];
        long s = Math.Max(Math.Min(w, h) / 4, 2);
        long bx = (n * 5) % Math.Max(w - s, 1);
        long by = (n * 3) % Math.Max(h - s, 1);
        for (long y = 0; y < h; y++)
            for (long x = 0; x < w; x++)
            {
                long i = (y * w + x) * 4;
                byte b, g, r;
                if (pattern == "scroll") { b = (byte)((x + n * 4) & 255); g = (byte)((y * 2 + n) & 255); r = (byte)((x ^ y) & 255); }
                else if (pattern == "noise")
                {
                    uint v = unchecked((uint)x * 73_856_093u ^ (uint)y * 19_349_663u ^ (uint)n * 83_492_791u);
                    v ^= v << 13; v ^= v >> 17; v ^= v << 5;
                    b = (byte)v; g = (byte)(v >> 8); r = (byte)(v >> 16);
                }
                else if (x >= bx && x < bx + s && y >= by && y < by + s) { b = 40; g = 180; r = 250; }
                else if (y >= h - 4 && x * 64 / w == n % 64) { b = g = r = 255; }
                else { b = (byte)((x + x0) & 255); g = (byte)((y + y0) & 255); r = (byte)(((x / 8 + y / 8) % 2) * 60 + 40); }
                o[i] = b; o[i + 1] = g; o[i + 2] = r; o[i + 3] = 255;
            }
        return o;
    }

    public record Slot(long N, long Number, bool Discontinuity);

    static long[] Ints(JToken t) => t?.Type == JTokenType.Array ? t.Select(v => (long)v).ToArray() : Array.Empty<long>();

    public static List<Slot> Slots(JObject c)
    {
        long frames = (long)c["frames"];
        long[] drops = Ints(c["drop"]);
        var still = c["still"]?.Select(r => ((long)r[0], (long)r[1])).ToList() ?? new();
        var o = new List<Slot>();
        long slot = 0, previous = -1;
        for (long k = 0; k < frames; k++)
        {
            while (drops.Contains(slot)) slot++;
            long n = k;
            foreach (var (a, b) in still) if (k >= a && k < b) { n = a - 1; break; }
            o.Add(new(n, slot, previous >= 0 && slot > previous + 1));
            previous = slot;
            slot++;
        }
        return o;
    }

    public static byte[] CameraFrame(string format, int w, int h, long m)
    {
        int shift = (int)(m * 3);
        if (format == "RGB32")
        {
            byte[] o = new byte[w * h * 4];
            for (int y = 0; y < h; y++)
                for (int x = 0; x < w; x++)
                {
                    int i = (y * w + x) * 4;
                    o[i] = (byte)((x + shift) * 255 / w);
                    o[i + 1] = (byte)(y * 255 / h);
                    o[i + 2] = (byte)(((x ^ y) + shift) & 0xFF);
                    o[i + 3] = 255;
                }
            return o;
        }
        if (format == "YUY2")
        {
            byte[] o = new byte[w * h * 2];
            for (int y = 0; y < h; y++)
                for (int x = 0; x < w; x += 2)
                {
                    int i = (y * w + x) * 2;
                    o[i] = (byte)(16 + (x + shift) * 219 / w);
                    o[i + 1] = (byte)(64 + y * 128 / h);
                    o[i + 2] = (byte)Math.Min(16 + (x + 1 + shift) * 219 / w, 235);
                    o[i + 3] = (byte)(192 - ((x + shift) * 128 / w) % 128);
                }
            return o;
        }
        byte[] nv = new byte[w * h * 3 / 2];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
                nv[y * w + x] = (byte)(16 + ((x + y + shift) % 220));
        for (int y = 0; y < h / 2; y++)
            for (int x = 0; x < w; x += 2)
            {
                int i = w * h + y * w + x;
                nv[i] = (byte)(64 + x * 128 / w);
                nv[i + 1] = (byte)(192 - y * 2 * 128 / h);
            }
        return nv;
    }

    public static (float, float) Sample(int source, long f)
    {
        if (source == 0) return ((float)((f * 37) % 2000 - 1000) / 2500f, (float)((f * 53) % 1500 - 750) / 2000f);
        float v = (float)((f * 11) % 960 - 480) / 1200f;
        return (v, v);
    }

    public record Packet(int Source, long Timestamp, int Frames, float[] Data, float Gain, bool Discontinuity, long End);

    public static List<Packet> MixerPackets(JObject c)
    {
        long packets = (long)c["packets"], baseFrames = (long)c["packet_frames"];
        bool vary = (bool?)c["vary"] ?? false;
        long[] jitter = Ints(c["jitter"]), gaps = Ints(c["gap_after"]), disc = Ints(c["discontinuity"]), silent = Ints(c["silent"]), nan = Ints(c["nan"]);
        long drift = (long?)c["drift_ppm"] ?? 0, gapFrames = (long?)c["gap_frames"] ?? 0;
        int[] sources = c["sources"].Select(s => (string)s == "system" ? 0 : 1).ToArray();
        float[] gains = c["gains"]?.Select(g => (float)(double)g).ToArray() ?? Enumerable.Repeat(1f, sources.Length).ToArray();
        var o = new List<Packet>();
        long f = 0;
        for (long i = 0; i < packets; i++)
        {
            long frames = vary ? baseFrames + (i % 3) * 7 - 7 : baseFrames;
            long ts = f * TPS / 48_000;
            if (drift != 0) ts = ts * 1_000_000 / (1_000_000 + drift);
            if (jitter.Length > 0) ts += jitter[i % jitter.Length] * TPS / 48_000;
            long end = ts + frames * TPS / 48_000;
            for (int si = 0; si < sources.Length; si++)
            {
                float[] data;
                if (silent.Contains(i)) data = Array.Empty<float>();
                else if (nan.Contains(i)) data = Enumerable.Repeat(float.NaN, (int)frames * 2).ToArray();
                else
                {
                    data = new float[frames * 2];
                    for (long j = 0; j < frames; j++) { var (l, r) = Sample(sources[si], f + j); data[j * 2] = l; data[j * 2 + 1] = r; }
                }
                o.Add(new(sources[si], Math.Max(ts, 0), (int)frames, data, gains[si], disc.Contains(i), end));
            }
            f += frames;
            if (gaps.Contains(i)) f += gapFrames;
        }
        return o;
    }

    public static List<Packet> Mp4Packets(JToken audio, long until)
    {
        var sources = new List<(int, float)>();
        if ((bool?)audio["system"] ?? false) sources.Add((0, (float)((double?)audio["system_gain"] ?? 1.0)));
        if ((bool?)audio["microphone"] ?? false) sources.Add((1, (float)((double?)audio["microphone_gain"] ?? 1.0)));
        var o = new List<Packet>();
        for (long f = 0; ; f += 480)
        {
            long ts = f * TPS / 48_000;
            if (ts >= until) break;
            foreach (var (source, gain) in sources)
            {
                float[] data = new float[960];
                for (long j = 0; j < 480; j++) { var (l, r) = Sample(source, f + j); data[j * 2] = l; data[j * 2 + 1] = r; }
                o.Add(new(source, ts, 480, data, gain, false, ts + 480 * TPS / 48_000));
            }
        }
        return o;
    }
}
