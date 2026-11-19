# Windows live checks

ShareX reference `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` and ShareXrs
`72045af`, both in portable mode. Input files and settings were generated
locally. Before a capture or overlay, the desktop backdrop was checked
with `scripts/pattern-check.py`. Evidence from an individual app window is
client-area PrintWindow and has a JSON sidecar. The tables use `SX` for
`G:/scripts/sharexrs-ref/shots/sharex/live-checks/` and `RS` for
`G:/scripts/sharexrs-ref/shots/sharexrs/live-checks/`.

After the user briefly took control of the desktop, both portable profiles
still had the test settings and the reference backup reverified at 13
files. Affected window captures with a `-repeat` suffix supersede the
earlier captures of the same state. Some repeated ShareX captures use a
separate portable copy of the same executable (SHA-256
`645c0e6dc55b2edb7c039cebbafbad8359b5c984609ff5c1bfb8320524c73bc7`),
because PID 48724 received commands but stopped executing
them. All retained screen-derived PNGs passed the pattern check again.

The shared-control run and the first app-window checks used 96 DPI (100%).
The subsequent desktop session and the resumed run used 120 DPI (125%) at
2560×1600. DPI comes from the captured HWND or the verified test monitor
for file-only checks. A dash marks a check with no desktop execution.

## App windows

Checks from [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent).
The exclusions also cover the additional cases on local `master` at
`48ca3d1`. The tested ShareXrs executable is still `72045af`.

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| APP-01 | App windows | 96 (100%) | File collision with Ask. Save the custom region `320,260,320,240` twice using the constant filename `capture-check`. Try Cancel, Escape, Use new name, Use unique name, Overwrite and Enter on the unchanged name. | Cancel and Escape dismissed the dialog without a new file. New name wrote `another-name.png`, unique wrote `capture-check (2).png`, and Overwrite and Enter both replaced the generated sentinel at `capture-check.png`. | Same decisions, filenames and decoded pixels. | PASS | `SX/app-file-exists-dark.png`, `RS/app-file-exists-dark.png`. All six output files passed the pattern check at 1.0000. Decoded RGB SHA-256 `0aa571ac63e04af1df72866484eabb850f5865049e6ef24868a8599388b9ae3f` in both apps. |
| APP-02 | App windows | 96 (100%) | Start with `ShowIconPackPrompt=true` and `ShowStartScreen=false`. Pick Legacy, inspect the main window, then Continue. | Prompt opened over the main window. Legacy applied before Continue. | Same prompt, live icon change and persistence. | PASS | `SX/app-icon-prompt-dark.png`, `RS/app-icon-prompt-dark.png`, and `app-icon-legacy-live-dark.png` in both directories. Both JSON files saved `ThemeOptions.IconPack="Legacy"` and `ShowIconPackPrompt=false`. |
| APP-03 | App windows | — | About window, update status and links. | Skipped. About starts an update request even with `AutoCheckUpdate=false` unless the registry policy is active. | Skipped. The equivalent update and browser routes require network. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |
| APP-05a | App windows | 120 (125%) | Save a local capture with BottomRight toast placement, 8 s duration and 1 s fade. | Showed a saved-image toast. It was gone after the timed wait. | Same appearance of a toast on completion and timeout. | PASS | `SX/app-toast-task-dark.png`, `RS/app-toast-task-dark.png`. |
| APP-05b | App windows | 120 (125%) | Save the same local pattern crop with each of the nine `ToastWindowPlacement` values on the single 2560×1600 display. Measure the client bounds. | Each toast uses the corresponding work-area anchor. Left, center and right X coordinates are −13, 1148 and 2309. Top, middle and bottom Y coordinates are −13, 676 and 1365. Client size is 264×188. | Same nine anchors and Y coordinates. Client width is 263 px, shifting the right-aligned X coordinate to 2310 while preserving the right edge at 2573. | PASS | `app-toast-{topleft,topcenter,topright,middleleft,middlecenter,middleright,bottomleft,bottomcenter,bottomright}-dark.png` and the matching JSON under `SX` and `RS`. |
| APP-06 | App windows | 96 (100%) | Close the main window to the tray twice with `FirstTimeMinimizeToTray=true`. Reopen with `-OpenMainWindow` between closes. | First close displayed a toast. Second close hid the main window with no toast. | Same behavior with identical toast client geometry, `605,626,428,103`. | PASS | `SX/app-tray-tip-dark.png`, `RS/app-tray-tip-dark.png`. Both saved `FirstTimeMinimizeToTray=false`. |
| APP-07a | App windows | 120 (125%) | Capture the generated region with Show after capture window and Save image to file. Inspect the initial dialog and cancel. | Opened the dialog. Escape returned with no `after-check.png`. | Same dialog flow and no output. | PASS | `SX/app-after-task-repeat-dark.png`, `RS/app-after-task-repeat-dark.png`. |
| APP-08a | App windows | 96 (100%), 120 (125%) | Open `-ImageViewer pattern-03.png` with `pattern-04.png` in the same generated folder. Check fullscreen and press Right once, then Escape. | Opened fullscreen at 1024×768. Right advanced from pattern-03 to pattern-04. Escape closed the window. | Opened fullscreen and closed on Escape. Right left pattern-03 displayed. | FAIL | `app-viewer-initial-dark.png` and `app-viewer-next-dark.png` under both `SX` and `RS`. The centre pixel after Right was RGB `120,90,170` in ShareX and `54,150,110` in ShareXrs. Generate the folder with `scripts/history-fixture.py`. Reproduces at 125%, with the `app-viewer-*-verified-dark.png` pair under each directory. |
| APP-11a | App windows | 120 (125%) | Enter Edit in the sidebar Tools grid and turn off Hash checker. | Hash checker is visibly switched off. The application writes its tool key into `HiddenTools`. | Same key and checked state. | PASS | `app-tools-hidden-dark.png` under both directories. Both write `["MainMenuBuilder_HashChecker"]` in `HiddenTools`. |
| APP-12a | App windows | 120 (125%) | Enable Show quick task menu with only local Save image to file. Run `-CustomRegion` over the pattern, then dismiss the popup. | Opened the quick task menu. Dismissal left no `quick-check.png`. | Same open and dismiss behavior, with no output. | PASS | `SX/app-quick-task-repeat-dark.png`, `RS/app-quick-task-repeat-dark.png`. Earlier reference-only capture is superseded. |
| APP-13a | App windows | 96 (100%) | Run `-ToggleActionsToolbar` with no saved position, start a drag at client `26,19`, move `-160,-80` in 12 steps with `scripts/input.ps1`, then middle-click the title. | Opened flush with the bottom right of the work area. Moved `-160,-80`, saved `611, 600` and closed on middle click. | Opened at the same work-area anchor and closed on middle click. Moved `-147,-73` and saved `614, 607`. | FAIL | `app-toolbar-first-dark.png` and `app-toolbar-dragged-dark.png` under both `SX` and `RS`. Persisted field `ActionsToolbarPosition`. |
| APP-13b | App windows | — | Drop a file on the Actions toolbar. | Skipped. The drop uploads the file. | Skipped for the same reason. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |
| APP-14 | App windows | 120 (125%) | Configure FFmpeg recording with the local command `-nostdin -sharex-live-invalid-ffmpeg-option`. Record the generated region with audio disabled. | Opens FFmpeg error with the unrecognized option and stderr. | Opens the same Output box with the same error. | PASS | `app-ffmpeg-error-dark.png` under `SX` and `RS`. |
| APP-26a | App windows | 120 (125%) | With `ShowClipboardContentViewer=true` and `Generated clipboard viewer test text` in the clipboard, focus the main window and press Ctrl+V. Dismiss the viewer without uploading. | Opens `ShareX - Clipboard upload` with the generated text. | The `72045af` build leaves the main window in place and logs `Upload from clipboard: not ported yet`. | FAIL | `app-clipboard-shortcut-dark.png` under both directories. Item 26 on `master` appeared after this executable was built. |
| APP-26b | App windows | — | Use Ctrl+U to upload a file task, Ctrl+D to download a URL task, and Open URL with the default browser. | Skipped. The specified actions reach an upload provider or a URL. | Skipped for the same reason. | BLOCKED | `48ca3d1:docs/APP-WINDOWS.md`, item 26. |
| APP-27 | App windows | — | Confirm or cancel Delete remotely on an S3-backed task. | Skipped. No permitted provider or 127.0.0.1 mock was used. | Skipped for the same reason. | BLOCKED | `48ca3d1:docs/APP-WINDOWS.md`, item 27. |
| APP-29 | App windows | — | Tray › Restart as administrator. | Skipped. UAC. | Skipped. UAC. | BLOCKED | `48ca3d1:docs/APP-WINDOWS.md`, item 29. |

## Video tools

Checks from [TOOLS.md](TOOLS.md#the-video-windows-in-sharexrs) and the Video row of [PORTING.md](PORTING.md).

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| VID-01 | Video tools | 120 (125%) | Open the same generated silent 8 s, 320×180 H.264 MP4 in Video converter. Choose local output and x264 constant quality 23. Start encoding. | Wrote a 224,247-byte MP4 and displayed result properties. | Same file bytes and result properties. | PASS | `video-converter-result-repeat-dark.png` under `SX` and `RS`. Output SHA-256 `fe855a7e213b5a005032219749974b099d007c09aa67f9c9c9b7f4ca6e146871`. The generated 24 fps timecode input had SHA-256 `1a24d11610a9e03f78b5de85ebe21deac2ea8ec69d2345986d34c36f098e98c8`. |
| VID-02 | Video tools | 120 (125%) | At 125% DPI open the encoder list in Video converter without resizing its 850×550 logical client. | Separate popup from y=694 to y=1324, 196 px beyond the client bottom. The initial viewport shows MPEG-4 / Xvid, GIF and WebP. | Popup stops at the client bottom, y=1128. The initial viewport ends at VP9. Scroll is needed for the entries ShareX shows immediately. | FAIL | `video-converter-ready-repeat-dark.png` and `video-converter-codecs-popup-repeat-dark.png` under `SX` and `RS`. |
| VID-03 | Video tools | 120 (125%) | Start `-AnimatedGifTrimmer` on the generated three-frame `sequence.gif`, then on a text file named `invalid.gif`. | Loads at frame 1 of 3 with duration 840 ms. The invalid file shows “Select an animated GIF with at least two frames.” | Same initial frame, duration and invalid-file message. | PASS | `video-gif-loaded-repeat-dark.png` and `video-gif-invalid-repeat-dark.png` under `SX` and `RS`. |
| VID-04 | Video tools | 120 (125%) | Start `-VideoTrimmer invalid.mp4` with a file containing the ASCII bytes `not a video`. Wait for the error. | Shows a modal and an inline error. In the repeat, the Media Foundation detail reports that the stream has no more samples, with code `0xC00D3E84`. | Shows a modal and an inline error with the generic detail “Select a video with a known, finite duration.” | FAIL | `video-trimmer-invalid-repeat-dark.png` and `video-trimmer-invalid-underlying-repeat-dark.png` under `SX` and `RS`. The earlier claim that only ShareXrs opened a modal is superseded. |
| VID-05 | Video tools | 120 (125%) | Open `-VideoTrimmer silent.mp4` using VID-01's clip. | Loads a paused first frame and sets Start to 0 and End to 8 s. | Same first frame and times. Both timelines have thumbnails after loading. | PASS | `video-trimmer-loaded-repeat-dark.png` under `SX` and `RS`. |
| VID-06 | Video tools | — | Upload generated thumbnails. | Skipped. Upload requires network. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-video-windows-in-sharexrs) |
| VID-07 | Video tools | 120 (125%) | Load the generated `sequence.gif`, choose Re-encode, drag the crop corner from client `364,192` to `414,217` and the end handle from `1070,591` to `780,591`. Save to scratch. | Shows the crop and changes the timeline. Writes a 3,899-byte GIF. | Same steps and identical output bytes. | PASS | `video-gif-crop-dark.png` and `video-gif-trim-dark.png` under both directories. GIF SHA-256 `7ed06e4a1688da89e4b660728f1c7ca8335f113da2cbe005a81bd444f66a5eac`. |
| VID-08 | Video tools | 120 (125%) | In Video player, open the generated 8 s silent timecode clip. After playback, press Space, M, Right, F11 and Escape. Check pause, forward seek and fullscreen restore. | After Right the paused timecode reads 6.917 s. F11 covers 2560×1600. Escape restores `680,409,1200,750`. The controls live in a separate window, so the client capture does not verify mute. | After Right the paused timecode reads 6.583 s. F11 and Escape give the same bounds. The mute icon is visible in the client capture. | PASS | `video-player-seek-muted-dark.png` and `video-player-fullscreen-dark.png` under `SX` and `RS`. The playing intervals before Space were not synchronized, so the timestamps establish the seek range only. |
| VID-09 | Video tools | 120 (125%) | In Animated GIF maker, add three generated PNGs, select a frame, leave delay at 500 ms, set animation repeat count to 2, and Create GIF. | Writes a 3,345-byte GIF with three frames and loop count 2. | Same frame properties and identical file bytes. | PASS | `video-gif-maker-frames-dark.png`, `video-gif-maker-repeat-dark.png`, `video-gif-maker-result-dark.png` under both directories. SHA-256 `df00c3c41c8caff9fa7d9e44f6f74a920901acfe86add13a86f086fede89f761`. |
| VID-10 | Video tools | 120 (125%) | In Video trimmer, open the silent timecode clip. Drag the end handle from client `1070,641` to `640,641`. Save separate Lossless and Re-encode copies. | Lossless writes 140,386 bytes, 4.708333 s and 113 frames. Re-encode writes 182,677 bytes, 4.625 s and 111 frames. Both are H.264 at 320×180. | Both outputs match byte for byte. | PASS | `video-trimmer-drag-dark.png` and `video-trimmer-exported-dark.png` under both directories. Lossless SHA-256 `ea7aa43152a8cc9f2a9e4b3f6d81741def304a52a9792ab91a51e21fb79bd058`. Re-encode SHA-256 `58c73348bddc1c64f5489599e26fa508c4eb889d7ed8995d51e8bf8c656f5da4`. |
| VID-11 | Video tools | 120 (125%) | Load the same silent timecode clip as `thumb.mp4` in Video thumbnailer. Leave the default contact-sheet options, turn Upload thumbnails and Open directory off, then Create thumbnails. | Writes `thumb_Thumbnail.png`, 72,920 bytes at 1000×609. | Writes the same filename and dimensions, 74,628 bytes. Decoded RGB differs in a bounding box `(11,12)–(734,443)`, with mean absolute channel differences of 0.42, 0.44 and 0.46 out of 255. | FAIL | `video-thumbnailer-input-dark.png` and `video-thumbnailer-results-dark.png` under both directories. Decoded RGB SHA-256 in ShareX `e7c182f2f5b4314726cf5a18f21d0ed9ce9adeb6b51cf8eaa67bbf27b6d2df4a` and ShareXrs `49d59ded98bfb681ef4cbb71485d7ef37d9fe7f6ba5d43ab3defcd6ab7f8edac`. |

## Data and system tools

Checks from [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) and the Data and System rows of [PORTING.md](PORTING.md).

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| DAT-01 | Data tools | 120 (125%) | Write `ShareXrs live-check fixture` plus a single LF as UTF-8. Run `-HashCheck hash.txt`, choose default SHA-256 and press Check. | Displayed the expected uppercase SHA-256. | Same value. | PASS | `data-hash-ready-repeat-dark.png` and `data-hash-result-repeat-dark.png` under `SX` and `RS`. SHA-256 `D1CDDE2CABE05501325828167FC6A70DC2B6D1A6CEB417DDA69797F7021F24E0`. Earlier `data-hash-*-dark.png` without `repeat` is superseded. |
| DAT-02 | Data tools | 120 (125%) | Open `-Metadata metadata.jpg` on the generated JPEG from `phase2-32616/inputs`. Read the total beside Metadata. | Header says `22 tags`. | Header shows the literal placeholder `{0:N0} tags`. The visible category headers and rows still populate. | FAIL | `SX/data-metadata-repeat-dark.png`, `RS/data-metadata-repeat-dark.png`. Earlier `data-metadata-dark.png` is superseded. |
| DAT-03 | Data tools | 120 (125%) | Run `-StripMetadata` on two separate copies of the DAT-02 input. | Reduced the file from 13,470 to 13,354 bytes. | Wrote exactly the same 13,354 bytes. | PASS | Post-interruption repeat output SHA-256 `234ce2b417ec4f85f0318ffca26ed637849956367b9ce4cf6681316de557f48` in both apps. Earlier strip output hashes are superseded by this repeat. |
| DAT-04a | Data tools | 120 (125%) | Create `index-fixture/alpha.txt` and `index-fixture/sub/beta.txt`. Open Tools › Index folder. In the picker, enter the absolute path to `index-fixture` in its address bar and select that folder. Generate the default HTML and use Save as. | Produces both file entries and `<title>Index for </title>`. | Produces the same file entries and `<title>Index for index-fixture</title>`. The footer timestamp also reflects its later start. | FAIL | `data-folder-index-selected-dark.png` and `data-folder-index-output-dark.png` under both directories. ShareX HTML SHA-256 `272f6cc4fa8ad98c24d583393d87fba4ca90b30aa5edb07fbcfcf39cf0f842ad`, ShareXrs `ccfb308f8efceeecb642b3a08a3b2b0eccffb8064b7fef42d084f6830aab4539`. |
| DAT-04b | Data tools | — | Upload the generated folder index. | Skipped. The upload route needs a provider or loopback network. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |
| DAT-05 | Data tools | 120 (125%) | Enter `ShareX local QR fixture` in QR code, leave Size at 0, and choose Copy image. | Copies 417×417 pixels. | Copies 418×418 pixels. The visible module pattern matches. | FAIL | `data-qr-generated-dark.png` under both directories. Decoded RGB SHA-256 in ShareX `affbdf91e441f4a9251622872552d2e6a8d1c184febbc1dafd93c0a42b5e9a20` and ShareXrs `89ac99685b3b160d430b5fbc0a7e95c4fa452bcbd95e6aaf9259331c62419c6a`. |
| DAT-06a | Data tools | 120 (125%) | Open Analyze image in an empty generated profile with auto-start off. | No input or result, with Analyze image disabled. | Same initial state. | PASS | `data-analyzer-offline-dark.png` under both directories. |
| DAT-06b | Data tools | — | Analyze image, Load models and Test connection against a provider or mock. | Skipped. Requires network and a configured endpoint. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |
| SYS-01 | System tools | 120 (125%) | Open Clipboard viewer with only the generated Unicode text `Synthetic clipboard Ω`. | Shows one UnicodeText format and a 21-character preview. | Same format, text and count. | PASS | `system-clipboard-unicode-dark.png` under both directories. |
| SYS-03a | System tools | 120 (125%) | Open Monitor test on the monitor with the verified pattern. | Opens fullscreen in Grayscale mode with the brightness controls visible. | Same mode and control state. | PASS | `system-monitor-default-dark.png` under both directories. |
| SYS-04 | System tools | — | Network monitor pings, samples, events and log. | Skipped. Opening the window starts network probes. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |

## Screen recording

Checks from [SCREEN-RECORDING.md](SCREEN-RECORDING.md).

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| REC-01 | Screen recording | 120 (125%) | Record `320,260,320,240` of the verified pattern with `-ScreenRecorderCustomRegion`, FFmpeg gdigrab, x264, no audio source, 30 fps, a 1 s configured start delay and a 2 s fixed duration. Save locally. | Wrote a 2.000 s H.264 video, 320×240, 60 video frames at 30 fps, with no audio stream. | Same streams, duration and file bytes. | PASS | Both `record-test.mp4` files were 21,980 bytes with SHA-256 `7267ca89ef92d74af18a01c33ab49f11465436cf5c89b2c70c2e3f7aa4102617`. Stream properties from local `ffprobe`. |
| REC-02 | Screen recording | 120 (125%) | Leave `record-test.mp4` from REC-01 in the output folder. Set File exists to Overwrite and use APP-14's deliberately invalid FFmpeg command. Inspect History after the error. | Shows the error, removes the previous output, and keeps the seeded 60 History rows. | Shows the error, leaves the previous MP4 unchanged and adds History row 61 for it. | FAIL | `app-ffmpeg-error-dark.png` under both directories. SQL diff `+ (61, 'record-test.mp4', 'File')` in ShareXrs. The retained MP4 has the REC-01 SHA-256. |
| REC-04 | Screen recording | — | Actual WASAPI loopback, microphone device selection and low-latency capture, mixes and meters. | Skipped. Needs real audio input or recording. | Skipped for the same reason. | BLOCKED | [SCREEN-RECORDING.md](SCREEN-RECORDING.md#left-for-a-desktop-session) |
| REC-05 | Screen recording | — | Capture a camera in the native recorder, device and mode selection, overlay, encoder and composition fallbacks. | Skipped. Needs a camera. | Skipped for the same reason. | BLOCKED | [SCREEN-RECORDING.md](SCREEN-RECORDING.md#left-for-a-desktop-session) |
| REC-06 | Screen recording | — | FFmpeg missing › Yes. Check the downloader's progress, Cancel and extraction of `ffmpeg.exe`. | Skipped. Requires a network download. | Skipped for the same reason. | BLOCKED | [SCREEN-RECORDING.md](SCREEN-RECORDING.md#finding-ffmpeg) and item 28 of `48ca3d1:docs/APP-WINDOWS.md`. |

## Scrolling capture

Checks from [SCROLLING-CAPTURE.md](SCROLLING-CAPTURE.md#not-ported-or-not-checked).

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| SCR-01 | Scrolling capture | 120 (125%) | Use PageDown, ScrollDelay 375 ms, AutoUpload off, over `--test-pattern --scroll 300,120,420,480`. Select the generated page with a click at `500,300`. | “Capture failed” after the first 420×480 viewport. No stitched page. | Same result and dimensions. | PASS | `SX/scrolling-page-down-dark.png`, `RS/scrolling-page-down-dark.png`. Covers the failure path only. The pre-overlay screen guard matched 0.9509 of samples with the generated page over the backdrop. |
| SCR-02 | Scrolling capture | 120 (125%) | Inspect the unzoomed 420×480 viewport preview from SCR-01 on the 125% display. | Displays the image at 525×600 physical pixels inside the preview. | Displays the image at 420×480 physical pixels inside the preview. | FAIL | Same shots as SCR-01. Compare the image rectangle within each window, excluding surrounding panel. |
| SCR-03 | Scrolling capture | — | Scroll by `WM_VSCROLL`. | The supplied test pattern page handles wheel and keys and has no native scroll bar to receive this message. | Same fixture limitation. | BLOCKED | [SCROLLING-CAPTURE.md](SCROLLING-CAPTURE.md#not-ported-or-not-checked) |
| SCR-04 | Scrolling capture | — | Open Help. | Skipped. Opens a web page. | Skipped for the same reason. | BLOCKED | [SCROLLING-CAPTURE.md](SCROLLING-CAPTURE.md#not-ported-or-not-checked) |
| SCR-05 | Scrolling capture | 120 (125%) | Set Down Arrow, ScrollDelay 375 ms, ScrollAmount 2 and AutoUpload off. Open `--test-pattern --scroll 300,120,420,480`. Hover at `500,300` in the selection overlay for 650 ms, click and wait for the window to restore. | Capture successful, 420×2400. | Capture successful, 420×2400. | PASS | `scrolling-downarrow-verified-dark.png` under `SX` and `RS`. The first pair, `scrolling-downarrow-dark.png`, selected different targets and is superseded. |

## History

Checks from [HISTORY.md](HISTORY.md#validation-of-the-windows) and commits
`96f91ec` and `e4df0e5` where they describe window behavior.

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| HIS-01 | History | 120 (125%) | Seed 60 synthetic image rows with paths to generated files and `example.invalid` URL text. Search for `item-42`. | Lists only `item-42.png` and previews the generated sixth pattern. | Same row and preview. | PASS | `history-search-dark.png` and `local-history-dark.png` under `SX` and `RS`. |
| HIS-02a | History | 120 (125%) | Open Image history with `MaxItemCount=6`, `AutoLoadMoreItems=false` and the 60-row fixture. | Initial six images, ordered newest first, items 60 to 55. | Same six items and order. | PASS | `local-image-history-dark.png` under `SX` and `RS`. |
| HIS-02b | History | 120 (125%) | Enable `AutoLoadMoreItems` on the same 60-row fixture and start with `MaxItemCount=6`. Scroll down beyond the first viewport. | Fills the viewport across multiple batches and shows rows 44 through 33 after scrolling. | Also fills and extends the list. Rows 44 through 33 become visible in order. | PASS | `history-auto-page-initial-dark.png` and `history-auto-page-scroll-dark.png` under both directories. |
| HIS-03 | History | 120 (125%) | Open Stats on the same fixture. | Total 60, all images, busiest day November 1, busiest hour 09:00–10:00. | Same counts and bins. | PASS | `local-stats-dark.png` under `SX` and `RS`. |
| HIS-04 | History | — | Remote URL image preview and Stats Share. | Skipped. Requires download or upload. | Skipped for the same reason. | BLOCKED | [HISTORY.md](HISTORY.md#validation-of-the-windows) |
| HIS-05 | History | 120 (125%) | Select the generated History row `item-59.png`, then press Ctrl+C and Ctrl+Shift+C. | Copies the fixture URL and the local `pattern-11.png` path. | Copies the same two values. Used the search filter for the exact row because the recording test added one History entry. | PASS | `SX/history-selected-row-dark.png`, `RS/history-selected-item59-dark.png`. Generated URL `https://example.invalid/item-59.png`. |
| HIS-06 | History | 120 (125%) | Open History › Advanced, enable Date range, click the day column of the first date and leave the pointer on that column. | The day column exposes an up and a down button. | The flyout opens, but the `72045af` build exposes no up or down buttons on the day column. | FAIL | `SX/history-date-flyout-popup-dark.png` and `RS/history-date-flyout-owner-dark.png`. Reference sidecar has `PART_DayUpButton` and `PART_DayDownButton`. Item 25 on `master` appeared after this executable was built. |

## Settings windows

Checks from [SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md#live-checks-for-the-desktop-agent).

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| SET-01a | Settings windows | 120 (125%) | Application settings › Settings. Uncheck Settings and History. | Export becomes disabled. | Export becomes disabled. | PASS | `settings-export-disabled-dark.png` under `SX` and `RS`. |
| SET-01b | Settings windows | 120 (125%) | Export with Settings and History both selected. | Exports to the verified scratch filename and shows the completion banner. | Exports the same four members and shows the completion banner. | PASS | `settings-export-done-dark.png` under both directories. Archives contained `ApplicationConfig.json`, `HotkeysConfig.json`, `UploadersConfig.json`, `History.db`. This supersedes earlier picker attempts that were safely cancelled. |
| SET-01c | Settings windows | 120 (125%) | Export with only Settings selected. | Writes the settings-only archive. | Writes the settings-only archive. | PASS | Both ZIPs contained `ApplicationConfig.json`, `HotkeysConfig.json` and `UploadersConfig.json`. |
| SET-01d | Settings windows | 120 (125%) | Export with only History selected. | Writes an archive with just `History.db`. | Writes an archive with the same member and bytes. | PASS | Both 1,738-byte archives contain `History.db` with SHA-256 `62848c5754cac94fb81410922f556a1853eb9addbe641cffb74205150d910bf0`. |
| SET-02a | Settings windows | 120 (125%) | Import the other application's SET-01b archive into each disposable profile with `AutoCheckUpdate=false` in the archive. | Shows Backup imported and returns to General. Applies the source archive's scratch screenshot path. | Same outcomes in the opposite direction. Both profiles still have updates off. | PASS | `settings-import-cross-dark.png` under both directories. Saved `CustomScreenshotsPath` matches the corresponding source JSON. |
| SET-02b | Settings windows | 120 (125%) | Import `invalid-resume.sxb`, containing generated plain text. | Opens Error while importing backup. After dismissal, Settings displays Backup import failed. | Same error flow and status. ShareX includes the .NET exception and stack, while ShareXrs shows its ZIP parser detail. | PASS | `settings-import-invalid-dark.png` and `settings-import-invalid-status-dark.png` under both directories. |
| SET-03a | Settings windows | 120 (125%) | In Settings, choose Reset settings and then No. | Dialog closed. Theme stayed Dark and `AutoCheckUpdate=false` remained. | Same outcome and retained values. | PASS | `settings-reset-confirm-dark.png` under `SX` and `RS`. |
| SET-03b | Settings windows | — | Choose Yes and check the reset, theme and hotkey reload. | Skipped. Reset reloads defaults with automatic update checks enabled. | Skipped for the same reason. | BLOCKED | [SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md#export-import-reset) |
| SET-04a | Settings windows | 120 (125%) | At 125% open Application settings › Paths and click inside the first Sub folder pattern box where it shows `%y-%mo`. | Opens the variable menu next to the field on focus. | The same click focuses the box with no variable menu visible. | FAIL | `SX/settings-path-focus-dark.png`, `SX/settings-path-focus-popup-dark.png`, `RS/settings-path-focus-dark.png`. |
| SET-05a | Settings windows | 120 (125%) | Application settings › Clipboard formats › Add. Right-click inside Format, then press Escape once. | The first Escape dismisses the variable menu and leaves Add clipboard format open. The second Escape closes the editor. | The first Escape already closes the editor. | FAIL | `settings-clipboard-variable-menu-dark.png`, `settings-clipboard-format-escape-dark.png` and `settings-clipboard-format-escape-twice-dark.png` under both directories. Native menu in `SX/settings-clipboard-variable-popup-dark.png`. |
| SET-06a | Settings windows | 120 (125%) | Enable toast in Task settings › Notifications. Open Configure, move the first action down once, and Save. | The editor shows the reordered choices and saves the changed array. | Saves the same choices and attributes. | PASS | `settings-toast-button-editor-dark.png` under both directories. `ToastWindowButtons` actions in both JSON files are `AnnotateImage, CopyImageToClipboard, PinToScreen, Upload`, each with `DismissNotification=true`. |
| SET-07 | Settings windows | 120 (125%) | Task settings › Capture › Select region. Drag from physical `312,156` to `491,275` over the pattern, then close Task settings. | X, Y, Width and Height update and persist. | Same saved values. | PASS | `settings-region-selected-dark.png` under both directories. Both save `CaptureCustomRegion="312, 156, 180, 120"`. |
| SET-08a | Settings windows | 120 (125%) | Enable Use white ShareX icon in Application settings › General and inspect the main window icon. | Saves the flag and switches the main window icon to white immediately. | Saves the flag but keeps its blue main window icon. | FAIL | `settings-white-window-icon-dark.png` under both directories. Dominant icon pixels are RGB `251,251,251` in ShareX and `37,99,235` in ShareXrs. |
| SET-09 | Settings windows | — | Toggle each Integration checkbox and exercise Send To. | Skipped. Writes real registry or shell integration. | Skipped for the same reason. | BLOCKED | [SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md#live-checks-for-the-desktop-agent) |

## Image tools and Image editor

Checks from [TOOLS.md](TOOLS.md#the-images-windows-in-sharexrs) and
[IMAGE-EDITOR.md](IMAGE-EDITOR.md#the-rest), including the AutoSizeWindow
case mentioned in `e4df0e5`.

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| IMG-01 | Image tools | 120 (125%) | Run `-ImageEffects pattern-01.png` on the generated 640×400 image with the same Canvas and Text watermark options. | Opens Image effects and shows both enabled effects and the preview. | Same effects and preview content. | PASS | `images-effects-dark.png` under both directories. |
| IMG-02 | Image tools | 120 (125%) | Run `-ImageBeautifier pattern-01.png`. | Opens the image, with the Background panel and Auto ratio selected. | Same entry point and Background panel state. | PASS | `images-beautifier-dark.png` under both directories. |
| IMG-03 | Image tools | — | Exercise Photo editor and Pixel editor. | Skipped as requested. | Skipped as requested. | BLOCKED | [TOOLS.md](TOOLS.md) |
| IMG-04 | Image tools | 120 (125%) | Open the generated 640×400 `test-input.png` in Image resizer. Keep Fit, 1920×1080 and PNG, then run Resize images. | Writes `test-input_resized.png` at 1920×1080. The PNG is 110,582 bytes. | Same filename, dimensions and decoded RGB. The PNG is 110,049 bytes. | PASS | `images-resizer-input-dark.png` and `images-resizer-result-dark.png` under `SX` and `RS`. Decoded RGB SHA-256 `836224015674d40f33268ce705615f988c3839ccf20801c1d1891db38fb15f72` in both apps. |
| ED-01 | Image editor | 120 (125%) | Run `-ImageEditor pattern-01.png` with the 640×400 generated image. At the initial 100% zoom, measure the canvas in physical pixels. | Opens maximized with the image at 640×400 physical pixels. | Opens maximized with the image at 800×500 physical pixels at the same 100% zoom. | FAIL | `images-editor-dark.png` under both directories. Client image bounds in ShareX `960,556,640,400` and ShareXrs `880,506,800,500`. |
| ED-02 | Image editor | — | Open the system print dialog. | Skipped as requested. | Skipped as requested. | BLOCKED | [IMAGE-EDITOR.md](IMAGE-EDITOR.md#print-printrs-dialogsprint_windowrs) |

## Uploaders

Checks from [UPLOADERS.md](UPLOADERS.md#the-destination-settings-windows-services)
and the live upload work referenced by commit `79b3bcc`.

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| UPL-01 | Uploaders | 120 (125%) | Open Destination settings and Custom uploader settings against the empty generated uploaders profile. | Imgur shows Not connected, Anonymous and No albums loaded. The custom uploader library is empty. | Same profile state and initial pages. | PASS | `uploader-destinations-dark.png` and `uploader-custom-empty-dark.png` under both directories. |
| UPL-02 | Uploaders | 120 (125%) | Open Tools › Remote storage browser without usable providers. | Shows the message to configure S3, FTP, FTPS or SFTP before opening the browser. | Same message and dismissal. | PASS | `uploader-remote-unconfigured-dark.png` under both directories. |
| UPL-03 | Uploaders | — | OAuth authorization, token refresh, loopback listener, account and device retrieval, 2FA and connected album or folder browsing. | Skipped. Requires accounts and network. | Skipped for the same reason. | BLOCKED | [UPLOADERS.md](UPLOADERS.md#the-destination-settings-windows-services) |
| UPL-04 | Uploaders | — | Connected remote-storage listing, refresh, filtering, folder navigation, download, rename, deletion, upload and remote image preview. | Skipped. Requires a live provider. | Skipped for the same reason. | BLOCKED | [UPLOADERS.md](UPLOADERS.md#remote-storage-browser) |
| UPL-05 | Uploaders | — | Live custom uploader tests, Email, YouTube options from upload, upload progress, errors, URL shortening and sharing. | Skipped. Requires sending data or reaching a provider. | Skipped for the same reason. | BLOCKED | [UPLOADERS.md](UPLOADERS.md#upload-tasks) |

## Run coverage and teardown

The table records 43 PASS, 16 FAIL and 22 BLOCKED. Outstanding offline
cases include the debug-log actions, tray workflows and recent items,
the missing-file message, toast hover and click actions, the remaining
settings menu interactions, stopping scrolling capture by activation or
close, native Windows Graphics Capture, and the recorder bar's Mouse
highlighter panel. The new clipboard shortcut and date-flyout findings
apply to `72045af`. The corresponding changes on `master` were outside
the executable tested here.

All 18 resumed toast source crops passed `scripts/pattern-check.py` at
1.0000. Scroll pattern startup probes that failed were deleted before
viewing. The verified Down Arrow shots supersede the first pair. No
additional desktop takeover was observed after the session resumed.

The final test ShareX and ShareXrs exited normally after their dialogs
were closed. The reference portable profile matched all 13 SHA-256
hashes before restart. ShareX restarted as PID 49164 with `-NoHotkeys`.
The startup used `AutoCheckUpdate=false`, then the exact original
`ApplicationConfig.json` bytes were restored and reverified. The test
pattern stopped and the backup and temporary portable profiles were
removed.

## Fixes for the FAILs (branch `fix/live-check-failures`)

Fixed headlessly from the evidence above, with unit tests and `--render`
checks at 100% and 125%. Each needs a live re-check against ShareX on a
build of that branch.

| ID | Cause | Fix | Re-check |
|---|---|---|---|
| APP-08a | `FileHelpers.GetAbsolutePath` ends in `Path.GetFullPath`; the port didn't normalise, so a path spelled with `/` or `..` didn't match the folder listing and the viewer had one image. | `paths::absolute_path` and the viewer normalise with `std::path::absolute`. | Right advances to pattern-04; status shows `1 / 2`. |
| APP-13a | winit's `drag_window` posts `WM_NCLBUTTONDOWN` a frame after the press, and the move loop anchors at the cursor when it's posted (`GetMessagePos`), losing the pointer's first movement. Avalonia calls `DefWindowProc` inside the press. | Dialogs' move drag (Actions toolbar, drop window) anchors at the press and follows `GetCursorPos` until the release. | Same drag saves `611, 600`. |
| APP-26a, HIS-06 | Tested on `72045af`; fixed on `master` by the inert-entries merge. | Verified in code: Ctrl+V opens Clipboard upload with `ShowClipboardContentViewer`; the date flyout has its up/down buttons. | Both as written. |
| VID-02 | Popups were egui layers inside their window. | `sharex_ui::popup`: popups place against the screen's work area (Avalonia's positioner: flip, slide, resize), and one that overflows its window is drawn in an owned, never-activated OS popup window whose input goes back to the owner. Windows only. `--render … --popup-host L,T,R,B` prints where popups go: the codec list `33,100 326x504`, Hash checker `139,189 101x218`, Capture menu `206,45` and its flipped submenu `-28,365` match the shared-control sidecars exactly. | Codec list 408×630 below the 125% client; the shared-control edge and submenu states. |
| VID-04 | FFmpeg's `Invalid data found when processing input` was reported as `VideoTrimmer_InvalidVideo`. | Mapped to Media Foundation's `MF_E_END_OF_STREAM` as .NET words it (`<system message> (0xC00D3E84)`); see TOOLS.md. | The modal and inline text. |
| VID-11 | Frames, layout, shadow and border are pixel-exact (tools harness `vthumbs-plain`); the difference is the video info and timestamp text: Skia rasterises DirectWrite glyph masks, ShareXrs fills the outlines. The byte sizes differ by PNG encoder too. | Not fixed: needs Skia m119's `SkScalerContext_win_dw.cpp` and `SkMaskGamma` ported, which aren't in the local Skia subset. | — |
| DAT-02 | `data_tools::format` replaced `{0}` only. | Composite formatting (`format_dotnet`) with the current culture's separators; no other resource uses an unformatted specifier. | `22 tags`. |
| DAT-04a | Avalonia's `IStorageFolder.Path` ends in `/`, so ShareX's title takes `Path.GetFileName` of a path ending in a separator. | Picked and dropped folders keep the trailing separator. | `<title>Index for </title>`. |
| DAT-05 | At 125% Avalonia rounds the 430 column to 538 px (430.4), leaving the preview 465.6 wide. | The column is layout-rounded (`widgets::layout_round`). | Copy image is 417×417. |
| REC-02 | ShareX's `FFmpegCLIManager.Run` with `ShowError` blocks the recording thread on the error box, so the bar stays up (`Working`) and ending it there aborts: the output is deleted and no task made. The port posted the error and finished at once. | The worker waits for the error box. | Repeat the exact steps; with the bar ended while the error shows, the file is deleted and History stays at 60. |
| SCR-02 | The preview drew image pixels as screen pixels. | An image pixel is a DIP (Avalonia's 96 DPI `Bitmap`), as is the image viewer's (its 125% shot: ShareX 1500×375 for 1200×300). | 525×600. |
| SET-04a | A click whose press and release land in one frame opened the menu and closed it again as a press outside it. | The opening frame's press doesn't dismiss (variable menus, the image effects add menu). | Menu opens on the first click. |
| SET-05a | The Format box's right click never reached the menu (egui's hit test under the editor's blocker), so Escape closed the editor. | The right click is read from the input; and generally a window or editor ignores Escape while a popup was open (`popup::escape_pressed`). | First Escape closes the menu, the second the editor. |
| SET-08a | The main window's title bar logo was always the blue one; other windows' icons didn't follow the setting. | The title bar draws the current icon (`TitleBarIcon`), and every window's icon follows (`ShareX.WindowIcon`). | Title bar icon turns white. |
| ED-01 | ShareX's `Zoom` is image pixels per screen pixel (`EffectiveZoom = Zoom / DpiScale`). | The editor draws at the effective zoom; zoom to fit multiplies by the DPI. | 640×400 physical at 100%. |

Image surfaces at 125%, from ShareX's source and shots: the editor canvas
and pinned images are screen pixels; the scrolling capture preview, the
image viewer and every `DownOnly` preview are DIPs (the main window's list
preview was fitted as screen pixels, fixed too: `thumbnail::fit_at`);
thumbnails are fitted to DIP boxes.

Also found: Avalonia sizes some windows' clients a pixel smaller than
ShareXrs at 125% (Video converter 1062×687 vs 1063×688, QR code 1175×762 vs
1175×763; the 950-wide settings window is 1188 in both). The rule isn't
clear from these, so it's left for a live check. Offscreen renders at
`--scale 1.25` laid out their first frame on egui's 8000-point default
screen (dialog renders panicked); fixed.

## Pass 2 · Windows desktop checks

ShareX `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` and ShareXrs
`832b1efdf9904c9a79b68c64afb578ff126034fc`. Both ran with generated
portable profiles. ShareXrs was built with `cargo build` offline.
One 2560×1600 display ran at 120 DPI (125%) throughout. The display
settings were left as found. `SX` and `RS` refer to the same evidence
directories as in pass 1. The prior 16 failures remain in the pass 1
tables. Rows for the new master behavior use the pass 2 executable.

### Native recording

The fixed region was `320,260,640,480` over `--test-pattern`. The MP4
frame check decoded every frame with FFmpeg and compared RGB values at
sample points inside the generated 16 px tiles. The check allows 22
levels per channel for H.264 and 40 for indexed GIF. First and last
decoded frames were then compared byte for byte for accepted pairs.
Every recording was deleted after checking.

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| REC2-01 | Native recorder | 120 (125%) | Run `-ScreenRecorderCustomRegion` twice to open the built-in bar and record. Set 30 fps and 3 s fixed duration with cursor, audio and camera off. | H.264, 640×480, 30 fps, 3.000 s, 90 frames, 540,424 bytes. NVIDIA hardware encoder, zero dropped frames. | Same stream properties, encoder and frame count. 540,423 bytes. | PASS | Preflight region matched 100%. Every decoded frame passed at 1.0000. First and last decoded RGB frames were pixel-identical across apps. |
| REC2-02 | Native recorder | 120 (125%) | Repeat REC2-01 with `-ScreenRecorderGIFCustomRegion` and the default 15 fps GIF rate. | 640×480, 3.000 s, 45 recorder slots. The GIF stores one image held over the full static clip. 120,539 bytes. | Same recorder slots, encoded image count, timing and byte size. | PASS | Sequential repeat SHA-256 `8ab65d9693e9931a12f8c2e0a88c035fd16ef5238a6e304c3e58c8e06f279b4a` in both apps. FFprobe average rate `100/300`. Decoded RGB matched across apps and passed the pattern check at 1.0000. |
| REC2-03a | Native recorder | 120 (125%) | Leave the main window visible. Launch `--test-pattern --window 320,260,801,601`, activate its uniquely titled test window and verify foreground HWND and PID. Forward `-ScreenRecorderActiveWindow` twice with `CaptureClientArea=false` and REC2-01's settings. | 800×600, 30 fps, 3.000 s, 90 frames, 942,006 bytes, alternate pattern verified on all decoded frames. | On two attempts the selected target had the main window's dimensions, rounded even to 1124×878. The repeat was 30 fps, 3.000 s, 90 frames and 1,690,621 bytes. The requested pattern matched at 0.0007. | FAIL | Both ShareXrs recordings were rejected and deleted unviewed. The ShareXrs main window measured 1125×879. The tray-hidden control is REC2-03b. |
| REC2-03b | Native recorder | 120 (125%) | Repeat REC2-03a with each app's main window hidden to the tray before activating the test window. | 800×600, 30 fps, 3.000 s, 90 frames, 942,006 bytes. | Same stream properties and frame count. 942,005 bytes. | PASS | Alternate pattern passed at 1.0000 on every decoded frame. First and last decoded RGB frames were pixel-identical across apps. |
| REC2-04a | Native recorder | 120 (125%) | Start REC2-01 over the alternate pattern, forward `-PauseScreenRecording` about 1.25 s after Record and leave it paused beyond the 3 s fixed duration. Forward the same command to resume in place. | Completed after resuming. 640×480, 30 fps, 3.000 s, 90 frames, 544,464 bytes. | Same behavior and stream properties. 544,463 bytes. | PASS | All decoded frames matched the alternate pattern at 0.9992. First and last decoded RGB frames were pixel-identical across apps. |
| REC2-04b | Native recorder | 120 (125%) | Drag the paused recording frame to a different region before resuming. | The paused tool window was verified by HWND and PID. The Windows input tool could not select it and its PrintWindow client was black. | The frame was listed by the input tool and its PrintWindow client was also black. No drag was issued. | BLOCKED | REC2-04a establishes pause and resume in place. |
| REC2-05 | Native recorder | 120 (125%) | Select the non-default GoXLR Sample output, verify it differs from all three default output roles and has no active foreign playback session. Play a generated 1 kHz tone there for 5 s with a scratch WASAPI player. Run REC2-01 with only system audio enabled. | AAC 48 kHz stereo, 3.007979 s, 1 kHz present in all thirty 100 ms windows. 614,063 bytes. | Same codec, rate, channels, duration and tone coverage. 614,089 bytes. | PASS | Playback session guard found no active foreign streams. FFmpeg decoded 144,384 samples per channel in both files. Video was 640×480 at 30 fps, 90 frames, with 1.0000 pattern match and pixel-identical first and last decoded frames. |
| REC2-06 | Native recorder | 120 (125%) | Make a short recording with a verified muted microphone or a virtual input fed silence. | No safe microphone state was verified. | No microphone recording attempted. | BLOCKED | Microphone was disabled for every recording. |
| REC2-07 | Native recorder | 120 (125%) | Enable a covered or virtual camera and verify its placement and lifetime. | No covered or synthetic camera was verified. | No camera recording attempted. | BLOCKED | Camera was disabled for every recording. |
| REC2-08 | Native recorder | 120 (125%) | In Task settings › Capture › Screen recorder, compare all three device lists. Select Sample output, Sample input, and Cam Link 4K, turn capture sources off, close settings, and compare saved IDs. | Order after the default row: **output** Chat, Game, Music, Sample, System (all GoXLR). **Input** Broadcast Stream Mix and Chat Mic (GoXLR), HDMI (Cam Link 4K), Microphone (HD Pro Webcam C920), Sample (GoXLR). **Camera** Cam Link 4K, HD Pro Webcam C920. | Same default labels, names and order. All three selected IDs were nonempty and matched byte for byte across apps. | PASS | `pass2-device-{system,mic,camera}-dark.png` under SX and RS. All sources were confirmed off after the list test. |
| REC2-09 | Native recorder | 120 (125%) | Compare the three source dropdowns on the recorder bar itself. | The reference bar HWND was verified for PrintWindow and unavailable as an input target. | The ShareXrs bar was targetable. A side-by-side list comparison from this surface was unavailable. | BLOCKED | Task settings list and ID results are in REC2-08. |

### App windows and task list

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| APP2-01a | Debug log | 120 (125%) | Debug › Show debug log › Copy all in each generated profile. | Copied 197 lines including startup and recording completion entries. | Copied 168 lines including corresponding startup and recording completion entries. | PASS | Clipboard contents checked against generated run markers. The clipboard offered zero formats before the run and again after clearing the fixture. |
| APP2-01b | Debug log | 120 (125%) | Choose Upload log, click Cancel on the confirmation, then choose Open log file. | Cancel dismissed the warning. Open log file opened the generated monthly log in Notepad. | Same warning, dismissal and local log viewer. | PASS | `pass2-debug-confirm-dark.png` under SX and RS. The log body is outside the retained crops. |
| APP2-02a | Task list | 120 (125%) | With `ShowClipboardContentViewer=true`, load `ShareXrs pass two generated clipboard fixture` on the clipboard. Press Ctrl+V and dismiss the viewer with Escape. | Viewer showed the 45-character text. Escape dismissed it. | Same viewer, text and dismissal on current master. | PASS | `pass2-ctrl-v-dark.png` under SX and RS. |
| APP2-02b | Task list | 120 (125%) | Seed `recent-local.txt` as a recent task and set `DisableUpload=true`. Select its file card and press Ctrl+U. | Started the task and displayed a modal saying the feature will not work while DisableUpload is enabled. | Started the task, logged the same warning and finished the row without showing the modal. | FAIL | `SX/pass2-ctrl-u-dark.png`. ShareXrs log: `Task status: Preparing`, warning, `Task status: Completed`. No upload. |
| APP2-02c | Task list | 120 (125%) | Seed a URL-only recent task with the local `file:///…/download-source.txt` URI. Select it and press Ctrl+D. | Started a download task, then displayed a modal with `Download failed: System.NotSupportedException: The 'file' scheme is not supported.` | Started a download task, logged `Download failed: The 'file' scheme is not supported.` and left the row stopped. No modal. | FAIL | `SX/pass2-ctrl-d-dark.png`. No file was written. The fixture contains a local URI and never addresses a remote host. |
| APP2-02d | Task list | 120 (125%) | Set `ThumbnailClickAction=OpenURL` and `BrowserPath` to a generated offline launcher that records one approved argument. Seed a URL-only recent task for `https://example.invalid/fixture-open.txt`. Click its card body once and its title once. | Passed the exact URL to the launcher twice. | Passed the exact URL to the launcher twice. | PASS | The launcher logged one line per click and had no network code. |
| APP2-02e | Task list | 120 (125%) | Start from a generated portable profile with `RecentTasksShowInMainWindow=true`, thumbnail titles on and two recent items with URL and no file path. Use `file:///…/download-source.txt` and `https://example.invalid/fixture-open.txt`. | Titles are `download-source.txt` and `fixture-open.txt`. | Both titles are `Untitled task`. The adjacent file-backed `recent-local.txt` title is correct. | FAIL | `pass2-seeded-dark.png` under SX and RS. |

### Tray

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| TRAY2-01 | Tray | 120 (125%) | Forward `-ToggleTrayMenu` from each portable app with generated recent rows and workflows. | App-owned tray popup opened with Workflows and Recent items. | Same entries in the app-owned tray popup. | PASS | `pass2-tray-root-dark.png` under SX and RS. The notification-area icon itself was unavailable in the input tool. |
| TRAY2-02 | Tray | 120 (125%) | Seed a default Hash Check workflow bound to F8, an unbound edited Hash Check workflow and a None job. Inspect labels and run the edited workflow from the tray. | The tray root was visible. Its independently hosted popup could not be selected as an input target. The sidebar showed both labels, F8 and the edited asterisk, omitted None and opened Hash Checker. | The tray showed the matching labels, F8 and asterisk. Selecting the edited workflow opened Hash Checker. | BLOCKED | `SX/pass2-sidebar-workflows-dark.png`, `RS/pass2-tray-workflows-dark.png`. Side-by-side tray interaction and the edited-only filter were unresolved. |
| TRAY2-03 | Tray | 120 (125%) | Tray › Recent items › generated local `recent-local.txt` › Copy, then Open. | Recent items was visible in the tray root. The independently hosted popup was unavailable as an input target. | Copied the exact generated path. Open launched the generated text file in Notepad. The test tab was closed and the clipboard cleared. | BLOCKED | `RS/pass2-recent-actions-dark.png`. |

### History

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| HIS2-01 | History | 120 (125%) | History › Advanced › Date range. Open the first picker's day column at 19 November 2026, click down once and up once, and accept. | Day changed from 19 to 20 and back to 19. The accepted date showed 19 November 2026. | Same buttons, step order and accepted date. | PASS | `pass2-date-{down,up}-dark.png` under SX and RS. This row exercises the new buttons on `832b1ef`. |

### Mouse highlighter

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| HIL2-01 | Mouse highlighter | 120 (125%) | Over the generated pattern, open the native recorder bar's Mouse highlighter arrow. Compare Ripple, Circle highlight and the Primary button color popup. Change alpha, increment Ripple size, select Circle and cancel the bar. | The reference bar was visible and verified by HWND. Neither the bar nor its host was available as an input target. A click addressed to the main window was rejected at the bar's verified position. | Opened Ripple and Circle and the alpha popup. After canceling the bar, saved `Mode=Circle`, `RippleSize=61` and `PrimaryColor="128, 54, 169, 225"` in `ToolsSettings.MouseHighlighterOptions`. | BLOCKED | `pass2-recorder-highlighter-bar-dark.png` under SX and RS, plus `pass2-recorder-highlighter-{ripple,primary-color,circle}-dark.png` under RS. The live effects and light-theme comparison remain open. |
| HIL2-02 | Mouse highlighter | 120 (125%) | Task settings › Tools › Mouse highlighter › Primary button color. Open the popup and change alpha without changing RGB. | Avalonia tabbed ColorView has vertical value and alpha sliders. Saved alpha 128 and RGB 54, 169, 225. | The shared popup has numeric RGBA controls and horizontal hue and alpha sliders. Saved alpha 198 at the selected point with the same RGB. | FAIL | `pass2-hi-settings-color-dark.png` under SX and RS. This verifies the existing ColorView gap documented in [TOOLS.md](TOOLS.md#the-screen-tools-in-sharexrs). |

### Scrolling capture

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| SCR2-01 | Scrolling capture | 120 (125%) | MouseWheel, ScrollAmount 2, ScrollDelay 1000 ms, AutoUpload off. Start on the generated `--test-pattern --scroll 300,120,420,480` page and activate the minimized capture window before the run finishes. | Opened the selection overlay over the verified page. The input tool omitted the overlay HWND, so selection was canceled via its verified window close. | Activated the verified capture window while the outline was present. It returned early with `Capture successful`, 420×1280 of the 420×2400 page. | BLOCKED | `RS/pass2-scroll-activated-dark.png` shows the status and dimensions. |
| SCR2-02 | Scrolling capture | 120 (125%) | Repeat with the generated page at `40,700,420,480` and close the minimized capture window while the outline is present. | Reference selection was unavailable for the same reason as SCR2-01. | Closed the verified window while the 422×482 outline was present. Both capture window and outline were gone on the next check, and the app stayed running. | BLOCKED | PID-scoped window geometry and lifecycle result. |
| SCR2-03 | Scrolling capture | 120 (125%) | Let MouseWheel run to completion on the same relocated page. Copy and verify the stitched image. | Reference selection was unavailable for the same reason as SCR2-01. | Reported `Capture successful`, 420×2400. The copied pixels matched 0.9808 of sampled points against the exact generated page and failed the 0.9900 guard. The image was deleted unviewed and the clipboard cleared. | BLOCKED | `RS/pass2-scroll-full-dark.png` retains status only. |
| SCR2-04 | Scrolling capture | 120 (125%) | Exercise the Scroll message method with `WM_VSCROLL`. | The generated scroll page does not host a native scroll bar or implement this message. | Same fixture limitation. | BLOCKED | PageDown and Down Arrow already have pass 1 results and were outside this check. |

### Toast and missing files

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| TOAST2-01 | Toast | 120 (125%) | Enable a 15 s task toast at MiddleCenter. Save a local `900,540,640,480` crop from the verified pattern and move the pointer over the image. | Showed the generated path and task title, revealed the action buttons and stayed open past 15 s. | Same hover behavior and duration hold. | PASS | `pass2-toast-hover-dark.png` under SX and RS. Both source PNGs matched the pattern at 1.0000. |
| TOAST2-02 | Toast | 120 (125%) | With the TOAST2-01 action row visible, move the pointer onto Copy image and wait longer than 400 ms. The button center was about 1192,920 physical pixels in both apps. | Opened a separate 108×39 tooltip above Copy image. | No tooltip window or text appeared after the dwell or after a second pointer movement within the button. The toast and its buttons stayed visible. | FAIL | `pass2-toast-tip-dark.png` under SX and RS. The reference evidence is the tooltip HWND. The ShareXrs evidence is its toast client at the hovered position. |
| TOAST2-03 | Toast | 120 (125%) | Click Copy image and compare image-body click actions and focus. The disposable profiles configured left, right and middle to Close notification. | The independently hosted toast HWND could be inspected and hovered, but the click was rejected by the input tool. | A click sent to Copy image yielded no clipboard image. A right click on the image did not immediately close the toast. It later disappeared. | BLOCKED | Button dispatch and focus parity could not be verified. |
| TOAST2-04 | Toast | 120 (125%) | Drag the toast image to Explorer and compare SpecificMonitor behavior on another display. | Toast input was unavailable and the machine had one verified display. | Same single-display limitation. No test drag. | BLOCKED | Existing placement and expiry results are in pass 1. |
| APP2-03 | App windows | 120 (125%) | Save a generated image with `-CustomRegion`, move only the scratch output away, then select its retained thumbnail and press Ctrl+E and Ctrl+Enter. Also try `-ImageEditor` with the absent scratch path. | The stale task commands did not open the missing-file message. The CLI rejected the path before the message handler. | Same result on both paths. | BLOCKED | The missing-file message itself was not reached. No editor or file dialog opened. |

### Settings

| ID | Area | DPI (scaling) | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|---|
| SET2-01 | Settings | 120 (125%) | Task settings › Notifications › Configure › Add. Inspect the unused actions, pick Close notification and Cancel. | Menu lists eight unused actions. Cancel preserved the original four buttons. | Same eight names and order. Cancel preserved identical `ToastWindowButtons` JSON. | PASS | `pass2-notify-add-popup-dark.png` under SX and RS. |
| SET2-02 | Settings | 120 (125%) | Task settings › File naming › Use custom time zone. Select (UTC-01:00) Azores and close settings. | Saved `Azores Standard Time`, offset `-01:00:00`. | Saved the same zone and identical decoded `CustomTimeZone` JSON, including adjustment rules. | PASS | `pass2-time-zone-dark.png` under SX and RS. The OS time zone remained `GMT Standard Time`. |
| SET2-03 | Settings | 120 (125%) | Open the Time zone dropdown in the same unresized settings window from its default UTC selection. | Popup begins above the client and shows 15 rows, starting at Araguaina and including Asuncion. | Popup begins at the client top and displays 13 rows, starting at Brasilia. The shared entries appear in the same order. | FAIL | `pass2-time-zone-dark.png` under SX and RS. Reference popup y=271; client y=334. |
| SET2-04 | Settings | 120 (125%) | Task settings › File naming, click inside the first `native-%i` box with the unresized settings window centered. Press Escape with the menu open. | Menu opened to the right of the field, extending beyond the client. Escape dismissed it and left settings open. | Same category labels and Escape behavior. Menu opened to the left of the field over the existing form labels. | FAIL | `pass2-naming-menu-dark.png` under SX and RS. This check uses File naming, a different field from the pass 1 Paths failure. |

### Pass 2 coverage and teardown

Pass 2 recorded **15 PASS, 8 FAIL and 14 BLOCKED**. The 43 PASS,
16 FAIL and 22 BLOCKED from the first pass remain in their original
tables. The clipboard shortcut and date button checks here use the new
master executable. UAC, the FFmpeg downloader, live uploads and external
Help links retain their earlier exclusions. Coverage on independent
reference popups was limited by the Windows input tool's window list.
The surviving gaps include the full tray submenus, recorder bar panels,
paused retargeting, the missing-file dialog, full toast click and drag
parity, native `WM_VSCROLL` and the remaining settings combinations.

The first ShareXrs start reached Welcome because the fixture was staged
under `ShareX` instead of the port's `ShareXrs` portable folder. The owned
process stopped at Welcome, before update initialization. Corrected
starts loaded `AutoCheckUpdate=false` before initialization. The first
GIF probe encountered an in-progress reserved filename, and the
recordings briefly overlapped. The reported GIF pair is the sequential
repeat. Two visible-main-window WGC attempts targeted ShareXrs's own
fixture window and were deleted unviewed after the pattern guard rejected
them. Failed scrolling preflights and the rejected stitched image were
also discarded unviewed. After a modal keyboard attempt opened the
generated recent text file, its Notepad tab was closed. The reference
upload-disabled and download-error dialogs were closed by exact HWND and
PID after the input tool could not target them.

The original ShareX PID 49164 and both fixture instances exited via
`-ExitShareX`. After closing all fixture dialogs, all 13 original
reference profile files matched their backup SHA-256 hashes. The
reference was restarted without arguments as **PID 36232**. The
restored profile enabled automatic updates, so startup used a temporary
`AutoCheckUpdate=false` value and the exact original settings bytes
were restored immediately afterward. The startup hotkey registration
warning was closed by its verified HWND and the main window finished
opening. The final verification found 12 unchanged hashes. The monthly
log still contained its exact original bytes followed by 2,664 bytes
written during normal startup.

The temporary profiles, backup, recordings, generated data, test
pattern, ShareXrs, tone players and local fixture launcher were removed
or exited. The generated Notepad tabs were closed and the clipboard
offered zero formats again. New PNGs and sidecars are in the pass 2
evidence set under SX and RS. The offline build and macOS Clippy check
completed.


## Pass 3 · Windows desktop checks

ShareX reference `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` and ShareXrs
built from local master `11f9e78` at the start of the run. Master advanced
during the session. The codec, shared-control popup and settings popup
repeats marked below also use `81fab7e`. Both builds completed with
`cargo build` offline. The apps used generated portable profiles, and
the reference test executable was a SHA-256-identical copy of
`ShareX-build/ShareX.exe`. The test display measured 2560×1600 with a
2560×1540 work area at 120 DPI (125%). The existing display settings
were used throughout.

Evidence uses the same `SX` and `RS` directories as earlier passes.
Every Pass 3 screenshot has a `pass3-` prefix and a JSON sidecar with
its client rectangle, DPI, process, build and SHA-256. The original
Pass 1 and Pass 2 tables remain historical results.

### App windows and History

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| APP-08a | 120 (125%) | Open `-ImageViewer` on the generated `viewer/../viewer/pattern-03.png` with `pattern-04.png` as its only sibling. Confirm fullscreen, press Right once, then Escape. | Fullscreen at 2560×1600. Right changes the image to pattern-04. Escape closes the viewer. | Same fullscreen, image change and dismissal. | PASS | `pass3-app-viewer-{initial,next}-dark.png` in SX and RS. In both, the centre changes from RGB `53,149,109` to `117,87,167`. |
| APP-13a | 120 (125%) | With the toolbar position unset, run `-ToggleActionsToolbar`. Drag from client `26,19` by `-160,-80` in 12 steps via `scripts/input.ps1`. Middle-click the title. | Moves the exact delta and closes. Saves `2082, 1411`. | Moves the exact delta and closes. Saves `2071, 1410`. | PASS | `pass3-app-toolbar-{first,dragged}-dark.png` in SX and RS. Both started flush with the work area. Toolbar clients were 318×49 and 329×50 respectively. The original absolute `611, 600` position requires the former 96 DPI work area. |
| APP-26a | 120 (125%) | Enable `ShowClipboardContentViewer`, put `Generated clipboard viewer test text` on the clipboard, focus the main window and press Ctrl+V. Dismiss with Escape. | Opens Clipboard upload containing the generated text. Escape closes it. | Same viewer, text and dismissal. | PASS | `pass3-app-clipboard-shortcut-dark.png` in SX and RS. Clipboard cleared after the pair. |
| HIS-06 | 120 (125%) | History › Advanced › Date range. Click the day column of the first date and leave the pointer there. | Day column shows up and down controls. | The same two controls appear. | PASS | `SX/pass3-history-date-flyout-popup0-dark.png` and `RS/pass3-history-date-flyout-owner-dark.png`. |


### Video and data tools

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| VID-02 | 120 (125%) | Open Video converter with a generated silent H.264 clip. Open the encoder list without resizing the default 850×550 logical client. | An owned 408×630 popup contains MPEG-4 / Xvid, GIF and WebP in its first viewport. Its bottom is 196 px below the client. | Same viewport and popup size. Its bottom is 197 px below the client. Repeated on `81fab7e`. | PASS | `SX/pass3-video-converter-codecs-popup-dark.png` and `RS/pass3-video-converter-codecs-popup-latest-dark.png`. Client heights are 687 and 688 px. |
| VID-04 | 120 (125%) | Launch `-VideoTrimmer invalid.mp4` with the exact ASCII bytes `not a video`. Wait for the modal and inspect the inline error. | Both surfaces show that the stream has no more samples, with `0xC00D3E84`, on the configured repeat. | Same Media Foundation detail and code on both surfaces. | PASS | `SX/pass3-video-invalid-{modal,underlying}-repeat-dark.png` and `RS/pass3-video-invalid-{modal,underlying}-dark.png`. |
| DAT-02 | 120 (125%) | Launch `-Metadata metadata.jpg` on the generated JPEG from `phase2-32616/inputs`. Read the total beside Metadata. | `22 tags`. | `22 tags`. | PASS | `pass3-data-metadata-dark.png` in SX and RS. |
| DAT-04a | 120 (125%), setup only | Stage `index-fixture/alpha.txt` and `index-fixture/sub/beta.txt`. Tools › Index folder. Enter the fixture's absolute path in the picker address bar, select the folder, generate the default HTML and Save as. | Fixture staged. The native folder picker has no suggested initial folder. | Fixture staged. The native folder picker has no specified initial folder. | BLOCKED | The picker starts with an unverified shell location. The exact selection and HTML title are still unverified. See [the safe picker prerequisites](REFERENCE-SHOTS.md#phase-2-prerequisites-and-fixtures). |
| DAT-05 | 120 (125%) | In QR code, enter `ShareX local QR fixture`, leave Size at 0 and click Copy image. | Copies a 417×417 bitmap. | Copies a 417×417 bitmap with identical decoded pixels. | PASS | `pass3-data-qr-generated-dark.png` in SX and RS. Both clipboard PNGs decode to RGB SHA-256 `affbdf91e441f4a9251622872552d2e6a8d1c184febbc1dafd93c0a42b5e9a20`. |

### Shared-control popups

Comparison uses the [shared-control references](REFERENCE-SHOTS.md#shared-controls-shared-controls).
The source shots use 96 DPI on a 1024×768 display. These comparisons
exercise the same control states with the current, larger work area.
ShareXrs uses `81fab7e` for this group.

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| CTL3-01 | 120 (125%) | Open the Video converter codec combo at its centered default position, then reopen with the converter client at `750,785` to constrain the downward placement. | The owned codec HWND extends below the centered client. At the low position it flips above the combo and beyond the client top. | Same overflow and flip in an owned HWND. At the low position the popup starts at `791,360` versus `791,358` in ShareX. | PASS | `pass3-controls-edge-video-{owner,popup}-dark.png` in SX and RS, plus VID-02 evidence. The smaller 96 DPI reference work area uses a slide instead. |
| CTL3-02 | 120 (125%) | Open Hash Checker with the generated `alpha.txt`, place the client at `845,835`, and open Hash type. | The algorithm list occupies an owned 126×270 HWND from `1019,1070` and extends below the owner. | The same algorithm list extends below the owner in an owned 126×273 HWND from `1019,1071`. | PASS | `pass3-controls-edge-hash-positioned-{owner,popup}-dark.png` in SX and RS. |
| CTL3-03 | 120 (125%) | Place the main client at `1864,39`, the scaled equivalent of the reference Capture menu's right-edge position. Open Capture and hover Screenshot delay. | The submenu flips left of Capture and starts at `1828,495` in a separate HWND. | The submenu flips left of Capture and starts at `1829,495` in a separate HWND. The Capture root fits inside the main client and is drawn there. | PASS | `SX/pass3-controls-edge-menu-sub-latest-popup{0,1}-dark.png`, `RS/pass3-controls-edge-menu-open-latest-dark.png` and `RS/pass3-controls-edge-menu-sub-latest-popup0-dark.png`. |


### Capture, recording and Image Editor

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| REC-02 | 120 (125%) | Leave the old `record-test.mp4` from a local REC-01 setup. Seed 60 History rows. With File exists set to Overwrite, run `-ScreenRecorderCustomRegion` for `320,260,320,240` using silent gdigrab and the custom command `-nostdin -sharex-live-invalid-ffmpeg-option`. Forward `-AbortScreenRecording` while the FFmpeg error is still open, then dismiss it. | Error and working frame coexist. After abort and dismiss, the old MP4 is gone and History has 60 rows. | Same working frame, error, file removal and 60 rows. | PASS | `pass3-recording-invalid-error-dark.png` and `pass3-recording-working-frame-dark.png` in SX and RS. File existence and SQLite counts were checked before and after. The screen guard matched 0.9966 overall and 1.0000 inside the recording region. |
| SCR-02 | 120 (125%) | Use the SCR-01 setup on `--test-pattern --scroll 300,120,420,480`, with PageDown, 375 ms delay and AutoUpload off. Click the generated window at `500,300` in the verified selection overlay. Measure the unzoomed preview. | Capture failed with a 420×480 result. The preview occupies `300,120,525,600` in client pixels. | Same status, result dimensions and preview rectangle. | PASS | `pass3-scrolling-page-down-dark.png` in SX and RS. The retained images show different offsets of the same generated page because it was reused. The tested viewport and scale are identical. |
| ED-01 | 120 (125%) | Run `-ImageEditor pattern-01.png` with the generated 640×400 PNG. At initial 100% zoom, measure the image inside the maximized window. | Image bounds `960,556,640,400` in client pixels. | Same physical image bounds at 100%. | PASS | `pass3-editor-100pct-dark.png` in SX and RS. Colour samples from the generated checker and accents were used to measure the image boundary. |
| CAP3-01 | 120 (125%) | Enable the save-with-dialog task, leave uploads disabled and run `-CustomRegion` for the verified `900,540,640,480` pattern crop. Save PNG into the generated output tree. | Opens the native save dialog. Accepting the default name writes a 640×480 PNG. | Opens the native save dialog. Entering `chosen-pass3.png` writes an identical decoded 640×480 PNG. | PASS | Both saved files passed the exact crop guard at 1.0000 and decoded to RGB SHA-256 `62235e3f9ece771f005a22d4d8abf2c97945dc1f9ca2eb685b5038bdf285e75f`. The custom name in the reference picker remains unverified. |
| CAP3-02 | 120 (125%) | Run CAP3-01 with Save thumbnail image to file also enabled. Set width 200, height 0 and suffix `-thumbnail`. | Writes a 200×150 JPEG alongside the PNG, with the saved basename and suffix. | Same placement, dimensions and filename rule. | PASS | The decoded JPEGs differ by less than 0.09 RGB levels per channel on average. The PNGs were verified against the pattern before inspecting the thumbnail output. |
| CAP3-03 | 120 (125%), setup only | Send image to printer after capture. | Skipped. | Skipped. | BLOCKED | The system print dialog stayed closed. |


### Application and Hotkey settings

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| SET-04a | 120 (125%) | Application settings › Paths. Click once inside the first Sub folder pattern field showing `%y-%mo`. | A variable menu opens alongside the field on the first click. | The first click opens the menu, including on `81fab7e`. | PASS | `SX/pass3-settings-path-focus-popup0-dark.png` and `RS/pass3-settings-path-focus-latest-popup0-dark.png`. |
| SET-05a | 120 (125%) | Application settings › Clipboard formats › Add. Right-click inside Format, press Escape once, then press it again. | The right click opens the variable menu. The first Escape dismisses the menu, and the second closes the editor. | Same sequence, including on `81fab7e`. | PASS | SX `pass3-settings-clipboard-variable-popup0-dark.png` and `pass3-settings-clipboard-format-escape{,-twice}-dark.png`. RS `pass3-settings-clipboard-variable-menu-latest-dark.png` and `pass3-settings-clipboard-format-escape{,-twice}-latest-dark.png`. |
| SET-08a | 120 (125%) | Enable Use white ShareX icon in Application settings › General and inspect the main title bar immediately. | Title-bar icon turns white. The flag saves as true. | Title-bar icon turns white immediately. The flag saves as true. | PASS | `pass3-settings-white-window-icon-dark.png` in SX and RS. The dominant icon pixels were RGB `251,251,251` and `255,255,255` respectively. |
| HOT3-01 | 120 (125%) | In English Hotkey settings, add two unbound rows. With a row selected, hover Move up, Move down and the row pencil long enough for a tooltip. | Shows Move up, Move down and Edit task settings… in tooltip windows. | Shows the same three labels inside the client. | PASS | `pass3-hotkeys-{move-up,move-down-repeat,row-edit}-popup0-dark.png` in SX and the corresponding `-owner-dark.png` shots in RS. |
| HOT3-02 | 120 (125%) | Seed two unbound Hash checker rows. Give the first `UseDefaultAfterCaptureJob=false` and `AfterCaptureJob=None`. At the stock 850×750 client, hover the first row's custom pencil at client `594,139` and dashed status icon at `801,139` for 1.1 s each. | The custom marker and dashed status icon appear. Neither hover shows tooltip text or creates a tooltip HWND. | Shows `This task has custom settings` and `Not configured` respectively. | FAIL | `pass3-hotkeys-custom-owner-dark.png` and `pass3-hotkeys-status-exact-owner-dark.png` in SX and RS. The reference status hover was also attempted with the two freshly added unbound rows. |
| HOT3-03 | 120 (125%) | Drag the stock English Hotkey settings window's right resize border toward the left by more than the available shrink distance. Inspect the toolbar at its minimum. | Stops at a 775×750 client, with the toolbar and Reset button visible. | Stops at the same client size with all toolbar controls visible. | PASS | `pass3-hotkeys-min-width-repeat-dark.png` in SX and RS. The measured English minimum is 620 logical pixels. |


### Drag and drop and custom uploader

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| DROP3-01 | 120 (125%) | With `DisableUpload=true`, drag the generated `alpha.txt` from the scratch OLE source onto the main client. Hold the drag over the client before releasing. | Dims the client and shows the centered Release to upload files card with the upload icon and file description. | Same card, text, icon and dimming. | PASS | `pass3-drop-file-hover-dark.png` in SX and RS. Both drops returned OLE Copy. |
| DROP3-02 | 120 (125%) | With `DisableUpload=true`, repeat with the scratch source's Unicode text `Pass 3 generated drop text`. | Dims the client and shows the Release to upload text card and text description. | Same card, text, icon and dimming. | PASS | `pass3-drop-text-hover-dark.png` in SX and RS. Both drops returned OLE Copy. |
| DROP3-03 | 120 (125%) | Complete DROP3-01 and DROP3-02 by releasing the generated file and text on the main client with `DisableUpload=true`. | Opens an upload-disabled modal after each release. | Shows a task toast and logs the upload-disabled warning after each release. No modal appears. | FAIL | `SX/pass3-drop-disabled-modal-{991798,5579242}-dark.png`. ShareXrs logged the warning twice and showed a separate toast HWND each time. The drag runs used disabled uploads before the loopback mock started. |
| UPL3-01 | 120 (125%) | Custom uploader settings › Test › Text uploader. Use the sole generated uploader, with a form POST to `http://127.0.0.1:38353/pass3`. Configure `{select}` with Alpha and Beta, `{inputbox}` with a title and default, and `{outputbox}` plus `{response}` as the result expression. Submit the test sample, select Beta, enter `generated parser input` and dismiss the output box. | Shows the two choices, the selected default input and the generated output text. Response shows `http://127.0.0.1:38353/result`. | Same choices, prompts, request and response. The Select client is 250×123 versus 250×128 px. | PASS | `pass3-uploader-{select-initial,inputbox,outputbox,response}-dark.png` in SX and RS. The mock received two identical bodies, `choice=Beta&input=generated+parser+input`. |


### Pass 3 coverage and teardown

Pass 3 recorded **24 PASS, 2 FAIL and 2 BLOCKED**. Fourteen of the
fifteen requested Pass 1 re-checks passed. DAT-04a remains blocked by
the picker startup location. The two failures came from the new
drop and hotkey tooltip checks. VID-11 retains its accepted status.
The eight Pass 2 failures under separate repair were excluded.

One recording preflight and two scrolling startup grabs failed their
pattern thresholds and were deleted unviewed. Successful recording
and save regions passed at 1.0000. The valid FFmpeg baseline was
repeated after the initial fixture interpreted `None` as a nonexistent
DirectShow device name. It produced no media on that attempt. The
corrected fixture used an empty audio source and both old output files
contained only video. The first ShareX invalid-trimmer attempt showed
`oldValue has incorrect type for this property`. The configured repeat
produced the Media Foundation code recorded above.

ShareXrs Scrolling capture initially stayed minimized after forwarding
the CLI job with the pattern in front. Forwarding the job again reached
the verified overlay on the completed run. A subsequent reset attempt
did not present a safe selection target in time. Its test window and
pending overlay were closed by HWND. The reference save task completed
with its default filename. The optional custom-name repeat was canceled
when focus could not be verified. The loopback File test rejected
`FormURLEncoded` before sending a request, so the reported parser check
uses the Text test. The fixture's startup icon prompt and app dialogs
omitted by the input window list were handled with the verified window
identity.

The original ShareX PID 36232 and the portable test instances exited
through `-ExitShareX`. All 13 original portable profile files matched
their SHA-256 backup before restart. The reference restarted without
arguments as **PID 53424**. A temporary `AutoCheckUpdate=false` value
suppressed its startup check, then the exact original config bytes
were restored. The startup hotkey warning was dismissed and the main
window was the only remaining reference window. The final verification
found 12 unchanged hashes and the original monthly log bytes followed
by 2,663 bytes from startup.

The scratch profiles, backup, pattern, drag source and loopback server
were removed or exited. The clipboard offered zero formats. The 97
new PrintWindow PNGs and their DPI sidecars passed hash verification.
Both offline builds and the macOS Clippy check completed.

## Fixes for pass 2 (branch `fix/live-checks-2`)

Fixed headlessly with unit tests, `--render` at 100% and 125%,
`--render --popup-host` and the new `tools/editor-harness --control-dump
colorview`. Pass 3 ran on `11f9e78`/`81fab7e`, before this branch, so
every row still needs a live re-check on a build of it. The two Pass 3
FAILs are at the end.

| ID | Cause | Fix | Re-check |
|---|---|---|---|
| REC2-03a | winit's `focus_window` fakes an Alt press before `SetForegroundWindow`, so a command line forwarded to a visible main window took the foreground from the test window before the recording read it. Avalonia's `Window.Activate` is a bare `SetForegroundWindow`, which Windows refuses without foreground rights. | Master's `activation` module fixed the main window; this branch routes every window's activation through it (`window::activate`, 32 call sites: dialogs, tool windows, overlays, tray menu). | Same repro with the main window visible: 800×600 of the alternate pattern. Also: hotkey and tray-launched windows still come to the front. |
| APP2-02b | `IsUploadAllowed`'s message boxes were only logged. | A task's thread asks for a message box and waits for it (`TaskUi::message_box`, `UiRequest::Message`), as `MessageBox.Show` off the UI thread does. | Ctrl+U on the file card: the "will not work while DisableUpload" box; the toast comes after it closes. |
| APP2-02c | `DownloadFromURL`'s failure was only logged. | The same waiting box, with `Exception.ToString()`'s type for the errors whose .NET type is known (`System.NotSupportedException`, `System.UriFormatException`). .NET's stack trace lines aren't ported. | Ctrl+D on the `file:///` task: `Download failed:` / `System.NotSupportedException: The 'file' scheme is not supported.` |
| APP2-02e | `CreateHistoryTask` sets `Info.FileName = RecentTask.FileName` (`GetFileNameSafe` of the path, else the URL); the port left it empty. | `RecentTask::file_name`, `file_helpers::get_file_name_safe`. | Cards titled `download-source.txt` and `fixture-open.txt`. |
| HIL2-02 | The egui stand-in. | Master's ColorView port (`79a1330`, 0.09% against this shot at 125%). This branch compared every tab with Avalonia's own headless renders (`--control-dump colorview`) and fixed the Components tab: ShareX's `TextBox`/`NumericUpDown` `MinHeight="40"` beats the template's `Height="32"` (6.8–7.8% → 1.4–1.6%; the rest is the harness's unthemed text boxes). Spectrum 0.11%/0.13%, palette 0.10%/0.68% (dark/light). | The flyout's three tabs, alpha change saves alpha 128 with RGB 54,169,225; the Components tab's box heights. |
| TOAST2-02 (and TOAST2-03's click) | The toast registered its card's click interaction after the action buttons; egui's hit test gives the pointer to the last widget sensing clicks, so the buttons were never hovered or clicked. | The card is sensed first. A headless test rests on Copy image past 400 ms (tooltip asked) and clicks it (image copied); it fails without the fix. | The 108×39 tooltip above Copy image; a click copies the image (TOAST2-03). |
| SET2-03 | The Pass 2 build (`832b1ef`) predates the popup windows (`a95e526`), so the list stopped at the client top. Also: Fluent's `PART_Popup` sets no `Placement`, so it's `Popup`'s default `Bottom` (centred, not left-aligned); opening scrolled the list's bottom margin into view too; items weren't layout-rounded (34 pt rows are 42 px at 125%). | Centred placement, `ScrollIntoView` of the item only, layout-rounded rows. `--popup-host` puts the list at client `570,-61` 370×630, ShareX's place and size; the list's pixels compare at 0.94% with ShareX's popup (8.93% before). | 15 rows, Araguaina to UTC, the popup above the client. |
| SET2-04 | The Pass 2 build had no popup host, so the menu was bounded by the window and flipped left. | Nothing new: on master the menu opens right of the field (`--popup-host`: client `1135,138`, 210×230). At 125% ShareX's is 226 tall: Avalonia rounds each part to pixels (border, padding, margin, 42 px items); left for the 125% layout work. | Opens to the right of the field. |
| DROP3-03 | Same cause as APP2-02b: a drop makes a file or text task whose `ThreadDoWork` calls `IsUploadAllowed`. | Covered by the APP2-02b fix; a test pins the text drop. As in ShareX, a dropped file's task toasts its file after the box closes. | Both drops show the modal. |
| HOT3-02 | ShareX sets the two tooltips on an `IconPresenter` and on a `Grid` with no `Background`. Avalonia hit-tests those only where they draw, so the outline icons are under the pointer on their strokes only; at the icons' centres nothing opens. | The tooltips open only over the icon's painted pixels (`icons::painted_at`); the pencil's centre is 2.3 units from its outline. | No tooltip at either icon's centre; on a stroke pixel, both show. |

Sweep of ShareX's other `MessageBox.Show` calls that ShareXrs reached but
only logged or skipped (same commit as APP2-02b/c): `ImageData.Write`'s
write error (the capture task goes on, and `WriteToFile` now makes the
folder first, as ShareX), `FileHelpers.OpenFolder`'s missing folder
(Application settings' two Open buttons, Open screenshots folder, the
image effect packager, and the debug log's startup path, which opened
the folder as a file), `CreateDirectory`'s and `RenameFile`'s errors
(History/Image history rename, the recording's custom file name) and
`ApplicationLifecycle.ForceClose`'s question while a recording is on
(menu and tray Exit, the Exit hotkey, closing the main window without
the tray). Not ported: the clipboard upload retry question
(`ExternalException`; ShareXrs's clipboard layer reports no error),
`PersonalPathManager`'s and `HistoryManager`'s file errors, and the dev
build's "up to date" box (dev builds aren't ported).

Regression: every compare script (menus, data, image, image-finish,
screen, system and video tools, editor dialogs; dark and light) was run
with a baseline binary that has master's `controls.rs` and `color_view.rs`
and with this branch's: 240 compared states, none changed (the dropdown
change alone against the pre-merge controls: also 240 unchanged). The
mouse highlighter flyout stays at 0.09% against ShareX's 125% shot.

## Pass 4 · Windows desktop checks

ShareX reference `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` and ShareXrs
built offline from master `8723def1d238150b078cc33e8af47a0466a73e98`.
Both executables ran from generated portable copies with automatic updates
off. The reference copy matched the original executable's SHA-256. The
single 2560×1600 display had a 2560×1540 work area at 120 DPI (125%).
Display settings were left as found. Evidence uses the same SX and RS
directories as the earlier passes. The new PrintWindow images and their
SHA-256 sidecars start with `pass4-`.

| ID | DPI (scaling) | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|---|
| APP2-02b | 120 (125%) | Seed the generated `recent-local.txt` recent task with `DisableUpload=true`, select its file card and press Ctrl+U. | The task opens the DisableUpload information box. | Same wording and 600×189 client. The task remains in Preparing while the box is open and the toast appears after OK. | PASS | `pass4-ctrl-u-dark.png` in SX and RS. |
| APP2-02c | 120 (125%) | With uploads disabled, select the URL-only card for the generated `file:///…/download-source.txt` and press Ctrl+D. | The modal begins with “Download failed:” and “System.NotSupportedException: The 'file' scheme is not supported.” and continues with the .NET stack trace. | The modal has the same error and type. It contains the two-line error only, as documented in the fix. | PASS | `pass4-ctrl-d-dark.png` in SX and RS. Only the local `file:` URL was used. |
| APP2-02e | 120 (125%) | Start with recent tasks saved and shown in the main window, thumbnail titles on, and two URL-only items for `file:///…/download-source.txt` and `https://example.invalid/fixture-open.txt`. | Titles are `download-source.txt` and `fixture-open.txt`. The file card is titled `recent-local.txt`. | All three titles match. | PASS | `pass4-seeded-repeat-dark.png` in SX and RS. The external URL was displayed as fixture text. |
| SWP4-01 | 120 (125%) | Use a temp portable profile with a custom screenshots path, remove its empty directory after startup, and invoke `-OpenScreenshotsFolder`. | An information box says “Folder does not exist:” and shows the configured scratch path. | Same text, path, icon, OK action and 600×199 client. The path wraps at a different character. | PASS | `pass4-missing-folder-dark.png` in SX and RS. |
| REC2-03a | 120 (125%) | Leave the fixture main window exposed. Click and verify the foreground HWND/PID of `--test-pattern --window 320,260,801,601`. Forward `-ScreenRecorderActiveWindow` twice, with `CaptureClientArea=false`, native H.264 at 30 fps for 3 s, and cursor, audio and camera off. | 800×600, 30 fps, 3.000 s, 90 frames, 942,006 bytes. | Same stream properties and frame count. 942,005 bytes. | PASS | The launch helper used no console window. The alternate pattern matched at 1.0000 on every decoded frame in both apps, including the exposed-main-window repeat. First and last decoded RGB frames were identical across apps. |
| ACT4-01 | 120 (125%) | Bind global Hash Checker hotkeys to Ctrl+Shift+F10 in ShareXrs and Ctrl+Shift+F11 in ShareX. Send each chord with the other app in the foreground. | Hash Checker opens and is the foreground HWND. | Same result. | PASS | `pass4-hotkey-hash-dark.png` in SX and RS. |
| DROP3-03 | 120 (125%) | With `DisableUpload=true`, use an owned OLE source to hold and release the generated `alpha.txt` and the Unicode text `Pass 3 generated drop text` on the main client. | Both OLE drops return Copy and each release opens the DisableUpload modal. | Same modal after each release, with the file toast after dismissal. | PASS | `pass4-drop-{file,text}-{hover,modal}-dark.png` in SX and RS. The drop targets were 120 DPI. The WinForms source reported 96 DPI and was scaled by Windows on the 125% display. |
| HIL2-02 | 120 (125%) | Task settings › Tools › Mouse highlighter › Primary button color. With the fixture at A=166 and RGB 54,169,225, open Spectrum, Palette and Components. On Spectrum click the vertical alpha midpoint at screen y=845, then inspect Components and close settings. | All tabs render. Components has 40 logical px inputs. Before the edit blue reads 225. After the edit the fields read A=128 and RGB 54,169,225, and the same value saves. | All three tabs are present with the matching layout and nominal Components input heights. The Components blue value reads 224 before and after the edit. A=128 is shown and saved, but the saved RGB is 54,169,223. | FAIL | Original re-check still fails on RGB preservation. `SX/pass4-color-{spectrum,palette,components,alpha}-dark.png` and RS's corresponding `-owner-dark.png` images. Saved `DefaultTaskSettings.ToolsSettings.MouseHighlighterOptions.PrimaryColor` was checked in each portable config. |
| TOAST2-02 | 120 (125%) | On a 15 s MiddleCenter toast for a local `900,540,640,480` pattern crop, hover the image to reveal actions. Move to Copy image at screen `1192,920` and dwell for 600 ms. | Shows a 108×39 Copy image tooltip above the button. | Shows the same label in a 109×40 tooltip above the button. | PASS | `SX/pass4-toast-tip-dark.png`, `RS/pass4-toast-extra-tip-dark.png`. Both source crops passed at 1.0000. |
| TOAST2-03 | 120 (125%) | Click the Copy image action on the same toast with an empty clipboard. The fixture's left, right and middle image-body actions were Close notification. | Copies a 640×480 bitmap identical to the saved source. | Copies a 640×480 bitmap with identical decoded RGB to the source and ShareX. | PASS | Decoded RGB SHA-256 `62235e3f9ece771f005a22d4d8abf2c97945dc1f9ca2eb685b5038bdf285e75f` in both. This checks the Copy image action. |
| SET2-03 | 120 (125%) | In the centered unresized Task settings › File naming window, open the Time zone dropdown from UTC. | A 370×630 popup starts at y=271, 63 px above the client, and displays 15 rows from Araguaina through UTC, including Asuncion. | The same size and 15 rows. The popup starts at y=273, 61 px above the client. The top strip is darker in the PrintWindow image. | PASS | `pass4-time-zone-popup-dark.png` in SX and RS. |
| SET2-04 | 120 (125%) | In the same unresized window, click the first `native-%i` field and press Escape with the menu open. | The variable menu opens right of the field at `1823,473`. Escape closes the menu and leaves settings open. | Same opening side, categories and dismissal at `1822,472`. The popup is 230 px tall versus 226 px as previously documented. | PASS | `pass4-naming-popup-dark.png` in SX and RS. |
| HOT3-02 | 120 (125%) | Seed two unbound Hash checker rows with the first set to `UseDefaultAfterCaptureJob=false` and `AfterCaptureJob=None`. In the 850×750 physical client, dwell 1.1 s at the original coordinates `594,139` and `801,139`, the blue pencil's interior at `511,139`, and measured stroke pixels. | No tooltip at the original coordinates or the blue pencil interior. The pencil stroke at `504,142` shows This task has custom settings. The status stroke at `802,149` shows Not configured. | Same no-tooltip results on the centers. Stroke hits at `506,144` and `801,149` show the same labels. | PASS | `pass4-hotkey-*-centre-dark.png` in both. SX `pass4-hotkey-*-stroke-tip-dark.png` and RS `pass4-hotkey-*-stroke-repeat-dark.png`. Stroke spots were measured separately from each app's pixels. |
| REG4-01 | 120 (125%) | Open each hotkey-launched Hash Checker's Hash type dropdown and press Escape. | Opens a 126×270 owned popup that extends below the client. Escape dismisses the popup. | Same position, size, and Escape behavior. | PASS | `pass4-hash-dropdown-dark.png` in SX and RS. |
| ACT4-02 | 120 (125%) | With the test pattern in front, invoke `-ToggleTrayMenu` and choose Application settings from the tray menu. | The settings window becomes the foreground HWND. | Same result. | PASS | `pass4-tray-menu-dark.png` and `pass4-tray-application-settings-dark.png` in SX and RS. The shell reported both notification-area icons at its hidden-icons overflow anchor, which was covered by the host, so the menu was opened with its command-line job. |
| SWP4-02 | 120 (125%) | Start an unfixed-duration native recording over the verified `900,540,640,480` test pattern using `-ScreenRecorderCustomRegion` twice. While recording, invoke `-ExitShareX` and choose Yes. Cursor, audio and camera are off. | The warning asks whether to abort the active screen recording, with Yes and No. Yes closes the recording frame, leaves ShareX running, and produces no completed MP4. | Same wording, actions and outcome. | PASS | `pass4-recording-exit-dark.png` in SX and RS. Both rectangle preflights passed at 1.0000. The dialogs were moved beyond the recording region before clicking Yes. |
| REG4-02 | 120 (125%) | Launch `-ImageEditor` on the generated 640×400 `pattern-01.png`. Leave Rectangle selected, open the Solid stroke-style dropdown, and inspect the selected row. Press Escape. | Image Editor takes the foreground. The dropdown lists Solid, Dash, Dot, Dash Dot and Dash Dot Dot. Solid has black text on the blue selection. Escape closes the editor. | Same activation, options and Escape behavior. Solid has light text on the blue selection. | FAIL | New visual mismatch observed in the sweep. `pass4-editor-stroke-style-dark.png` in SX and RS. |

### Pass 4 coverage and teardown

Pass 4 recorded **15 PASS, 2 FAIL and 0 BLOCKED**. HIL2-02 is the
remaining failure from the requested re-checks. REG4-02 is a newly
observed failure from the optional editor sweep.

The first disposable profile had recent-task saving off and reused one
global hotkey across both apps. The corrected profiles enabled saving,
used distinct shortcuts and loaded all three fixture cards. A plain
click opened the generated `recent-local.txt` in Notepad. The generated
tab was closed and modifier-click was used to select the cards for the
shortcut checks.

An early active-window recording launched through a shell start that
changed the foreground. The 1124×878 MP4 was rejected on dimensions
and deleted unviewed. Another setup was aborted when the window
geometry showed the console had taken the foreground. The successful
repeats used a no-console launcher and verified the pattern HWND and
PID immediately before each forward. Several toast grab preflights and
two saved crops were rejected and deleted unviewed. The mismatching
area was under the automation cursor overlay. Moving the overlay clear
of the crop yielded 1.0000 preflights, source images and clipboard
images. The scratch captures and recordings were removed after checking.

The original ShareX PID 53424 and all disposable ShareX instances
honored `-ExitShareX`. All 13 original profile files matched their
backup SHA-256 hashes before restart. The original reference restarted
without arguments as **PID 19224**. Startup used a temporary
`AutoCheckUpdate=false` value and the original config bytes were copied
back afterward. The hotkey warning was dismissed. The main window was
the only remaining ShareX window. Twelve hashes still matched, and the
monthly log retained its exact original bytes followed by 2,663 bytes
from normal startup.

The disposable apps, test patterns and OLE source exited. The backup
and all test profiles and fixtures were deleted. The clipboard
reported zero formats. Display settings were unchanged. All 63 new
PrintWindow PNGs and DPI sidecars passed hash verification. The offline
build and macOS Clippy check completed.

## Fixes for pass 4 (branch `fix/live-checks-4`)

Fixed headlessly with unit tests, `--render` and two new
`tools/editor-harness --control-dump` modes run against ShareX's own
controls (`borderstyle`, `colorpanel`). Pass 4 ran on `8723def`, before
this branch, so both rows need a live re-check on a build of it.

| ID | Cause | Fix | Re-check |
|---|---|---|---|
| HIL2-02 | The colour APIs (`ColorView::show`, `color_popup*`, `color_view::popup`, the recent colours) carried egui's `Color32`, which premultiplies. At A=166 a blue of 225 came back as 224 when the flyout opened, and the caller's round trip saved 223 after the alpha edit. The view's HSV maths was exact. | `04db4d6`: the APIs carry unmultiplied `[R, G, B, A]` (`color_view::Rgba8`) and `Color32` is used only to paint. Every caller converts its setting losslessly: Task settings' mouse highlighter (`A, R, G, B`) and canvas fill (`#AARRGGBB`), the editor's options, background, canvas size, new image and effect colours, image effects and gradient stops, Pin to screen, the Monitor test, the accent colour and the screen colour picker's result. `42cdde8`: with exact colours, the fill colour's default `#00FFFFFF` matched the palette's `Transparent` and showed a selection that ShareX doesn't. `--control-dump colorpanel` shows why. ShareX's `ColorPickerPanel` replaces three palette entries on `Loaded`, after the `ListBox` bound its selection, so those entries are selected only after the colour changes while shown. The view now models that. | From A=166 RGB 54,169,225: Components reads blue 225 before the edit. The alpha midpoint gives A=128 with RGB 54,169,225 shown and saved. Also: the editor's fill dropdown opens with no palette selection, and choosing the palette's Transparent then selects it. |
| REG4-02 | The editor's option lists painted the active row's text in `ShareX.Brush.Text`, the value the item template sets. That value is a `DynamicResource` set at **Template** priority (DataTemplate content), so ShareX's `Button.active /template/ ContentPresenter TextBlock` and `IconPresenter` styles win at StyleTrigger priority with `ShareX.Brush.Accent.Foreground` (#111111). `--control-dump borderstyle` prints Solid `#ff111111` StyleTrigger and the others at Template. The previews' `Path` strokes keep `ShareX.Brush.Text`. | `67e37dc`: `editor_popups::list`'s active row draws its text and icons in the accent foreground and its previews in `Text`. Covers stroke style, arrow style, text alignment, step type, start number, font size and cursor type, in the editor and region capture. The grids (thickness, corner radius) keep their `Ellipse`/`Path` colours. The font list already did this. The shared `ComboBox`'s selected item already used the accent foreground (`ComboBoxItem:selected TextBlock`). | Rectangle's stroke-style dropdown: Solid in black on the accent with its line still light. Also the arrow style, alignment (icon and text), step type, start number, font size and cursor type dropdowns. |

Sweep for other colours with alpha that went through `Color32` and back
to settings: besides the callers above, the image editor's background
panel and effect dialog wrote the popup's `Color32` back every frame it
was open. Task settings' canvas fill wrote premultiplied channels. The
canvas resize fill and the New image colour reached the bitmap through
`Color32`. All of these are fixed in `04db4d6`. The rest only paint:
theme and accent colours, the toast's fixed colours, previews, and the
image tools' opaque `ColorPicker`s, whose alpha is off. The engine crates
(`sharex-image-editor`, `-image-effects`, `-settings`, `-tools`,
`-history`) don't use `Color32`.

Regression: each compare script (menus, editor dialogs, editor effects,
editor popups, data, image, image-finish, screen and system tools,
scrolling; dark and light where the script has both) ran with master's
binary (`668da63`) and this branch's. That is 271 compared states. Six
changed, all editor popups, and all moved closer to ShareX:

- text alignment: dark 0.08% → 0.07%, light 0.09% → 0.08%;
- font size: dark 0.12% → 0.11%;
- cursor type: dark 1.18% → 1.17%;
- step type: light 0.05% → 0.04%;
- stroke style: unchanged at 0.26% under the threshold. A crop shows
  Solid now black like ShareX's.

The fill-colour popup is unchanged at 0.05%. After `04db4d6` alone it
was 0.08% (the palette selection), before `42cdde8`. Video tools weren't
run: their FFmpeg states can write beside their inputs under
`sharexrs-ref`, and they use no colour or picker code.
`image-tools-compare.sh` stops at `comparer-identical` on a missing
reference input (`tools-ui\in\photo.png`) for both binaries. It and the
menu script ran from copies that write to a temp folder, not into
`sharexrs-ref`. The 125% mouse highlighter flyout in Components after
the edit (`--render color-view --content flyout --state components
--text "128, 54, 169, 225" --scale 1.25`) is 1.01% against
`SX/pass4-color-alpha-dark.png` for both binaries. The difference is the
hex box's text anti-aliasing; both read `36A9E180`.

## Pass 5 · Windows desktop checks

ShareX reference `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` (the
running portable build, PID 19224) and ShareXrs built from master
`a5e4b3d075df19e1c6f85c2ed2bd6474140bb25c` with `cargo build --offline`.
ShareXrs ran from a temporary copy with a `Portable` marker and
`AutoCheckUpdate=false` in place before startup. The single display
measured 2560×1600 with a 2560×1540 work area and 120 DPI (125%).
Display settings were left as found. Evidence paths below are relative
to the `pass5/` subdirectories of SX and RS. The editor used the same
generated 640×400 `photo.png` in both apps.

| ID | Repro | ShareX result | ShareXrs result | Status | Evidence |
|---|---|---|---|---|---|
| HIL2-02a | Task settings › Tools › Mouse highlighter › Primary button color. Start at A=166, RGB 54,169,225. Open Components, switch to Spectrum and click the vertical alpha midpoint, then check Components and close settings. | Blue is 225 before the edit. Afterward Components reads A=128, RGB 54,169,225 and hex `36A9E180`. | Same initial and edited values. Both save `128, 54, 169, 225` in `DefaultTaskSettings.ToolsSettings.MouseHighlighterOptions.PrimaryColor`. | PASS | `hil-components-before.png`, `hil-components-after.png` in SX and RS, and `RS/saved-colours.json`. |
| HIL2-02b | Open a fresh Image Editor on `photo.png`, select Rectangle, and open its Fill colour dropdown with the default transparent fill. Click the bottom-right Transparent palette cell. Click empty space within the popup to move the pointer off the cell. | The palette opens without selection. The click selects Transparent, and the blue cell stays selected after the pointer leaves. | The palette opens without selection. The checkerboard cell shows a hover outline on click, then returns to its initial unselected appearance when the pointer leaves. | FAIL | `fill-initial.png` and `fill-after-pointer-away.png` in SX and RS. |
| REG4-02a | Select Rectangle in the editor and open the Solid stroke-style dropdown. | Solid has black text on blue and a light line preview. The other rows are Dash, Dot, Dash Dot and Dash Dot Dot. | Same text, selection, line preview and options. | PASS | `stroke-style.png` in SX and RS. |
| REG4-02b | Select Arrow and open the arrow-style dropdown. | Classic has black text on blue and a light preview. The list also shows Double, Modern, Basic and Line. | Same selection, order and preview colours. | PASS | `arrow-style.png` in SX and RS. |
| REG4-02c | Select Text and open alignment. | Center has a black icon and text on blue, between Left and Right. | Same icon, text and selection. | PASS | `alignment.png` in SX and RS. |
| REG4-02d | Select Step and open the step-type dropdown. | The numeric sequence has black text on blue. The other four rows are upper- and lowercase letters and Roman numerals. | Same selection and order. | PASS | `step-type.png` in SX and RS. |
| REG4-02e | Select Step and open start number. | The list shows 1 through 10, with 1 in black on blue. | Same selection and order. | PASS | `start-number.png` in SX and RS. |
| REG4-02f | Select Text in the maximized Image Editor with its default 48 pt setting. Open the font-size dropdown and inspect the editable value at the top before typing. | The 48 pt row has black text on blue. The editable value `48` opens selected. | Same rows and 48 pt selection. The editable field opens with a caret after `48` and no text selection. | FAIL | `font-size.png` in SX and RS. |
| REG4-02g | Restore the Image Editor to its 1600×900 physical client. Press K to select Cursor, then open the Default dropdown without scrolling. | Default is black on blue. An owned popup extends below the client and shows the full list through Wait cursor. | Default is black on blue. The list stays within the client and initially ends at Size NESW. Size NS through Wait cursor are below the visible area. | FAIL | `cursor-type.png` in SX and RS. |
| REG5-01 | Pin `photo.png` over the verified test pattern with `-PinToScreen`. Open the gear menu's options and edit Border color with the first-row blue palette swatch and Spectrum alpha. Confirm A=128 in Components, dismiss the picker and click Save. | The picker shows A=128, RGB 0,120,215. Save updates the pin. | Same RGBA and applied colour. Both save `128, 0, 120, 215` in `ToolsSettings.PinToScreenOptions.BorderColor`. | PASS | `pin-color-edited.png` in SX and RS, and `RS/saved-colours.json`. |
| REG5-02 | In the editor's Background panel, select Color, open the swatch, choose the first-row blue cell and set alpha to 128 on Spectrum. Check Components and close the editor. | Opens, edits and displays A=128, RGB 0,120,215. | Same RGBA. Both application configs save `BackgroundType="Color"` and `BackgroundColorHex="#800078D7"` in `ToolsSettings.ImageEditorOptions`. | PASS | `bg-components-after.png` in SX and RS, and `RS/saved-colours.json`. |
| REG5-03 | Open `-ImageEffects photo.png`, select the Canvas effect with margin 0,0,0,30 and edit its Color. Choose the same blue and set alpha to 128. Check Components and close the tool. | Opens, edits and displays A=128, RGB 0,120,215. | Same RGBA. Both save the Canvas preset's `Color` as `128, 0, 120, 215`. | PASS | `effect-edited.png` in SX and RS, and `RS/saved-colours.json`. |
| REG5-04a | Open Monitor test over the verified pattern. Choose RGB, open Color, select the first-row blue palette cell and dismiss the picker without moving a slider. | Shows and retains `#0078D7` on the RGB control. | Same colour and in-window retention. Neither tool has a Save action for this session setting. | PASS | `SX/monitor-rgb-edited.png` and `RS/monitor-rgb-neutral-dismiss.png`. |
| REG5-04b | With Monitor test on RGB `#0078D7` and Color open, click the exposed left end of the red slider at screen `1000,273` to dismiss the popup. | Popup closes. Red stays 0 and the shown colour stays `#0078D7`. | The same click also moves the red slider. Red becomes 1 and the shown colour changes to `#0178D7`. Reproduced on a fresh Monitor test. | FAIL | `SX/monitor-rgb-edited.png` and `RS/monitor-rgb-slider-dismiss-repeat.png`. |

### Pass 5 coverage and teardown

**10 PASS, 4 FAIL and 0 BLOCKED** across the table's 14 checks. The
editor dropdown foreground fixes passed, including the selected rows
in the two dropdowns with separate interaction or placement failures.
No product code changed.

Activating an editor from the other app restored its window size. A
coordinate based on the maximized size opened ShareX's Select image
dialog. It was closed without selecting a file, and the Cursor check
used verified 1600×900 clients in both apps. ShareXrs's Pin options
opened behind the always-on-top test pattern. The test brought only
that temporary options window above the pattern. The first ShareXrs
pin alpha click landed at 127. The repeat clicked one pixel higher,
read 128 in Components and saved the matching value. The editor's exit
confirmation was dismissed without saving the generated image.

Six pattern preflights reached the 0.99 threshold before the Pin to
screen and Monitor test launches. The screen-level preflight files were
deleted without viewing. The 33 retained images used PrintWindow on
PID-verified app windows and have verified SHA-256 and DPI sidecars.
The missing `shots/sharex/tools-ui/in/photo.png` and the missing
`tools-ui/in/photo.png` used by `scripts/image-tools-compare.sh` were
generated from `scripts/image-tools-inputs.py` in scratch and copied
to both new paths. The PNG SHA-256 is
`3fd46090f3913e4a73c648903f18333addaa00516e924a2552cec492916deea1`.

Both apps honored `-ExitShareX` after every dialog closed. The pass
compared the saved colour fields after exit, then restored and verified
all 13 original ShareX profile hashes. The original executable restarted
without arguments as **PID 5920**, with a temporary
`AutoCheckUpdate=false` value for startup. After dismissing its usual
hotkey warning, the original config bytes were restored and the main
window was the only visible ShareX window. Twelve files still matched
their hashes. The monthly log kept all its original bytes and appended
2,663 bytes of normal startup logging.

The test processes, scratch profiles and profile backup were removed.
The clipboard reported zero formats. The display and work-area bounds
matched the pre-test measurements. The offline build and macOS Clippy
check completed.

## Fixes for pass 5 (branch `fix/live-checks-5`)

Fixed headlessly with unit tests, `--render` (a new `--render image-editor
--state option:<tool>:<item>` opens an option's dropdown) and Avalonia's
decompiled sources (`ListBoxItem`, `ItemSelectionEventTriggers`, `Popup`,
`LightDismissOverlayLayer`, `ContextMenu`, `PopupFlyoutBase`,
`PopupPositionerExtensions`). Pass 5 ran on `a5e4b3d`, before this branch,
so every row needs a live re-check on a build of it.

| ID | Cause | Fix | Re-check |
|---|---|---|---|
| HIL2-02b | The palette took a click only when it changed the colour, and drew its selection from the colour. The fill's default `#00FFFFFF` is the palette's `Transparent`, so the click changed nothing and the cell showed only its hover outline. | `6a706ff`: Avalonia's `ListBoxItem` selects on a mouse button's press (`ItemSelectionEventTriggers`: left or right, `PointerPressed` for a mouse), and the selection is its own state. The pressed cell is now selected on the press, whether or not the colour changes, until another cell or colour is chosen. A test presses `Transparent` on the fill's default and moves away. | Rectangle's Fill colour: the Transparent cell's press selects it, and the outline stays with the pointer elsewhere in the popup. Also: another cell moves the selection and the colour. |
| REG4-02f | `OnPopupOpened` sets the text and posts `Focus()` and `SelectAll()`. The port asked for the focus after drawing the box, so it showed a caret. On a popup's first showing the request also fell in egui's sizing pass, an invisible, disabled frame whose focus request is dropped. | `d96fa4b`: the box is focused and fully selected on the popup's first drawn frame. The font family search box takes its focus the same way. From the same control: the text is centred (`HorizontalContentAlignment="Center"`), and Enter applies `TryParseFontSize` (optional `pt`, 1 to 1000, one decimal). Enter never applied before: egui's single-line edit gives up the focus on Enter. An invalid entry goes back to the size, selected. | Text's font size: `48` opens selected and centred, and typing replaces it. Also: `36 pt` and Enter applies 36, `abc` and Enter shows `48` selected again. |
| REG4-02g | The option dropdowns placed themselves in the window, and egui's `Area` kept them there. The cursor list slid up over the toolbar and its last rows were cut off. ShareX opens each `Popup` (`Placement="Bottom"`, `VerticalOffset="4"`) in its own window on the work area. | `c2b8ca9`: the editor's option and colour dropdowns go through `sharex_ui::popup::position`: anchor and gravity `Bottom`, then flip, slide and resize. They register their layer, so the host moves one past the window into a popup window, and their area isn't constrained. The colour flyouts (`color_view::popup`) already targeted the work area but were still constrained to the window; they no longer are. `--render image-editor --state option:Cursor:CursorType --scale 1.25 --popup-host -384,-267.2,1664,964.8 --client-origin 480,334` puts the list in a popup window at screen y 397, 198×1143 px, ending on the work area at 1540. ShareX's is at 398, 197×1142. | The restored 1600×900 editor at 125%, Cursor (K), the Default dropdown: every row through Wait cursor in a popup below the client. Also: a colour flyout opened near a window's bottom edge goes past it. |
| REG5-04b | A press outside a popup closed it and also reached the control under it. Avalonia's light-dismiss `Popup` puts a `LightDismissOverlayLayer` over the owner window. The dismissing press lands on it and goes no further unless `OverlayDismissEventPassThrough` is set, which ShareX never sets. Nothing under the overlay is `:pointerover`. | `5ff3bb9`: `sharex_ui::popup::light_dismiss_overlay` puts an invisible click widget at the bottom of the popup's own layer. egui's hit test then leaves out the layers below, and the popup's own widgets stay on top. A popup's own button's press now closes it, without toggling it back open. Pin options passes its real `just_opened`. See the sweep below. | Monitor test, RGB `#0078D7`, Color open, a click on the red slider: the flyout closes, red stays 0. Also: a combo box's or editor dropdown's own button closes it; a click on another editor tool button only closes the open dropdown. |

Sweep of ShareX's popups for REG5-04b. These take the press and now
swallow it:

- `Flyout`s and `ColorPickerDropdown`: every `color_view` popup (Monitor
  test, task settings, mouse highlighter, pin options, image effects, the
  accent colour, image tools' `ColorPicker`).
- Fluent's `ComboBox` (`PART_Popup`).
- The editor's option dropdowns and colour dropdowns, in the editor and in
  region capture.
- `ZoomPickerDropdown`: the editor's canvas also swallows the press that
  closes it.
- The image effects' gradient `Flyout`.

`ContextMenu` sets `OverlayDismissEventPassThrough = true`, so the press
reaches what's under it. ShareXrs already passes it through for the text
box variable menus, history, hotkey and task settings menus. Two
differences were found and left for a live check:

- **The main window's sidebar menus.** They are `ContextMenu`s
  (`TryOpenContextMenu`), but `main.rs` swallows the dismissing press and
  its release. In ShareX the press should reach the sidebar button, so
  another section's button opens its menu at once, and the same button
  closes then reopens on the release. The tool grid is a `Flyout` and
  swallows.
- **The editor's File menu.** It is a `MenuFlyout`, so it should swallow
  the press, but `menu::Flyout` has no overlay yet.

Regression: every compare script that runs headlessly (menus, editor
popups, dialogs and effects, data, image, image-finish, screen and system
tools, scrolling; dark and light where the script has both) ran with
master's binary (`cba5884`) and this branch's: 277 compared states. Six
changed, all editor popups, and all moved closer to ShareX:

- stroke style: dark 0.26% → 0.15%, light 0.24% → 0.15%;
- font size: dark and light 0.11% → 0.08% (the selected, centred `48`);
- cursor type: dark 1.17% → 0.73%, light 1.65% → 1.24%.

The stroke style and cursor type gains come from the positioner's
placement. The menu and image tools scripts ran from copies that write
to a temp folder, not into `sharexrs-ref`. Video tools weren't run:
their FFmpeg states write beside their inputs and use no popup code.
