"""Writes cases.json for the recording harness (see README.md).

Every case is run by ShareX's own code (RecordingHarness) and by ShareXrs
(`cargo run -p sharex-screen-recording --example harness`); the results must
be identical.
"""

import json
import sys

OUT = r"C:\Users\me\Videos\ShareX\Screenshots\2026-11\ShareX_rec.mp4"
VIDEO_CODECS = ["libx264", "libx265", "libvpx", "libvpx_vp9", "libxvid", "h264_nvenc", "hevc_nvenc",
                "h264_amf", "hevc_amf", "h264_qsv", "hevc_qsv", "gif", "libwebp", "apng"]
AUDIO_CODECS = ["libvoaacenc", "libopus", "libvorbis", "libmp3lame"]
MIC = "Microphone (Realtek(R) Audio)"
SCREENS_1 = [{"bounds": [0, 0, 1920, 1080], "primary": True}]
SCREENS_3 = [
    {"bounds": [-1920, 0, 1920, 1080], "primary": False},
    {"bounds": [0, 0, 2560, 1440], "primary": True},
    {"bounds": [2560, -200, 1080, 1920], "primary": False},
]

cases = []


def rec(name, ffmpeg=None, kind="record", screens=None, **options):
    o = {"is_recording": True, "fps": 30, "area": [10, 20, 640, 480], "draw_cursor": True, "output_path": OUT}
    o.update(options)
    o["ffmpeg"] = ffmpeg or {}
    case = {"name": name, "kind": kind, "options": o}
    if screens is not None:
        case["screens"] = screens
    cases.append(case)
    return case


# Every video codec, without and with sound (gdigrab).
for v in VIDEO_CODECS:
    rec(f"gdigrab-{v}", {"VideoCodec": v})
    rec(f"gdigrab-{v}-aac", {"VideoCodec": v, "AudioSource": MIC})
# Every audio codec, with video and alone.
for a in AUDIO_CODECS:
    rec(f"gdigrab-x264-{a}", {"AudioSource": MIC, "AudioCodec": a})
    rec(f"audio-only-{a}", {"VideoSource": "", "AudioSource": MIC, "AudioCodec": a})
rec("no-source", {"VideoSource": "", "AudioSource": ""})
rec("no-source-gif-audio", {"VideoSource": "", "AudioSource": "", "VideoCodec": "gif"})

# Codec options.
rec("x264-bitrate", {"x264_Use_Bitrate": True, "x264_Bitrate": 4500, "x264_Preset": "veryslow"})
rec("x264-crf", {"x264_CRF": 17, "x264_Preset": "medium"})
rec("x265-preset", {"VideoCodec": "libx265", "x264_Preset": "placebo", "x264_CRF": 51})
rec("vp9-bitrate", {"VideoCodec": "libvpx_vp9", "VPx_Bitrate": 12345})
rec("xvid-qscale", {"VideoCodec": "libxvid", "XviD_QScale": 31})
for p in ["p1", "p7"]:
    for t in ["hq", "ll", "ull", "lossless"]:
        rec(f"nvenc-{p}-{t}", {"VideoCodec": "hevc_nvenc", "NVENC_Preset": p, "NVENC_Tune": t, "NVENC_Bitrate": 9000})
for u in ["transcoding", "ultralowlatency", "lowlatency", "webcam", "high_quality", "lowlatency_high_quality"]:
    rec(f"amf-{u}", {"VideoCodec": "h264_amf", "AMF_Usage": u, "AMF_Quality": "quality", "AMF_Bitrate": 7000})
for q in ["speed", "balanced", "quality"]:
    rec(f"amf-quality-{q}", {"VideoCodec": "hevc_amf", "AMF_Quality": q})
for p in ["veryfast", "veryslow"]:
    rec(f"qsv-{p}", {"VideoCodec": "h264_qsv", "QSV_Preset": p, "QSV_Bitrate": 2500})
rec("aac-bitrate", {"AudioSource": MIC, "AAC_Bitrate": 320})
rec("opus-bitrate", {"AudioSource": MIC, "AudioCodec": "libopus", "Opus_Bitrate": 512})
rec("vorbis-q", {"AudioSource": MIC, "AudioCodec": "libvorbis", "Vorbis_QScale": 10})
rec("mp3-q", {"AudioSource": MIC, "AudioCodec": "libmp3lame", "MP3_QScale": 0})

# Frame rates, regions, cursor, durations.
rec("fps-60", fps=60)
rec("fps-1", fps=1)
rec("negative-offset", area=[-1920, -200, 1921, 1081])
rec("no-cursor", draw_cursor=False)
for d in [3.0, 2.25, 0.05, 0.04, 10.05, 1.45, 0.35, 99.95, 123.456]:
    rec(f"duration-{d}", duration=d)
rec("duration-negative", duration=-1.0)

# Sources: ddagrab on several monitor layouts, DirectShow devices, ShareX's recorder devices.
rec("ddagrab-one", {"VideoSource": "ddagrab"}, screens=SCREENS_1)
rec("ddagrab-odd", {"VideoSource": "ddagrab"}, screens=SCREENS_1, area=[11, 21, 641, 481])
rec("ddagrab-odd-gif", {"VideoSource": "ddagrab", "VideoCodec": "gif"}, screens=SCREENS_1, area=[11, 21, 641, 481])
rec("ddagrab-left", {"VideoSource": "ddagrab"}, screens=SCREENS_3, area=[-1500, 100, 1200, 500])
rec("ddagrab-span", {"VideoSource": "ddagrab"}, screens=SCREENS_3, area=[-100, 100, 1200, 500])
rec("ddagrab-right", {"VideoSource": "ddagrab"}, screens=SCREENS_3, area=[2500, -150, 900, 700])
rec("ddagrab-outside", {"VideoSource": "ddagrab"}, screens=SCREENS_3, area=[9000, 9000, 100, 100])
rec("ddagrab-nvenc", {"VideoSource": "ddagrab", "VideoCodec": "h264_nvenc"}, screens=SCREENS_3)
rec("ddagrab-hevc-nvenc-audio", {"VideoSource": "ddagrab", "VideoCodec": "hevc_nvenc", "AudioSource": MIC}, screens=SCREENS_1)
rec("ddagrab-no-cursor", {"VideoSource": "ddagrab"}, screens=SCREENS_1, draw_cursor=False)
rec("ddagrab-upper", {"VideoSource": "DDAGRAB"}, screens=SCREENS_1)
rec("dshow-camera", {"VideoSource": "Integrated Camera"})
rec("dshow-camera-audio", {"VideoSource": "Integrated Camera", "AudioSource": MIC})
rec("dshow-quotes", {"VideoSource": 'Cam "1" \\ x', "AudioSource": 'Mic \\"2\\"'})
rec("screen-capture-recorder", {"VideoSource": "screen-capture-recorder", "AudioSource": "virtual-audio-capturer"})
rec("screen-capture-recorder-upper", {"VideoSource": "Screen-Capture-Recorder"}, draw_cursor=False)
rec("gdigrab-upper", {"VideoSource": "GDIGrab"})
rec("legacy-gdi-grab", {"VideoSource": "GDI grab"})

# User arguments and custom commands.
rec("user-args", {"UserArgs": "-loglevel warning -vf \"scale=iw/2:-2\""})
custom = '  -f gdigrab -framerate $FPS$ -offset_x $area_x$ -offset_y $Area_Y$ -video_size $area_width$x$AREA_HEIGHT$ -draw_mouse $cursor$ -i desktop -t $duration$ -y "$output$" $fps$$fps  \t'
rec("custom", {"UseCustomCommands": True, "CustomCommands": custom, "VideoCodec": "libvpx"}, duration=2.25)
rec("custom-off", {"UseCustomCommands": False, "CustomCommands": custom})
rec("custom-empty", {"UseCustomCommands": True, "CustomCommands": ""})
rec("custom-lossless", {"UseCustomCommands": True, "CustomCommands": custom}, is_lossless=True)
rec("custom-no-output", {"UseCustomCommands": True, "CustomCommands": "-i x $output$ é$duration$é"}, output_path=None)
for name, ff in [("default", {}), ("ddagrab", {"VideoSource": "ddagrab"}), ("dshow", {"VideoSource": "Cam", "AudioSource": MIC}),
                 ("gdigrab-audio-gif", {"AudioSource": MIC, "VideoCodec": "gif"}), ("audio-only", {"VideoSource": "", "AudioSource": MIC})]:
    rec(f"custom-args-{name}", ff, kind="custom_args", screens=SCREENS_3, duration=5.0)

# Two-pass: the lossless recording, the second pass, GIF through palettegen.
for v in ["libx264", "libvpx_vp9", "gif", "apng", "libwebp"]:
    rec(f"lossless-{v}", {"VideoCodec": v, "AudioSource": MIC}, is_lossless=True)
for v in VIDEO_CODECS:
    c = rec(f"encode-{v}", {"VideoCodec": v, "AudioSource": MIC}, kind="encode_video", duration=4.0)
    c["input"] = r"C:\tmp\ShareX_rec.mp4"
    c["output"] = r"C:\tmp\ShareX_out.webm"
c = rec("encode-custom", {"UseCustomCommands": True, "CustomCommands": "-i in -c:v libx264 $output$"}, kind="encode_video")
c["input"], c["output"] = r"C:\tmp\a.mp4", r"C:\tmp\b.mp4"
for stats in ["full", "diff", "single"]:
    for dither in ["none", "bayer", "heckbert", "floyd_steinberg", "sierra2", "sierra2_4a", "sierra3", "burkes", "atkinson"]:
        c = rec(f"gif-{stats}-{dither}", {"GIFStatsMode": stats, "GIFDither": dither, "GIFBayerScale": 4}, kind="encode_gif")
        c["input"], c["output"] = r"C:\tmp\ShareX_rec.mp4", r"C:\tmp\ShareX_rec.gif"

# Output paths.
rec("output-no-extension", output_path=r"C:\tmp\rec")
rec("output-dotted-folder", output_path=r"C:\tmp.d\rec")
rec("output-trailing-dot", output_path=r"C:\tmp\rec.")
rec("output-null", output_path=None, kind="commands")
rec("output-webm", {"VideoCodec": "libvpx"}, output_path=r"C:\tmp\rec.mp4")
c = rec("input-null", kind="commands", is_recording=False)

# Values ShareX can't read keep their defaults.
rec("unknown-codec", {"VideoCodec": "av1_nvenc", "AudioCodec": "flac", "x264_Preset": "ludicrous", "AudioSource": MIC})
rec("numeric-codec", {"VideoCodec": 3, "NVENC_Tune": 2})

# Concatenation (pause/resume).
cases.append({"name": "concat", "kind": "concat", "inputs": [r"C:\tmp\rec-concat.mp4", r"C:\tmp\rec.mp4"], "output": "rec-temp.mp4"})
cases.append({"name": "concat-quote", "kind": "concat", "inputs": [r"C:\tmp\it's.mp4"], "output": "o.mp4"})

# DirectShow device lists, as different FFmpeg versions print them.
cases.append({"name": "dshow-new", "kind": "dshow", "output":
              '[in#0 @ 000001f2] "Integrated Camera" (video)\r\n[in#0 @ 000001f2]   Alternative name "@device_pnp_\\\\?\\usb#vid"\r\n'
              '[in#0 @ 000001f2] "OBS Virtual Camera" (none)\r\n'
              '[in#0 @ 000001f2] "Microphone (Realtek(R) Audio)" (audio)\r\n[in#0 @ 000001f2]   Alternative name "@device_cm_{33D9}"\r\n'
              '[in#0 @ 000001f2] "virtual-audio-capturer" (audio)\r\n'})
cases.append({"name": "dshow-old", "kind": "dshow", "output":
              '[dshow @ 0abc] DirectShow video devices (some may be both video and audio devices)\n[dshow @ 0abc]  "screen-capture-recorder"\n'
              '[dshow @ 0abc]     Alternative name "@device_sw_{860BB310}"\n[dshow @ 0abc] DirectShow audio devices\n'
              '[dshow @ 0abc]  "Mikrofon (Pro "X")"\n[dshow @ 0abc]  "virtual-audio-capturer"\n'})
cases.append({"name": "dshow-none", "kind": "dshow", "output": "dummy: Immediate exit requested\r\n"})

# Output parsing: encode start and progress.
progress = [
    "ffmpeg version n9.0.2 Copyright (c) 2000-2026 the FFmpeg developers",
    "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'in.mp4':",
    "  Duration: 00:00:15.32, start: 0.000000, bitrate: 1095 kb/s",
    "",
    "Press [q] to stop, [?] for help",
    "frame=   10 fps=0.0 q=0.0 size=       0KiB time=00:00:00.33 bitrate=   0.0kbits/s speed=   0x",
    "frame=  100 fps=0.0 q=0.0 size=     256KiB time=00:00:03.33 bitrate= 629.1kbits/s speed=6.6x",
    "frame=  101 fps=0.0 q=0.0 size=     256KiB time=00:00:03.36 bitrate= 629.1kbits/s speed=6.6x",
    "PRESS [Q] TO STOP",
    "frame=  459 fps=187 q=35.0 size=    3072kB TIME=00:00:15.32 BITRATE= 660.5kbits/s speed=7.55x",
    "frame=  500 time=00:00:20.00 bitrate=1.0",
    "time=N/A bitrate=N/A",
]
for track in [True, False]:
    c = rec(f"progress-{'tracked' if track else 'untracked'}", kind="progress")
    c["lines"], c["track"] = progress, track
c = rec("progress-recording", kind="progress")
c["lines"] = ["[gdigrab @ 0x1] Capturing whole desktop", "Press [q] to stop, [?] for help", "frame= 1 time=00:00:00.03 bitrate=N/A"]
c["track"] = False

json.dump(cases, open(sys.argv[1] if len(sys.argv) > 1 else "cases.json", "w", encoding="utf-8", newline="\n"), indent=1, ensure_ascii=False)
print(len(cases), "cases")
