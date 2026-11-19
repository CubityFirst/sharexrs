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
