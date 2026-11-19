// The hardware side of ShareX's native recorder that runs headlessly:
//
// - `devices`: AudioCaptureDevices and CameraCaptureDevices, read-only
//   enumeration (whatever the machine has, including nothing);
// - `drain`: WasapiCapture.Drain itself, over a fake IAudioCaptureClient
//   that hands out the cases' synthetic packets (flags, timestamps, silent
//   and discontinuous packets), into AudioMixer: no audio device;
// - `hdr`: HdrToneMapShader.Draw on a synthetic FP16 scRGB texture into a
//   BGRA canvas, with MonitorColorInfo values from the case: no display.

using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Newtonsoft.Json.Linq;
using ShareX.ScreenRecordingLib;
using Vortice.Direct3D11;
using Vortice.DXGI;

static unsafe class Hardware
{
    const BindingFlags All = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly Assembly Lib = typeof(RecordingOptions).Assembly;
    static Type T(string name) => Lib.GetType("ShareX.ScreenRecordingLib." + name, true);

    public static JObject Devices()
    {
        JArray List(IEnumerable<(string Id, string Name)> d) => new(d.Select(x => new JObject { ["id"] = x.Id, ["name"] = x.Name }));
        JToken One(AudioCaptureDevice d) => d == null ? JValue.CreateNull() : new JObject { ["id"] = d.Id, ["name"] = d.Name };
        return new JObject
        {
            ["system_audio"] = List(AudioCaptureDevices.GetSystemAudioDevices().Select(d => (d.Id, d.Name))),
            ["microphones"] = List(AudioCaptureDevices.GetMicrophones().Select(d => (d.Id, d.Name))),
            ["default_system_audio"] = One(AudioCaptureDevices.GetDefaultSystemAudioDevice()),
            ["default_microphone"] = One(AudioCaptureDevices.GetDefaultMicrophone()),
            ["cameras"] = List(CameraCaptureDevices.GetCameras().Select(d => (d.Id, d.Name))),
        };
    }

    // ---- drain ----

    sealed class Fake
    {
        public readonly Queue<(uint Frames, uint Flags, ulong Qpc, float[] Data)> Packets = new();
        public nint Current;
    }

    static readonly Dictionary<nint, Fake> fakes = new();
    static void** vtable;

    static nint CreateFake(Fake fake)
    {
        if (vtable == null)
        {
            vtable = (void**)NativeMemory.Alloc(6, (nuint)sizeof(void*));
            vtable[0] = (delegate* unmanaged[Stdcall]<nint, Guid*, void**, int>)&QueryInterface;
            vtable[1] = (delegate* unmanaged[Stdcall]<nint, uint>)&AddRef;
            vtable[2] = (delegate* unmanaged[Stdcall]<nint, uint>)&Release;
            vtable[3] = (delegate* unmanaged[Stdcall]<nint, byte**, uint*, uint*, ulong*, ulong*, int>)&GetBuffer;
            vtable[4] = (delegate* unmanaged[Stdcall]<nint, uint, int>)&ReleaseBuffer;
            vtable[5] = (delegate* unmanaged[Stdcall]<nint, uint*, int>)&GetNextPacketSize;
        }
        // Never freed: ComPtr's finalizer may still Release it.
        nint obj = (nint)NativeMemory.Alloc((nuint)sizeof(void*));
        *(void***)obj = vtable;
        fakes[obj] = fake;
        return obj;
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static int QueryInterface(nint self, Guid* iid, void** result) { *result = null; return unchecked((int)0x80004002); }
    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static uint AddRef(nint self) => 1;
    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static uint Release(nint self) => 1;

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static int GetBuffer(nint self, byte** data, uint* frames, uint* flags, ulong* position, ulong* qpc)
    {
        Fake f = fakes[self];
        var p = f.Packets.Peek();
        f.Current = (nint)NativeMemory.Alloc((nuint)Math.Max(1, p.Data.Length * 4));
        Marshal.Copy(p.Data, 0, f.Current, p.Data.Length);
        *data = (byte*)f.Current; *frames = p.Frames; *flags = p.Flags; *position = 0; *qpc = p.Qpc;
        return 0;
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static int ReleaseBuffer(nint self, uint frames)
    {
        Fake f = fakes[self];
        f.Packets.Dequeue();
        NativeMemory.Free((void*)f.Current);
        f.Current = 0;
        return 0;
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvStdcall) })]
    static int GetNextPacketSize(nint self, uint* frames)
    {
        Fake f = fakes[self];
        *frames = f.Packets.Count == 0 ? 0 : f.Packets.Peek().Frames;
        return 0;
    }

    /// The case's packets for endpoint `e`, batch `b`: [frames, flags, qpc] each, samples generated.
    public static IEnumerable<(uint Frames, uint Flags, ulong Qpc, float[] Data)> DrainPackets(JToken endpoint, int batch)
    {
        int index = 0, current = 0;
        foreach (JArray b in endpoint["batches"])
        {
            foreach (JArray p in b)
            {
                if (current == batch)
                {
                    uint frames = (uint)p[0], flags = (uint)p[1];
                    float[] data = new float[frames * 2];
                    for (int i = 0; i < frames; i++)
                        for (int ch = 0; ch < 2; ch++)
                            data[i * 2 + ch] = ((index * 7 + i * 3 + ch * 5) % 200 - 100) / 128f;
                    yield return (frames, flags, (ulong)p[2], data);
                }
                index++;
            }
            current++;
        }
    }

    public static int Drain(JObject c, object mixer, Action<long, bool> readUntil)
    {
        Type wc = T("Audio.WasapiCapture");
        Type ep = wc.GetNestedType("Endpoint", All);
        Type client = T("Native.IAudioCaptureClient");
        Type comPtr = T("Native.ComPtr`1").MakeGenericType(client);
        object capture = RuntimeHelpers.GetUninitializedObject(wc);
        wc.GetField("mixer", All).SetValue(capture, mixer);
        MethodInfo drain = wc.GetMethod("Drain", All);
        var endpoints = new List<(object Endpoint, Fake Fake, JToken Spec)>();
        foreach (JToken e in c["endpoints"])
        {
            Fake fake = new();
            nint obj = CreateFake(fake);
            object endpoint = RuntimeHelpers.GetUninitializedObject(ep);
            object ptr = comPtr.GetConstructors(All).Single().Invoke(new object[] { Pointer.Box((void*)obj, client.MakePointerType()) });
            ep.GetField("Capture").SetValue(endpoint, ptr);
            ep.GetField("Source").SetValue(endpoint, (int)e["source"]);
            ep.GetField("Gain").SetValue(endpoint, (float)(double)e["gain"]);
            endpoints.Add((endpoint, fake, e));
        }
        int batches = endpoints.Max(e => ((JArray)e.Spec["batches"]).Count);
        long lastEnd = 0;
        for (int b = 0; b < batches; b++)
        {
            long batchEnd = 0;
            foreach (var (endpoint, fake, spec) in endpoints)
            {
                if (b >= ((JArray)spec["batches"]).Count) continue;
                foreach (var p in DrainPackets(spec, b))
                {
                    fake.Packets.Enqueue(p);
                    batchEnd = Math.Max(batchEnd, (long)p.Qpc + p.Frames * TimeSpan.TicksPerSecond / 48_000);
                }
                drain.Invoke(capture, new[] { endpoint });
            }
            lastEnd = Math.Max(lastEnd, batchEnd);
            readUntil(Math.Max(0, batchEnd - 50 * 10_000), false);
        }
        readUntil(lastEnd, true);
        object count = (object)mixer.GetType().GetProperty("Discontinuities", All)?.GetValue(mixer) ?? mixer.GetType().GetField("discontinuities", All).GetValue(mixer);
        return Convert.ToInt32(count);
    }

    // ---- hdr ----

    /// The case's scRGB value for pixel (x, y), channel ch (exact in FP16; some negative, some NaN).
    public static float HdrValue(int x, int y, int ch)
    {
        if (ch == 1 && (x * y) % 29 == 7) return float.NaN;
        float v = ((x * 5 + y * 3 + ch * 7) % 401) / 16f;
        return (x + y) % 13 == 0 ? -v : v;
    }

    public static byte[] Hdr(JObject c)
    {
        int sw = (int)c["width"], sh = (int)c["height"], cw = (int)c["canvas_width"], ch = (int)c["canvas_height"];
        int dx = (int)c["x"], dy = (int)c["y"];
        var graphics = (IDisposable)Activator.CreateInstance(T("Video.GraphicsDevice"), All, null, new object[] { (nint)0 }, null);
        try
        {
            var device = (ID3D11Device)graphics.GetType().GetProperty("Device").GetValue(graphics);
            var context = (ID3D11DeviceContext)graphics.GetType().GetProperty("Context").GetValue(graphics);
            ushort[] half = new ushort[sw * sh * 4];
            for (int y = 0; y < sh; y++)
                for (int x = 0; x < sw; x++)
                    for (int k = 0; k < 4; k++)
                        half[(y * sw + x) * 4 + k] = BitConverter.HalfToUInt16Bits(k == 3 ? (Half)1f : (Half)HdrValue(x, y, k));
            using ID3D11Texture2D source = device.CreateTexture2D(new Texture2DDescription
            {
                Width = (uint)sw, Height = (uint)sh, MipLevels = 1, ArraySize = 1, Format = Format.R16G16B16A16_Float,
                SampleDescription = new(1, 0), Usage = ResourceUsage.Default, BindFlags = BindFlags.ShaderResource
            });
            fixed (ushort* p = half) context.UpdateSubresource(source, 0, null, (nint)p, (uint)(sw * 8), 0);
            using ID3D11ShaderResourceView view = device.CreateShaderResourceView(source);
            using ID3D11Texture2D canvas = device.CreateTexture2D(new Texture2DDescription
            {
                Width = (uint)cw, Height = (uint)ch, MipLevels = 1, ArraySize = 1, Format = Format.B8G8R8A8_UNorm,
                SampleDescription = new(1, 0), Usage = ResourceUsage.Default, BindFlags = BindFlags.RenderTarget
            });
            using ID3D11RenderTargetView target = device.CreateRenderTargetView(canvas);
            context.ClearRenderTargetView(target, new Vortice.Mathematics.Color4(0, 0, 0, 1));
            object color = Activator.CreateInstance(T("Video.MonitorColorInfo"), All, null,
                new object[] { true, (float)(double)c["white"], (float)(double)c["peak"] }, null);
            var shader = (IDisposable)Activator.CreateInstance(T("Video.HdrToneMapShader"), All, null, new object[] { graphics }, null);
            try
            {
                shader.GetType().GetMethod("Draw", All).Invoke(shader, new object[] { target, view, new System.Drawing.Rectangle(dx, dy, sw, sh), color });
            }
            finally { shader.Dispose(); }
            using ID3D11Texture2D staging = device.CreateTexture2D(new Texture2DDescription
            {
                Width = (uint)cw, Height = (uint)ch, MipLevels = 1, ArraySize = 1, Format = Format.B8G8R8A8_UNorm,
                SampleDescription = new(1, 0), Usage = ResourceUsage.Staging, CPUAccessFlags = CpuAccessFlags.Read
            });
            context.CopyResource(staging, canvas);
            context.Map(staging, 0, MapMode.Read, Vortice.Direct3D11.MapFlags.None, out MappedSubresource mapped).CheckError();
            byte[] o = new byte[cw * ch * 4];
            for (int y = 0; y < ch; y++) Marshal.Copy(mapped.DataPointer + y * (nint)mapped.RowPitch, o, y * cw * 4, cw * 4);
            context.Unmap(staging, 0);
            return o;
        }
        finally { graphics.Dispose(); }
    }
}
