<#
.SYNOPSIS
  Runs the same History window actions in ShareX or ShareXrs (the window
  open at 4,4 1016x712, over the synthetic history): copy URL (Ctrl+C), file
  path (Ctrl+Shift+C) and Markdown linked image from row 2, edit row 2's tag,
  favourite row 3, delete row 4, edit row 1's host. Writes the clipboard
  texts to <Out>/clipboard-<name>.txt; compare History.db afterwards with
  scripts/history-load.py --dump.

.EXAMPLE
  scripts/history-behaviour.ps1 -Process ShareX -Out shots/history -Name sharex -MarkdownY 556
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [Parameter(Mandatory)] [string] $Name,
    # Where Copy › Markdown linked image is (ShareX's submenu runs past the
    # window; ShareXrs's is kept inside it).
    [int] $MarkdownY = 556,
    [string] $Title = 'History \('
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Add-Type -AssemblyName System.Windows.Forms
function Keys($k) { [System.Windows.Forms.SendKeys]::SendWait($k); Start-Sleep -Milliseconds 500 }
function Clip() { Start-Sleep -Milliseconds 400; [System.Windows.Forms.Clipboard]::GetText() }
function Click($x, $y) { & "$here/input.ps1" -Click $x,$y | Out-Null; Start-Sleep -Milliseconds 500 }
function Right($x, $y) { & "$here/input.ps1" -RightClick $x,$y | Out-Null; Start-Sleep -Milliseconds 800 }
& "$here/raise.ps1" -Process $Process -Title $Title | Out-Null
$rows = @{ 1 = 155; 2 = 189; 3 = 223; 4 = 257 }
$texts = @()
Click 112 $rows[2]
Keys '^c'; $texts += "URL: " + (Clip)
Keys '^+c'; $texts += "FilePath: " + (Clip)
Right 112 $rows[2]
& "$here/input.ps1" -MoveTo 160,($rows[2] + 58) | Out-Null; Start-Sleep -Milliseconds 900
Click 560 $MarkdownY
$texts += "MarkdownLinkedImage: " + (Clip)
Set-Content "$Out/clipboard-$Name.txt" $texts
# Edit tag (row 2): the prompt's text is selected; Save.
Right 112 $rows[2]
Click 180 ($rows[2] + 127)
Keys 'tagged'
Click 633 428
# Favourite (row 3).
Right 112 $rows[3]
Click 180 ($rows[3] + 93)
# Delete from history (row 4): Delete, then the prompt's Delete.
Click 112 $rows[4]
Keys '{DELETE}'
Click 629 408
# Edit item (row 1): the host.
Right 112 $rows[1]
Click 180 ($rows[1] + 161)
Click 572 310
Keys '^a'
Keys 'EditedHost'
Click 744 658
& "$here/input.ps1" -MoveTo 1010,700 | Out-Null
Start-Sleep -Milliseconds 500
& "$here/winshot.ps1" -Process $Process -Title $Title -Out "$Out/after-$Name.png" | Out-Null
