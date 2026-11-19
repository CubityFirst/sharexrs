# Windows live checks

ShareX reference `c4f1e6f7780f5e1a2f04aadd6703557806e9f868` and ShareXrs
`72045af`, both in portable mode. Input files and settings were generated
locally. Before a capture or overlay, the desktop backdrop was checked
with `scripts/pattern-check.py`. Evidence from an individual app window is
client-area PrintWindow and has a JSON sidecar. The tables use `SX` for
`G:/scripts/sharexrs-ref/shots/sharex/live-checks/` and `RS` for
`G:/scripts/sharexrs-ref/shots/sharexrs/live-checks/`.

## App windows

Checks from [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent).

| ID | Area | Check | ShareX result | ShareXrs result | PASS/FAIL/BLOCKED | Evidence |
|---|---|---|---|---|---|---|
| APP-01 | App windows | File collision with Ask. Save the custom region `320,260,320,240` twice using the constant filename `capture-check`. Try Cancel, Escape, Use new name, Use unique name, Overwrite and Enter on the unchanged name. | Cancel and Escape dismissed the dialog without a new file. New name wrote `another-name.png`, unique wrote `capture-check (2).png`, and Overwrite and Enter both replaced the generated sentinel at `capture-check.png`. | Same decisions, filenames and decoded pixels. | PASS | `SX/app-file-exists-dark.png`, `RS/app-file-exists-dark.png`. All six output files passed the pattern check at 1.0000. Decoded RGB SHA-256 `0aa571ac63e04af1df72866484eabb850f5865049e6ef24868a8599388b9ae3f` in both apps. |
| APP-02 | App windows | Start with `ShowIconPackPrompt=true` and `ShowStartScreen=false`. Pick Legacy, inspect the main window, then Continue. | Prompt opened over the main window. Legacy applied before Continue. | Same prompt, live icon change and persistence. | PASS | `SX/app-icon-prompt-dark.png`, `RS/app-icon-prompt-dark.png`, and `app-icon-legacy-live-dark.png` in both directories. Both JSON files saved `ThemeOptions.IconPack="Legacy"` and `ShowIconPackPrompt=false`. |
| APP-03 | App windows | About window, update status and links. | Skipped. About starts an update request even with `AutoCheckUpdate=false` unless the registry policy is active. | Skipped. The equivalent update and browser routes require network. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |
| APP-05a | App windows | Save a local capture with BottomRight toast placement, 8 s duration and 1 s fade. | Showed a saved-image toast. It was gone after the timed wait. | Same appearance of a toast on completion and timeout. | PASS | `SX/app-toast-task-dark.png`, `RS/app-toast-task-dark.png`. |
| APP-06 | App windows | Close the main window to the tray twice with `FirstTimeMinimizeToTray=true`. Reopen with `-OpenMainWindow` between closes. | First close displayed a toast. Second close hid the main window with no toast. | Same behavior with identical toast client geometry, `605,626,428,103`. | PASS | `SX/app-tray-tip-dark.png`, `RS/app-tray-tip-dark.png`. Both saved `FirstTimeMinimizeToTray=false`. |
| APP-07a | App windows | Capture the generated region with Show after capture window and Save image to file. Inspect the initial dialog and cancel. | Opened the dialog. Cancel returned with no `after-check.png`. | Opened the dialog. Escape returned with no `after-check.png`. | PASS | `SX/app-after-task-dark.png`, `RS/app-after-task-dark.png`. |
| APP-08a | App windows | Open `-ImageViewer pattern-03.png` with `pattern-04.png` in the same generated folder. Check fullscreen and press Right once, then Escape. | Opened fullscreen at 1024×768. Right advanced from pattern-03 to pattern-04. Escape closed the window. | Opened fullscreen and closed on Escape. Right left pattern-03 displayed. | FAIL | `app-viewer-initial-dark.png` and `app-viewer-next-dark.png` under both `SX` and `RS`. The centre pixel after Right was RGB `120,90,170` in ShareX and `54,150,110` in ShareXrs. Generate the folder with `scripts/history-fixture.py`. |
| APP-13a | App windows | Run `-ToggleActionsToolbar` with no saved position, start a drag at client `26,19`, move `-160,-80` in 12 steps with `scripts/input.ps1`, then middle-click the title. | Opened flush with the bottom right of the work area. Moved `-160,-80`, saved `611, 600` and closed on middle click. | Opened at the same work-area anchor and closed on middle click. Moved `-147,-73` and saved `614, 607`. | FAIL | `app-toolbar-first-dark.png` and `app-toolbar-dragged-dark.png` under both `SX` and `RS`. Persisted field `ActionsToolbarPosition`. |
| APP-13b | App windows | Drop a file on the Actions toolbar. | Skipped. The drop uploads the file. | Skipped for the same reason. | BLOCKED | [APP-WINDOWS.md](APP-WINDOWS.md#live-checks-for-the-desktop-agent) |
