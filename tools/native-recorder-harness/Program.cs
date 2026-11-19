// Drives ShareX's own native recorder code (ShareX.ScreenRecordingLib.dll)
// with the synthetic frames, camera frames and sound of cases.json, in the
// order ScreenRecorder.Run uses them, and writes what it produces. Nothing
// is captured: frames go straight into GpuVideoProcessor's canvas (where
// Windows Graphics Capture would copy them), camera frames are Media
// Foundation memory samples (what CameraCapture's source reader delivers),
// and sound goes straight into AudioMixer (where WasapiCapture.Drain writes).
// The classes are internal, so they're reached by reflection.
//
//   NativeRecorderHarness <cases.json> <out dir>

using System.Reflection;
using System.Runtime.InteropServices;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using ShareX.ScreenRecordingLib;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.MediaFoundation;

delegate void WriteAudioFn(ReadOnlySpan<short> pcm, long frameOffset);
delegate void MixerWriteFn(int source, long timestamp, int frames, ReadOnlySpan<float> packet, float gain, bool discontinuity);
delegate void MixerReadFn(long firstFrame, Span<short> destination);

static unsafe class Program
{
    const BindingFlags All = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly Assembly Lib = typeof(RecordingOptions).Assembly;

    static Type T(string name) => Lib.GetType("ShareX.ScreenRecordingLib." + name, true);
    static object New(string type, params object[] args) =>
        Activator.CreateInstance(T(type), All, null, args, null);
    static MethodInfo M(Type t, string name, int args) =>
        t.GetMethods(All).Single(m => m.Name == name && m.GetParameters().Length == args);
    static object Call(object o, string name, params object[] args) => M(o.GetType(), name, args.Length).Invoke(o, args);
    static object Static(string type, string name, params object[] args) => M(T(type), name, args.Length).Invoke(null, args);
    static object Get(object o, string name) =>
        (object)o.GetType().GetProperty(name, All)?.GetValue(o) ?? o.GetType().GetField(name, All).GetValue(o);
    static TDelegate Bind<TDelegate>(object o, string name) where TDelegate : Delegate =>
        (TDelegate)Delegate.CreateDelegate(typeof(TDelegate), o, M(o.GetType(), name, typeof(TDelegate).GetMethod("Invoke").GetParameters().Length));

    static int Main(string[] args)
    {
        if (args.Length != 2) { Console.Error.WriteLine("usage: NativeRecorderHarness <cases.json> <out dir>"); return 2; }
        JObject cases = JObject.Parse(File.ReadAllText(args[0]));
        string outDir = args[1];
        Directory.CreateDirectory(outDir);
        string only = Environment.GetEnvironmentVariable("CASE");
        MediaFactory.MFStartup(true).CheckError();
        foreach (JObject c in cases["mixer"])
        {
            string name = (string)c["name"];
            if (only != null && name != only) continue;
            File.WriteAllBytes(Path.Combine(outDir, name + ".pcm"), MixerCase(c));
            Console.WriteLine($"{name}: ok");
        }
        foreach (JObject c in cases["drain"] ?? new JArray())
        {
            string name = (string)c["name"];
            if (only != null && name != only) continue;
            object mixer = New("Audio.AudioMixer", Clock());
            var read = Bind<MixerReadFn>(mixer, "Read");
            var pcm = new MemoryStream();
            short[] block = new short[960];
            long first = 0;
            void Out(short[] b, int n, long _) { foreach (short v in b.AsSpan(0, n)) pcm.Write(BitConverter.GetBytes(v)); }
            int discontinuities = Hardware.Drain(c, mixer, (until, partial) => WriteAudioUntil(read, block, ref first, until, partial, Out));
            File.WriteAllBytes(Path.Combine(outDir, name + ".pcm"), pcm.ToArray());
            File.WriteAllText(Path.Combine(outDir, name + ".json"), new JObject { ["discontinuities"] = discontinuities }.ToString(Formatting.Indented));
            Console.WriteLine($"{name}: ok");
        }
        foreach (JObject c in cases["hdr"] ?? new JArray())
        {
            string name = (string)c["name"];
            if (only != null && name != only) continue;
            File.WriteAllBytes(Path.Combine(outDir, name + ".bgra"), Hardware.Hdr(c));
            Console.WriteLine($"{name}: ok");
        }
        if (only == null || only == "devices")
        {
            File.WriteAllText(Path.Combine(outDir, "devices.json"), Hardware.Devices().ToString(Formatting.Indented));
            Console.WriteLine("devices: ok");
        }
        foreach (string kind in new[] { "gif", "mp4" })
            foreach (JObject c in cases[kind])
            {
                string name = (string)c["name"];
                if (only != null && name != only) continue;
                JObject info;
                try { info = Encode(kind, c, outDir); Console.WriteLine($"{name}: {info["encoder"]}"); }
                catch (Exception e)
                {
                    Exception inner = e is TargetInvocationException t ? t.InnerException : e;
                    info = new JObject { ["error"] = inner.Message };
                    Console.WriteLine($"{name}: ERROR {inner}");
                }
                File.WriteAllText(Path.Combine(outDir, name + ".json"), info.ToString(Formatting.Indented));
            }
        MediaFactory.MFShutdown();
        return 0;
    }

    static object Clock()
    {
        object clock = New("RecordingClock");
        Call(clock, "Start", 0L);
        return clock;
    }

    /// ScreenRecorder.WriteAudioUntil, with the writer's call replaced by `write`.
    static void WriteAudioUntil(MixerReadFn read, short[] block, ref long firstFrame, long until, bool partial, Action<short[], int, long> write)
    {
        long endFrame = (long)((Int128)until * 48_000 / TimeSpan.TicksPerSecond);
        while (firstFrame + block.Length / 2 <= endFrame || (partial && firstFrame < endFrame))
        {
            int count = (int)Math.Min(block.Length / 2, endFrame - firstFrame);
            read(firstFrame, block.AsSpan(0, count * 2));
            write(block, count * 2, firstFrame);
            firstFrame += count;
        }
    }

    static byte[] MixerCase(JObject c)
    {
        object mixer = New("Audio.AudioMixer", Clock());
        var write = Bind<MixerWriteFn>(mixer, "Write");
        var read = Bind<MixerReadFn>(mixer, "Read");
        var pcm = new MemoryStream();
        short[] block = new short[960];
        long first = 0, lastEnd = 0;
        void Out(short[] b, int n, long _) { foreach (short v in b.AsSpan(0, n)) pcm.Write(BitConverter.GetBytes(v)); }
        foreach (var p in Synthetic.MixerPackets(c))
        {
            write(p.Source, p.Timestamp, p.Frames, p.Data, p.Gain, p.Discontinuity);
            lastEnd = p.End;
            WriteAudioUntil(read, block, ref first, Math.Max(0, p.End - 50 * 10_000), false, Out);
        }
        WriteAudioUntil(read, block, ref first, lastEnd, true, Out);
        return pcm.ToArray();
    }

    static RecordingOptions Options(string kind, JObject c, string path)
    {
        JToken audio = c["audio"] ?? new JObject(), camera = c["camera"];
        var o = new RecordingOptions
        {
            OutputPath = path,
            Region = new System.Drawing.Rectangle((int?)c["x"] ?? 0, (int?)c["y"] ?? 0, (int)c["width"], (int)c["height"]),
            FramesPerSecond = (int)c["fps"],
            VideoBitrate = (int?)c["bitrate"] ?? 8_000_000,
            AudioBitrate = (int?)audio["bitrate"] ?? 192_000,
            CaptureSystemAudio = kind == "mp4" && ((bool?)audio["system"] ?? false),
            CaptureMicrophone = kind == "mp4" && ((bool?)audio["microphone"] ?? false),
            SystemAudioGain = (float)((double?)audio["system_gain"] ?? 1.0),
            MicrophoneGain = (float)((double?)audio["microphone_gain"] ?? 1.0),
        };
        if (camera != null)
            o = o with
            {
                CaptureCamera = true,
                CameraShape = (string)camera["shape"] == "Circle" ? CameraOverlayShape.Circle : CameraOverlayShape.Rectangle,
                CameraPosition = Enum.Parse<CameraOverlayPosition>((string)camera["position"]),
                CameraWidthPercent = (int)camera["percent"],
                CameraMargin = (int)camera["margin"],
                CameraFramesPerSecond = (int)camera["fps"],
            };
        return o;
    }

    static ID3D11Texture2D Staging(ID3D11Device device, int w, int h, Format format) => device.CreateTexture2D(new Texture2DDescription
    {
        Width = (uint)w, Height = (uint)h, MipLevels = 1, ArraySize = 1, Format = format, SampleDescription = new(1, 0),
        Usage = ResourceUsage.Staging, CPUAccessFlags = CpuAccessFlags.Read
    });

    static byte[] Read(ID3D11Device device, ID3D11DeviceContext context, ID3D11Texture2D texture, int w, int h, bool nv12)
    {
        using ID3D11Texture2D staging = Staging(device, w, h, nv12 ? Format.NV12 : Format.B8G8R8A8_UNorm);
        context.CopyResource(staging, texture);
        context.Map(staging, 0, MapMode.Read, Vortice.Direct3D11.MapFlags.None, out MappedSubresource mapped).CheckError();
        int row = nv12 ? w : w * 4, rows = nv12 ? h + h / 2 : h;
        byte[] o = new byte[row * rows];
        for (int y = 0; y < rows; y++) Marshal.Copy(mapped.DataPointer + y * (nint)mapped.RowPitch, o, y * row, row);
        context.Unmap(staging, 0);
        return o;
    }

    static JObject Encode(string kind, JObject c, string outDir)
    {
        string name = (string)c["name"];
        string path = Path.Combine(outDir, $"{name}.{kind}");
        File.Delete(path);
        RecordingOptions o = Options(kind, c, path);
        bool gif = kind == "gif";
        bool hasAudio = !gif && (o.CaptureSystemAudio || o.CaptureMicrophone);
        int w = gif ? o.Region.Width : o.Region.Width & ~1, h = gif ? o.Region.Height : o.Region.Height & ~1;
        long fps = o.FramesPerSecond;
        JToken cam = c["camera"];
        int every = (int?)cam?["every"] ?? 1;
        var disposables = new List<IDisposable>();
        try
        {
            // ScreenRecorder.Run's set-up, in its order (the output is reserved first).
            using (new FileStream(path, FileMode.CreateNew, FileAccess.Write, FileShare.None)) { }
            var graphics = (IDisposable)New("Video.GraphicsDevice", (nint)0);
            disposables.Add(graphics);
            var device = (ID3D11Device)Get(graphics, "Device");
            var context = (ID3D11DeviceContext)Get(graphics, "Context");
            object writer = gif ? null : Static("Encoding.MediaFoundationWriter", "Create", graphics, o, w, h);
            if (writer != null) disposables.Add((IDisposable)writer);
            int ow = writer == null ? w : (int)Get(writer, "Width"), oh = writer == null ? h : (int)Get(writer, "Height");
            var processor = (IDisposable)New("Video.GpuVideoProcessor", graphics, w, h, o.FramesPerSecond, !gif, ow, oh);
            disposables.Add(processor);
            object gifWriter = gif ? New("Encoding.GifWriter", graphics, o, w, h) : null;
            if (gifWriter != null) disposables.Add((IDisposable)gifWriter);
            var encoder = (VideoEncoderInfo)(writer != null ? Get(writer, "Encoder") : T("Encoding.GifWriter").GetProperty("Encoder", All).GetValue(null));
            object textures = writer == null ? null : New("Video.VideoTexturePool", graphics, processor, Get(writer, "EncoderBindFlags"));
            if (textures != null) disposables.Insert(0, (IDisposable)textures);
            object overlay = null;
            Guid subtype = Guid.Empty;
            int camW = 0, camH = 0, camStride = 0;
            if (cam != null)
            {
                string format = (string)cam["format"];
                camW = (int)cam["width"]; camH = (int)cam["height"];
                subtype = format == "NV12" ? VideoFormatGuids.NV12 : format == "YUY2" ? VideoFormatGuids.YUY2 : VideoFormatGuids.Rgb32;
                camStride = camW * (format == "NV12" ? 1 : format == "YUY2" ? 2 : 4);
                // A bottom-up RGB32 camera: MF_MT_DEFAULT_STRIDE is negative.
                bool bottomUp = (bool?)cam["bottom_up"] ?? false;
                var info = new CameraCaptureInfo("Synthetic camera", camW, camH, (double)cam["fps"], format);
                // CameraCapture.ReadSelectedFormat's defaults for a type without these attributes.
                uint matrix = camH > 576 ? 1u : 2u, range = 2u;
                overlay = New("Video.CameraOverlay", graphics, info, subtype, bottomUp ? -camStride : camStride, matrix, range, o, w, h);
                disposables.Insert(0, (IDisposable)overlay);
            }
            object mixer = New("Audio.AudioMixer", Clock());
            var mixerWrite = Bind<MixerWriteFn>(mixer, "Write");
            var mixerRead = Bind<MixerReadFn>(mixer, "Read");
            WriteAudioFn writeAudio = writer == null ? null : Bind<WriteAudioFn>(writer, "WriteAudio");
            var canvas = (ID3D11Texture2D)Get(processor, "Canvas");

            var slots = Synthetic.Slots(c);
            long duration = (slots[^1].Number + 1) * Synthetic.TPS / fps + ((long?)c["extra_ticks"] ?? 0);
            var packets = hasAudio ? Synthetic.Mp4Packets(c["audio"], duration) : new();
            int nextPacket = 0;
            short[] block = new short[960];
            long audioFrame = 0;
            var composited = new MemoryStream();
            var nv12 = new MemoryStream();
            int nv12Frames = 0;
            var samples = new JArray();
            void Audio(short[] b, int n, long offset) => writeAudio(b.AsSpan(0, n), offset);
            for (int k = 0; k < slots.Count; k++)
            {
                var s = slots[k];
                long timestamp = s.Number * Synthetic.TPS / fps, end = (s.Number + 1) * Synthetic.TPS / fps;
                for (; nextPacket < packets.Count && packets[nextPacket].Timestamp < end; nextPacket++)
                {
                    var p = packets[nextPacket];
                    mixerWrite(p.Source, p.Timestamp, p.Frames, p.Data, p.Gain, p.Discontinuity);
                }
                byte[] frame = Synthetic.Frame((string)c["pattern"], s.N, o.Region.X, o.Region.Y, w, h);
                fixed (byte* data = frame) context.UpdateSubresource(canvas, 0, null, (nint)data, (uint)(w * 4), 0);
                object slot = null;
                if (textures != null)
                {
                    object[] rent = { null };
                    while (!(bool)Call(textures, "TryRent", rent)) Thread.Sleep(1);
                    slot = rent[0];
                }
                if (overlay != null && k % every == 0)
                {
                    byte[] camData = Synthetic.CameraFrame((string)cam["format"], camW, camH, k / every + 1);
                    if ((bool?)cam["bottom_up"] ?? false)
                    {
                        byte[] flipped = new byte[camData.Length];
                        for (int y = 0; y < camH; y++) Array.Copy(camData, y * camStride, flipped, (camH - 1 - y) * camStride, camStride);
                        camData = flipped;
                    }
                    IMFSample sample = MediaFactory.MFCreateSample();
                    using (IMFMediaBuffer buffer = MediaFactory.MFCreateMemoryBuffer(camData.Length))
                    {
                        buffer.Lock(out nint p, out _, out _);
                        Marshal.Copy(camData, 0, p, camData.Length);
                        buffer.Unlock();
                        buffer.CurrentLength = camData.Length;
                        sample.AddBuffer(buffer);
                    }
                    Call(overlay, "Update", sample);
                }
                if (gif)
                {
                    var texture = (ID3D11Texture2D)Call(processor, "GetBgraFrame", overlay);
                    if (overlay != null) composited.Write(Read(device, context, texture, w, h, false));
                    while (!(bool)Call(gifWriter, "TryWriteVideo", texture, timestamp)) Thread.Sleep(1);
                }
                else
                {
                    Call(processor, "Convert", Get(slot, "View"), (uint)s.Number, overlay);
                    if (nv12Frames++ < 3) nv12.Write(Read(device, context, (ID3D11Texture2D)Get(slot, "Texture"), ow, oh, true));
                    using var sample = (IMFSample)Call(textures, "CreateSample", slot);
                    Call(writer, "WriteVideo", sample, timestamp, end - timestamp, s.Discontinuity);
                }
                samples.Add(new JArray(timestamp, end - timestamp, s.Discontinuity));
                if (hasAudio) WriteAudioUntil(mixerRead, block, ref audioFrame, Math.Max(0, timestamp - 50 * 10_000), false, Audio);
            }
            for (; nextPacket < packets.Count; nextPacket++)
            {
                var p = packets[nextPacket];
                mixerWrite(p.Source, p.Timestamp, p.Frames, p.Data, p.Gain, p.Discontinuity);
            }
            if (hasAudio) WriteAudioUntil(mixerRead, block, ref audioFrame, duration, true, Audio);
            if (writer != null) Call(writer, "Finish");
            var progress = new JArray();
            if (gifWriter != null) Call(gifWriter, "Finish", duration, (Action<int>)(p => progress.Add(p)), (Func<bool>)(() => false));
            if (nv12.Length > 0) File.WriteAllBytes(Path.Combine(outDir, name + ".nv12"), nv12.ToArray());
            if (composited.Length > 0) File.WriteAllBytes(Path.Combine(outDir, name + ".frames"), composited.ToArray());
            return new JObject
            {
                ["encoder"] = encoder.Name,
                ["hardware"] = encoder.IsHardwareAccelerated,
                ["d3d11"] = encoder.IsD3D11Aware,
                ["output"] = new JArray(ow, oh),
                ["duration"] = duration,
                ["samples"] = samples,
                ["progress"] = progress,
            };
        }
        finally
        {
            foreach (var d in disposables) d.Dispose();
        }
    }
}
