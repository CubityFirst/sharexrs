<#
.SYNOPSIS
  One capture from the Capture menu of ShareX or ShareXrs over
  `sharexrs --test-pattern`, safely: the monitors are covered first, the
  pointer is parked at -MoveTo (both apps draw it in), and every new file
  the app saves is checked to be the pattern (pattern-check.py, deleted if
  it isn't). Prints the new files and the clipboard's image formats
  (saved to -Clip). Always closes the pattern.

.EXAMPLE
  scripts/capture-session.ps1 -Process ShareX -Entry Fullscreen -Clip shots/clip-sharex-full
  scripts/capture-session.ps1 -Process sharexrs -Entry Fullscreen -Clip shots/clip-sharexrs-full
  # A window: the pattern's small window on top (by its title).
  scripts/capture-session.ps1 -Process ShareX -Entry Window,'ShareXrs test window' -Window 300,200,400,300
  scripts/capture-session.ps1 -Process ShareX -Entry Monitor,'1. 1024x768'
  # Region (Transparent): drag over the live pattern.
  scripts/capture-session.ps1 -Process ShareX -Entry 'Region (Transparent)' -Drag 200,150,600,450
  scripts/capture-session.ps1 -Process ShareX -Entry 'Last region'
#>
param(
    [Parameter(Mandatory)] [ValidateSet('ShareX', 'sharexrs')] [string] $Process,
    # Capture menu path: Fullscreen | Window,<title> | Monitor,<label> |
    # 'Region (Transparent)' | 'Last region'.
    [Parameter(Mandatory)] [string[]] $Entry,
    [int[]] $Window,
    [int[]] $Drag,
    [int[]] $MoveTo = @(500, 400),
    [string] $Clip,
    # Where the capture's top-left pixel is on screen, for checking it
    # against the pattern.
    [int[]] $Origin = @(0, 0),
    [int] $WaitMs = 1500,
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    # The ShareXrs to drive (another one may be running).
    [int] $SharexrsId
)
$ErrorActionPreference = 'Stop'
$folder = if ($Process -eq 'ShareX') { 'G:/scripts/sharexrs-ref/ShareX-build/ShareX/Screenshots' } else { "$PSScriptRoot/../target/debug/ShareXrs/Screenshots" }
$before = @(Get-ChildItem -Recurse -File $folder -ErrorAction SilentlyContinue | ForEach-Object FullName)

# ShareXrs's Capture menu: each entry's place, from 1.
$order = @{ 'Fullscreen' = 1; 'Window' = 2; 'Monitor' = 3; 'Region' = 4; 'Region (Transparent)' = 5; 'Last region' = 6 }
# ShareX's entry names.
$names = @{ 'Region' = 'Region...'; 'Region (Transparent)' = 'Region (Transparent)...' }

# What Capture > Window lists (`WindowsList.GetVisibleWindowsList`), in order.
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class CapWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static List<string> Titles() {
        var list = new List<string>();
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) return true;
            int cloaked; if (DwmGetWindowAttribute(h, 14, out cloaked, 4) == 0 && cloaked != 0) return true;
            var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
            if (t.Length == 0) return true;
            var c = new StringBuilder(256); GetClassNameW(h, c, 256);
            var cls = c.ToString();
            if (string.Equals(cls, "Progman", StringComparison.OrdinalIgnoreCase) || string.Equals(cls, "Button", StringComparison.OrdinalIgnoreCase)) return true;
            RECT r; GetWindowRect(h, out r);
            if (r.R - r.L <= 0 || r.B - r.T <= 0) return true;
            list.Add(t.ToString());
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
'@

$patternArgs = @('--test-pattern', '--seconds', '60')
if ($Window) { $patternArgs += @('--window', ($Window -join ',')) }
$pattern = Start-Process -FilePath $Sharexrs -ArgumentList $patternArgs -PassThru
try {
    Start-Sleep -Seconds 2
    if ($Process -eq 'sharexrs') {
        # Its menus are drawn in its window, without UI Automation: clicked
        # where they are (items 28px tall, the Capture menu's first at
        # client y 66, a submenu's first 21px below its parent's top).
        # (Not `$origin`: PowerShell names ignore case, and -Origin is a parameter.)
        $client = (& "$PSScriptRoot/raise.ps1" -Id $SharexrsId -Title '^ShareXrs [0-9]') -split ','
        $ox = [int]$client[0]; $oy = [int]$client[1]
        & "$PSScriptRoot/input.ps1" -Click ($ox + 60), ($oy + 60)
        Start-Sleep -Milliseconds 500
        $i = $order[$Entry[0]] - 1
        $y = $oy + 66 + 28 * $i
        if ($Entry.Count -gt 1) {
            & "$PSScriptRoot/input.ps1" -MoveTo ($ox + 300), $y
            Start-Sleep -Milliseconds 800
            $index = if ($Entry[0] -eq 'Window') { [CapWin]::Titles().IndexOf($Entry[1]) } else { [int]($Entry[1] -replace '\..*', '') - 1 }
            if ($index -lt 0) { throw "'$($Entry[1])' isn't listed" }
            & "$PSScriptRoot/input.ps1" -Click ($ox + 580), ($y - 14 + 21 + 28 * $index)
        } else {
            & "$PSScriptRoot/input.ps1" -Click ($ox + 300), $y
        }
        & "$PSScriptRoot/input.ps1" -MoveTo $MoveTo
    } else {
        & "$PSScriptRoot/input.ps1" -MoveTo $MoveTo
        $path = @($Entry | ForEach-Object { if ($names.ContainsKey($_)) { $names[$_] } else { $_ } })
        & "$PSScriptRoot/sharex-menu.ps1" -Process ShareX -Menu Capture -Entry $path
    }
    "$(Get-Date -Format HH:mm:ss.fff) menu entry chosen"
    Start-Sleep -Milliseconds $WaitMs
    if ($Drag) {
        & "$PSScriptRoot/input.ps1" -Down $Drag[0], $Drag[1] -DragTo $Drag[2], $Drag[3]
        & "$PSScriptRoot/input.ps1" -MoveTo $MoveTo
        Start-Sleep -Milliseconds $WaitMs
    }
    $new = @(Get-ChildItem -Recurse -File $folder -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.FullName })
    foreach ($f in $new) {
        "$(Get-Date -Format HH:mm:ss.fff) saved $($f.Name) at $($f.LastWriteTime.ToString('HH:mm:ss.fff'))"
        # Only the pointer may differ from the pattern.
        $check = python "$PSScriptRoot/pattern-check.py" $f.FullName --origin ($Origin -join ',') --min 0.9 --delete
        "$($f.FullName): $check"
        if (-not (Test-Path $f.FullName)) {
            # The app copied what it saved: that goes too.
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.Clipboard]::Clear()
            'clipboard cleared'
            $Clip = $null
        }
    }
    if ($Clip -and $new.Count -gt 0 -and (Test-Path $new[0].FullName)) {
        & "$PSScriptRoot/clipboard-dump.ps1" -Out $Clip
    }
}
finally {
    Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue
    "$(Get-Date -Format HH:mm:ss.fff) pattern closed"
}
