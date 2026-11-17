<#
.SYNOPSIS
  One short screen recording in ShareX or ShareXrs over the test pattern:
  covers the monitors with `sharexrs --test-pattern` (and waits until a
  grab shows it), starts the recorder with a hotkey, draws the region,
  grabs the recorder bar, presses Record, grabs the frame, optionally
  pauses and resumes, and stops after a few seconds. Every grab that isn't
  mostly the pattern is deleted. Record no audio: both apps' FFmpeg audio
  source must be None.

.EXAMPLE
  # ShareX: its default Shift+Print Screen (Ctrl+Shift+Print Screen for GIF).
  scripts/record-session.ps1 -App ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/rec
  # ShareXrs: give it Ctrl+Alt+F9 (ScreenRecorder) and Ctrl+Alt+F10 (GIF)
  # hotkeys first; the main window stays hidden so only the pattern shows.
  scripts/record-session.ps1 -App ShareXrs -Gif -Pause -Out shots/rec/xrs
#>
param(
    [Parameter(Mandatory)] [ValidateSet('ShareX', 'ShareXrs')] [string] $App,
    [switch] $Gif,
    # Pause for a second after 1.5 s of recording, then resume.
    [switch] $Pause,
    # The region (x, y, width, height); its frame's toolbar goes below it.
    [int[]] $Region = @(300, 200, 320, 240),
    [double] $Seconds = 3,
    [string] $Out,
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    # Leave the frame up after recording (no stop), for more grabs.
    [switch] $NoStop,
    # ShareX only: start from Capture > Screen recording (GIF) instead of
    # the hotkey (taken by another program), and click Record and Stop.
    [switch] $Menu,
    # Bar only: click these points on the recorder bar ("x,y;x,y"), grab
    # after each (-bar-1, -bar-2...), then cancel with Escape (no recording).
    [string] $BarClicks
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class RecKeys { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
function Chord([string[]] $keys) {
    $vk = @{ Ctrl = 0x11; Alt = 0x12; Shift = 0x10; F9 = 0x78; F10 = 0x79; F11 = 0x7A; PrintScreen = 0x2C }
    foreach ($k in $keys) { [RecKeys]::keybd_event([byte]$vk[$k], 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
    $r = @($keys); [array]::Reverse($r)
    foreach ($k in $r) { [RecKeys]::keybd_event([byte]$vk[$k], 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
}
function Grab([string] $name) {
    if (-not $Out) { return }
    $f = "$Out-$name.png"
    & "$PSScriptRoot/screen-shot.ps1" -Out $f | Out-Null
    python "$PSScriptRoot/pattern-check.py" $f --min 0.5 --delete
}
$hotkey = if ($App -eq 'ShareX') {
    if ($Gif) { @('Ctrl', 'Shift', 'PrintScreen') } else { @('Shift', 'PrintScreen') }
} else {
    if ($Gif) { @('Ctrl', 'Alt', 'F10') } else { @('Ctrl', 'Alt', 'F9') }
}
$pattern = Start-Process -FilePath $Sharexrs -ArgumentList @('--test-pattern', '--seconds', '120') -PassThru
try {
    & "$PSScriptRoot/wait-pattern.ps1"
    $x, $y, $w, $h = $Region
    & "$PSScriptRoot/input.ps1" -MoveTo ($x - 40), ($y - 40)
    if ($Menu) {
        $entry = if ($Gif) { 'Screen recording (GIF)...' } else { 'Screen recording...' }
        & "$PSScriptRoot/sharex-menu.ps1" -Process $App -Menu Capture -Entry $entry
        & "$PSScriptRoot/input.ps1" -MoveTo ($x - 40), ($y - 40)
    } else {
        Chord $hotkey
    }
    Start-Sleep -Milliseconds 1800
    & "$PSScriptRoot/input.ps1" -Down $x, $y -DragTo ($x + $w - 1), ($y + $h - 1)
    Start-Sleep -Milliseconds 1500
    & "$PSScriptRoot/input.ps1" -MoveTo 900, 120
    Start-Sleep -Milliseconds 300
    Grab 'bar'
    if ($BarClicks) {
        $i = 0
        foreach ($c in $BarClicks -split ';') {
            $i++
            $cx, $cy = ($c -split ',') | ForEach-Object { [int]$_ }
            & "$PSScriptRoot/input.ps1" -Click $cx, $cy
            & "$PSScriptRoot/input.ps1" -MoveTo 1000, 740
            Start-Sleep -Milliseconds 900
            Grab "bar-$i"
        }
        & "$PSScriptRoot/input.ps1" -Key Escape
        Start-Sleep -Milliseconds 300
        & "$PSScriptRoot/input.ps1" -Key Escape
        Start-Sleep -Milliseconds 500
        return
    }
    # The compact toolbar (277 wide, timer shown) centred 3px under the frame.
    $frameW = $w + 2
    $content = [math]::Max($frameW, 277)
    $tbLeft = ($x - 1) - [math]::Floor(($content - $frameW) / 2) + [math]::Floor(($content - 277) / 2)
    $tbTop = $y - 1 + $h + 2 + 3
    $startAt = @(($tbLeft + 1 + 112 + 20), ($tbTop + 21))
    $pauseAt = @(($tbLeft + 1 + 112 + 41 + 20), ($tbTop + 21))
    if ($Menu) {
        # Record: the bar's red button, found in a grab of the pattern.
        $f = Join-Path ([System.IO.Path]::GetTempPath()) 'record-session-bar.png'
        & "$PSScriptRoot/screen-shot.ps1" -Out $f | Out-Null
        $at = python -c "from PIL import Image;im=Image.open(r'$f').convert('RGB');w,h=im.size;px=im.load();pts=[(x,y) for y in range(h) for x in range(w) if px[x,y]==(0xB8,0x32,0x3F)];print(sum(p[0] for p in pts)//len(pts), sum(p[1] for p in pts)//len(pts))"
        Remove-Item $f -ErrorAction SilentlyContinue
        $rx, $ry = ($at -split ' ') | ForEach-Object { [int]$_ }
        & "$PSScriptRoot/input.ps1" -Click $rx, $ry
        & "$PSScriptRoot/input.ps1" -MoveTo 900, 120
    } else {
        # The same hotkey presses Record on the bar.
        Chord $hotkey
    }
    Start-Sleep -Milliseconds 1500
    Grab 'frame'
    if ($Pause) {
        & "$PSScriptRoot/input.ps1" -Click $pauseAt
        & "$PSScriptRoot/input.ps1" -MoveTo 900, 120
        Start-Sleep -Milliseconds 1200
        Grab 'paused'
        & "$PSScriptRoot/input.ps1" -Click $pauseAt
        & "$PSScriptRoot/input.ps1" -MoveTo 900, 120
        Start-Sleep -Milliseconds 1500
        Grab 'resumed'
    }
    Start-Sleep -Milliseconds ([int]($Seconds * 1000) - 1500)
    if (-not $NoStop) {
        if ($Menu) { & "$PSScriptRoot/input.ps1" -Click $startAt } else { Chord $hotkey }
        # Encoding (GIF's second pass) and the task.
        Start-Sleep -Seconds 4
    }
}
finally {
    if (-not $NoStop) { Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue }
}
