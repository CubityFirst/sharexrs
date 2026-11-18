<#
.SYNOPSIS
  Runs an Images tool on the synthetic inputs in a running ShareX or
  ShareXrs, the same way in both (each window sized to ShareX's client
  size, the same client-coordinate clicks, the same file dialog answers),
  and moves what it wrote into -Out, for comparing the two apps' outputs.

.EXAMPLE
  scripts/image-tools-behaviour.ps1 -Process ShareX -Tool thumbnailer -Out G:\scripts\sharexrs-ref\tools-ui\out\sharex
  scripts/image-tools-behaviour.ps1 -Process sharexrs -Tool all -Out G:\scripts\sharexrs-ref\tools-ui\out\rs

  ShareXrs's main window must be visible (its Tools grid is clicked);
  ShareX's Tools menu is driven through UI Automation.
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Tool,
    [Parameter(Mandatory)] [string] $Out,
    [string] $In = 'G:\Scripts\sharexrs-ref\tools-ui\in'
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/uia-lib.ps1"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class BWin {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hgt, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
New-Item -ItemType Directory -Force $Out | Out-Null
$isRs = $Process -eq 'sharexrs'

# ShareXrs's Tools grid entries (client coordinates of the main window's grid).
$grid = @{
    'Icon converter...' = 483, 167; 'Image combiner...' = 300, 201; 'Image comparer...' = 489, 201
    'Image converter...' = 678, 201; 'Image effects...' = 480, 235; 'Image resizer...' = 669, 235
    'Image splitter...' = 292, 269; 'Image thumbnailer...' = 496, 269; 'Image watermark...' = 303, 303
}

function Handle($w) { [IntPtr]$w.Current.NativeWindowHandle }

function Open-Tool([string] $Entry, [string] $Title) {
    if ($isRs) {
        $m = Get-UiaWindow sharexrs '^ShareXrs 22'
        $h = Handle $m
        & "$PSScriptRoot/raise.ps1" -Process sharexrs -Title '^ShareXrs 22' | Out-Null
        Start-Sleep -Milliseconds 500
        $o = New-Object BWin+POINT
        [void][BWin]::ClientToScreen($h, [ref]$o)
        & "$PSScriptRoot/input.ps1" -Click ($o.X + 57), ($o.Y + 150)
        Start-Sleep -Milliseconds 1200
        $c = $grid[$Entry]
        & "$PSScriptRoot/input.ps1" -Click ($o.X + $c[0]), ($o.Y + $c[1])
        Start-Sleep -Milliseconds 600
        # raise.ps1 leaves it topmost.
        [void][BWin]::SetWindowPos($h, [IntPtr](-2), 0, 0, 0, 0, 0x13)
    } else {
        & "$PSScriptRoot/sharex-menu.ps1" -Process ShareX -Menu Tools -Entry $Entry | Out-Null
    }
    Start-Sleep -Milliseconds 1200
    return Get-UiaWindow $Process $Title
}

# Sizes the window's client area to w x h at 40,40 and focuses it.
function Size-Window($w, [int] $cw, [int] $ch) {
    $h = Handle $w
    $wr = New-Object BWin+RECT; $cr = New-Object BWin+RECT
    [void][BWin]::GetWindowRect($h, [ref]$wr); [void][BWin]::GetClientRect($h, [ref]$cr)
    $fw = ($wr.R - $wr.L) - ($cr.R - $cr.L); $fh = ($wr.B - $wr.T) - ($cr.B - $cr.T)
    # Topmost (as raise.ps1), so the other app's main window can't take the clicks.
    [void][BWin]::SetWindowPos($h, [IntPtr](-1), 40, 10, $cw + $fw, $ch + $fh, 0x0040)
    [void][BWin]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 600
}

function Click-Client($w, [int] $x, [int] $y) {
    $o = New-Object BWin+POINT
    [void][BWin]::ClientToScreen((Handle $w), [ref]$o)
    & "$PSScriptRoot/input.ps1" -Click ($o.X + $x), ($o.Y + $y)
    Start-Sleep -Milliseconds 500
}

function Answer([string[]] $Files) {
    & "$PSScriptRoot/file-dialog.ps1" -Process $Process -Files $Files | Out-Null
    Start-Sleep -Milliseconds 1500
}

function Close-Tool([string] $Title) {
    & "$PSScriptRoot/close-window.ps1" -Process $Process -Title $Title | Out-Null
    Start-Sleep -Milliseconds 500
}

# What a tool wrote into the inputs' folder (names matching the regex) goes
# to -Out. (Not -Filter: Win32's `?` also matches no character.)
function Collect([string] $Regex) {
    Start-Sleep -Milliseconds 1500
    Get-ChildItem $In | Where-Object { $_.Name -match $Regex } | Move-Item -Destination $Out -Force
}

$three = "$In\photo.png", "$In\landscape.png", "$In\portrait.png"
$tools = if ($Tool -eq 'all') { 'thumbnailer', 'splitter', 'resizer', 'converter', 'watermark', 'icon', 'combiner' } else { @($Tool) }
foreach ($t in $tools) {
    switch ($t) {
        'thumbnailer' {
            $w = Open-Tool 'Image thumbnailer...' 'Image Thumbnailer'; Size-Window $w 900 590
            Click-Client $w 65 49; Answer $three
            Click-Client $w 803 554
            Collect '_th\.jpg$'; Close-Tool 'Image Thumbnailer'
        }
        'splitter' {
            $w = Open-Tool 'Image splitter...' 'Image Splitter'; Size-Window $w 680 372
            Click-Client $w 612 49; Answer @("$In\photo.png")
            Click-Client $w 232 267; Click-Client $w 232 267; Click-Client $w 597 267
            Click-Client $w 604 336
            Collect '^photo\d+\.png$'; Close-Tool 'Image Splitter'
        }
        'resizer' {
            $w = Open-Tool 'Image resizer...' 'Image resizer'; Size-Window $w 1028 650
            Click-Client $w 65 49; Answer $three
            Click-Client $w 953 614
            Collect '_resized\.png$'; Close-Tool 'Image resizer'
        }
        'converter' {
            $w = Open-Tool 'Image converter...' 'Image converter'; Size-Window $w 1028 650
            Click-Client $w 65 49; Answer $three
            Click-Client $w 945 614
            Collect '_converted\.png$'; Close-Tool 'Image converter'
        }
        'watermark' {
            $w = Open-Tool 'Image watermark...' 'Image watermark'; Size-Window $w 1028 700
            Click-Client $w 65 49; Answer $three
            Click-Client $w 941 664
            Collect '_watermarked\.png$'; Close-Tool 'Image watermark'
        }
        'icon' {
            $w = Open-Tool 'Icon converter...' 'Icon Converter'; Size-Window $w 860 570
            Click-Client $w 791 49; Answer @("$In\alpha.png")
            Click-Client $w 795 534; Answer @("$Out\alpha.ico")
            Close-Tool 'Icon Converter'
        }
        'combiner' {
            $w = Open-Tool 'Image combiner...' 'Image Combiner'; Size-Window $w 980 650
            Click-Client $w 65 49; Answer $three
            Click-Client $w 894 614
            Start-Sleep -Milliseconds 2500
            Close-Tool 'Image Combiner'
        }
        default { throw "no tool $t" }
    }
}
