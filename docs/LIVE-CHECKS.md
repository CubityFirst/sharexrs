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

## App windows

Checks from [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent).

| ID | Area | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|
| APP-01 | App windows | File collision with Ask. Save the custom region `320,260,320,240` twice using the constant filename `capture-check`. Try Cancel, Escape, Use new name, Use unique name, Overwrite and Enter on the unchanged name. | Cancel and Escape dismissed the dialog without a new file. New name wrote `another-name.png`, unique wrote `capture-check (2).png`, and Overwrite and Enter both replaced the generated sentinel at `capture-check.png`. | Same decisions, filenames and decoded pixels. | PASS | `SX/app-file-exists-dark.png`, `RS/app-file-exists-dark.png`. All six output files passed the pattern check at 1.0000. Decoded RGB SHA-256 `0aa571ac63e04af1df72866484eabb850f5865049e6ef24868a8599388b9ae3f` in both apps. |
| APP-02 | App windows | Start with `ShowIconPackPrompt=true` and `ShowStartScreen=false`. Pick Legacy, inspect the main window, then Continue. | Prompt opened over the main window. Legacy applied before Continue. | Same prompt, live icon change and persistence. | PASS | `SX/app-icon-prompt-dark.png`, `RS/app-icon-prompt-dark.png`, and `app-icon-legacy-live-dark.png` in both directories. Both JSON files saved `ThemeOptions.IconPack="Legacy"` and `ShowIconPackPrompt=false`. |
| APP-03 | App windows | About window, update status and links. | Skipped. About starts an update request even with `AutoCheckUpdate=false` unless the registry policy is active. | Skipped. The equivalent update and browser routes require network. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |
| APP-05a | App windows | Save a local capture with BottomRight toast placement, 8 s duration and 1 s fade. | Showed a saved-image toast. It was gone after the timed wait. | Same appearance of a toast on completion and timeout. | PASS | `SX/app-toast-task-dark.png`, `RS/app-toast-task-dark.png`. |
| APP-06 | App windows | Close the main window to the tray twice with `FirstTimeMinimizeToTray=true`. Reopen with `-OpenMainWindow` between closes. | First close displayed a toast. Second close hid the main window with no toast. | Same behavior with identical toast client geometry, `605,626,428,103`. | PASS | `SX/app-tray-tip-dark.png`, `RS/app-tray-tip-dark.png`. Both saved `FirstTimeMinimizeToTray=false`. |
| APP-07a | App windows | Capture the generated region with Show after capture window and Save image to file. Inspect the initial dialog and cancel. | Opened the dialog. Escape returned with no `after-check.png`. | Same dialog flow and no output. | PASS | `SX/app-after-task-repeat-dark.png`, `RS/app-after-task-repeat-dark.png`. |
| APP-08a | App windows | Open `-ImageViewer pattern-03.png` with `pattern-04.png` in the same generated folder. Check fullscreen and press Right once, then Escape. | Opened fullscreen at 1024×768. Right advanced from pattern-03 to pattern-04. Escape closed the window. | Opened fullscreen and closed on Escape. Right left pattern-03 displayed. | FAIL | `app-viewer-initial-dark.png` and `app-viewer-next-dark.png` under both `SX` and `RS`. The centre pixel after Right was RGB `120,90,170` in ShareX and `54,150,110` in ShareXrs. Generate the folder with `scripts/history-fixture.py`. |
| APP-12a | App windows | Enable Show quick task menu with only local Save image to file. Run `-CustomRegion` over the pattern, then dismiss the popup. | Opened the quick task menu. Dismissal left no `quick-check.png`. | Same open and dismiss behavior, with no output. | PASS | `SX/app-quick-task-repeat-dark.png`, `RS/app-quick-task-repeat-dark.png`. Earlier reference-only capture is superseded. |
| APP-13a | App windows | Run `-ToggleActionsToolbar` with no saved position, start a drag at client `26,19`, move `-160,-80` in 12 steps with `scripts/input.ps1`, then middle-click the title. | Opened flush with the bottom right of the work area. Moved `-160,-80`, saved `611, 600` and closed on middle click. | Opened at the same work-area anchor and closed on middle click. Moved `-147,-73` and saved `614, 607`. | FAIL | `app-toolbar-first-dark.png` and `app-toolbar-dragged-dark.png` under both `SX` and `RS`. Persisted field `ActionsToolbarPosition`. |
| APP-13b | App windows | Drop a file on the Actions toolbar. | Skipped. The drop uploads the file. | Skipped for the same reason. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |

## Video tools

Checks from [TOOLS.md](TOOLS.md#the-video-windows-in-sharexrs) and the Video row of [PORTING.md](PORTING.md).

| ID | Area | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|
| VID-01 | Video tools | Open the same generated silent 8 s, 320×180 H.264 MP4 in Video converter. Choose local output and x264 constant quality 23. Start encoding. | Wrote a 224,247-byte MP4 and displayed result properties. | Same file bytes and result properties. | PASS | `video-converter-result-repeat-dark.png` under `SX` and `RS`. Output SHA-256 `fe855a7e213b5a005032219749974b099d007c09aa67f9c9c9b7f4ca6e146871`. Input generated locally with FFmpeg `testsrc2=size=320x180:rate=24`, `-t 8 -c:v libx264 -pix_fmt yuv420p -an`. |
| VID-02 | Video tools | At 125% DPI open the encoder list in Video converter without resizing its 850×550 logical client. | Separate popup from y=694 to y=1324, 196 px beyond the client bottom. The initial viewport shows MPEG-4 / Xvid, GIF and WebP. | Popup stops at the client bottom, y=1128. The initial viewport ends at VP9. Scroll is needed for the entries ShareX shows immediately. | FAIL | `video-converter-ready-repeat-dark.png` and `video-converter-codecs-popup-repeat-dark.png` under `SX` and `RS`. |
| VID-03 | Video tools | Start `-AnimatedGifTrimmer` on the generated three-frame `sequence.gif`, then on a text file named `invalid.gif`. | Loads at frame 1 of 3 with duration 840 ms. The invalid file shows “Select an animated GIF with at least two frames.” | Same initial frame, duration and invalid-file message. | PASS | `video-gif-loaded-repeat-dark.png` and `video-gif-invalid-repeat-dark.png` under `SX` and `RS`. |
| VID-04 | Video tools | Start `-VideoTrimmer invalid.mp4` with a file containing the ASCII bytes `not a video`. Wait for the error. | Shows a modal and an inline error. In the repeat, the Media Foundation detail reports that the stream has no more samples, with code `0xC00D3E84`. | Shows a modal and an inline error with the generic detail “Select a video with a known, finite duration.” | FAIL | `video-trimmer-invalid-repeat-dark.png` and `video-trimmer-invalid-underlying-repeat-dark.png` under `SX` and `RS`. The earlier claim that only ShareXrs opened a modal is superseded. |
| VID-05 | Video tools | Open `-VideoTrimmer silent.mp4` using VID-01's clip. | Loads a paused first frame and sets Start to 0 and End to 8 s. | Same first frame and times. Both timelines have thumbnails after loading. | PASS | `video-trimmer-loaded-repeat-dark.png` under `SX` and `RS`. |
| VID-06 | Video tools | Upload generated thumbnails. | Skipped. Upload requires network. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-video-windows-in-sharexrs) |

## Data and system tools

Checks from [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) and the Data and System rows of [PORTING.md](PORTING.md).

| ID | Area | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|
| DAT-01 | Data tools | Write `ShareXrs live-check fixture` plus a single LF as UTF-8. Run `-HashCheck hash.txt`, choose default SHA-256 and press Check. | Displayed the expected uppercase SHA-256. | Same value. | PASS | `data-hash-ready-repeat-dark.png` and `data-hash-result-repeat-dark.png` under `SX` and `RS`. SHA-256 `D1CDDE2CABE05501325828167FC6A70DC2B6D1A6CEB417DDA69797F7021F24E0`. Earlier `data-hash-*-dark.png` without `repeat` is superseded. |
| DAT-02 | Data tools | Open `-Metadata metadata.jpg` on the generated JPEG from `phase2-32616/inputs`. Read the total beside Metadata. | Header says `22 tags`. | Header shows the literal placeholder `{0:N0} tags`. The visible category headers and rows still populate. | FAIL | `SX/data-metadata-repeat-dark.png`, `RS/data-metadata-repeat-dark.png`. Earlier `data-metadata-dark.png` is superseded. |
| DAT-03 | Data tools | Run `-StripMetadata` on two separate copies of the DAT-02 input. | Reduced the file from 13,470 to 13,354 bytes. | Wrote exactly the same 13,354 bytes. | PASS | Post-interruption repeat output SHA-256 `234ce2b417ec4f85f0318ffca26ed637849956367b9ce4cf6681316de557f48` in both apps. Earlier strip output hashes are superseded by this repeat. |
| DAT-04b | Data tools | Upload the generated folder index. | Skipped. The upload route needs a provider or loopback network. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |
| DAT-06b | Data tools | Analyze image, Load models and Test connection against a provider or mock. | Skipped. Requires network and a configured endpoint. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |
| SYS-04 | System tools | Network monitor pings, samples, events and log. | Skipped. Opening the window starts network probes. | Skipped for the same reason. | BLOCKED | [TOOLS.md](TOOLS.md#the-data-and-system-windows-in-sharexrs) |
