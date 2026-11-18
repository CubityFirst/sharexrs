<#
.SYNOPSIS
  Applies image effects in a running image editor (the reference ShareX's or
  a portable ShareXrs's, 1028x720 client area on the 1024x768 monitor), one at a time: opens the browser, searches,
  opens the first result, clicks Apply (dialog effects), copies the image
  with the bottom toolbar's Copy, dumps the clipboard to <Out>/<id>
  (scripts/clipboard-dump.ps1) and undoes. The same input in both apps;
  compare the dumps with scripts/png-diff.py.
.EXAMPLE
  scripts/effects-side-by-side.ps1 -Process ShareX -Out target/shots/fx/sbs-sharex
  scripts/effects-side-by-side.ps1 -Process sharexrs -Out target/shots/fx/sbs-sharexrs
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string[]] $Only
)
$ErrorActionPreference = 'Stop'
# id, search, Apply's y (0: applied at once), extra clicks before Apply.
$effects = @(
    @('gaussian_blur', 'gaussian', 191, @()),
    @('gaussian_blur_r', 'gaussian', 191, @(@(900, 120))),
    @('colorize', 'colorize', 263, @()),
    @('channel_swap', 'channel swap', 175, @()),
    @('border', 'border', 407, @()),
    @('invert', 'invert', 0, @()),
    @('emboss', 'emboss', 0, @()),
    @('polaroid', 'polaroid', 0, @()),
    @('rotate_90_clockwise', 'turn right', 0, @()),
    @('flip_horizontal', 'flip horizontal', 0, @()),
    @('black_and_white', 'black and white', 0, @()),
    @('heat_map', 'heat map', 0, @()),
    @('draw_checkerboard', 'checkerboard', 319, @()),
    @('pixelate', 'pixelate', 191, @()),
    @('sharpen', 'sharpen', 191, @()),
    @('vignette', 'vignette', 279, @()),
    @('sepia', 'sepia', 191, @()),
    @('rounded_corners', 'rounded corners', 191, @()),
    @('shadow', 'shadow', 537, @()),
    @('posterize', 'posterize', 191, @())
)
New-Item -ItemType Directory -Force $Out | Out-Null
# (.NET's file calls resolve against the process's directory, not this
# location.)
$Out = (Resolve-Path $Out).Path
# Client coordinates from here on: raise.ps1 prints the client origin.
$origin = (& "$PSScriptRoot/raise.ps1" -Process $Process -Title 'Image Editor' | Select-Object -Last 1) -split ','
$ox = [int]$origin[0]; $oy = [int]$origin[1]
# The pointer arrives first: egui decides a click by the widget under the
# pointer, and a synthetic click's move and press can share one frame.
function Click([int]$x, [int]$y) {
    & "$PSScriptRoot/input.ps1" -MoveTo ($x + $ox), ($y + $oy)
    Start-Sleep -Milliseconds 250
    & "$PSScriptRoot/input.ps1" -Click ($x + $ox), ($y + $oy)
}
Start-Sleep -Milliseconds 400
if ($Process -ne 'ShareX') {
    # Scroll the toolbar to its end (Image Effects in view).
    & "$PSScriptRoot/input.ps1" -MoveTo (500 + $ox), (22 + $oy)
    & "$PSScriptRoot/input.ps1" -Wheel -10
    Start-Sleep -Milliseconds 500
}
foreach ($e in $effects) {
    if ($Only -and $Only -notcontains $e[0]) { continue }
    if ($Process -eq 'ShareX') {
        & "$PSScriptRoot/uia.ps1" -Process $Process -Window 'Image Editor' -InvokeMatch '^Image Effects' | Out-Null
    } else {
        Click 1002 22
    }
    Start-Sleep -Milliseconds 700
    & "$PSScriptRoot/input.ps1" -Text $e[1]
    Start-Sleep -Milliseconds 600
    Click 180 254
    Start-Sleep -Milliseconds 2500
    foreach ($c in $e[3]) {
        Click $c[0] $c[1]
        Start-Sleep -Milliseconds 2500
    }
    if ($e[2] -gt 0) {
        & "$PSScriptRoot/winshot.ps1" -Process $Process -Title 'Image Editor' -Out "$Out/$($e[0])-dialog.png" | Out-Null
        Click 952 $e[2]
        Start-Sleep -Milliseconds 2000
    }
    Click 464 694
    Start-Sleep -Milliseconds 1200
    & "$PSScriptRoot/clipboard-dump.ps1" -Out "$Out/$($e[0])" | Select-String 'PNG' | Out-Null
    "$($e[0]) copied"
    & "$PSScriptRoot/input.ps1" -MoveTo (600 + $ox), (620 + $oy)
    & "$PSScriptRoot/input.ps1" -Chord Ctrl+Z
    Start-Sleep -Milliseconds 800
}
& "$PSScriptRoot/input.ps1" -MoveTo 1010, 740
