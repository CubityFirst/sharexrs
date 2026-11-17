<#
.SYNOPSIS
  Captures the Image history window's states in ShareX (or ShareXrs) over
  the synthetic history (scripts/history-fixture.py), like
  history-shots.ps1.

.EXAMPLE
  scripts/image-history-shots.ps1 -Process ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/history -Theme dark
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Theme = 'dark',
    [string] $MainTitle = '^ShareX',
    [string] $Title = 'Image History \(',
    [string[]] $Only
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
New-Item -ItemType Directory -Force $Out | Out-Null

function Want($name) { -not $Only -or $Only -contains $name }
function Shot($name) {
    & "$here/winshot.ps1" -Process $Process -Title $Title -Out "$Out/$name-$Theme.png" -Width 1016 -Height 712 -X 4 -Y 4 | Out-Null
}
function Popups($name) {
    $rects = & "$here/winrects.ps1" -Process $Process | Where-Object { $_ -match '^\d+ "" window=' }
    $i = 0
    $meta = @()
    foreach ($r in $rects) {
        $null = $r -match 'client=(-?\d+),(-?\d+) (\d+)x(\d+)'
        & "$here/winshot.ps1" -Process $Process -Title '^$' -Index $i -Out "$Out/$name-$Theme.popup$i.png" | Out-Null
        $meta += "$($Matches[1]),$($Matches[2])"
        $i++
    }
    $client = & "$here/winrects.ps1" -Process $Process | Where-Object { $_ -match "^\d+ `"[^`"]*$Title" } | Select-Object -First 1
    $null = $client -match 'client=(-?\d+),(-?\d+)'
    Set-Content "$Out/$name-$Theme.popups.txt" (@("$($Matches[1]),$($Matches[2])") + $meta)
}
function Open() {
    & "$here/uia.ps1" -Process $Process -Window $MainTitle -Invoke 'Image history...' | Out-Null
    Start-Sleep -Milliseconds 1500
    Shot '_tmp'
    Remove-Item "$Out/_tmp-$Theme.png" -ErrorAction SilentlyContinue
    & "$here/raise.ps1" -Process $Process -Title $Title | Out-Null
    # Thumbnails load after the resize.
    Start-Sleep -Milliseconds 2500
}
function Close() { & "$here/close-window.ps1" -Process $Process -Title $Title | Out-Null; Start-Sleep -Milliseconds 500 }
function Invoke-Button($name) { & "$here/uia.ps1" -Process $Process -Window $Title -Invoke $name | Out-Null; Start-Sleep -Milliseconds 700 }
function Key($k) { & "$here/input.ps1" -Key $k | Out-Null; Start-Sleep -Milliseconds 400 }

$cx = 12; $cy = 35
# The first card's middle: the grid starts under the 57px toolbar, 6px in.
$card1 = @(($cx + 144), ($cy + 160))
$card2 = @(($cx + 144 + 268), ($cy + 160))

Open
if (Want 'image-history') { & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Start-Sleep -Milliseconds 300; Shot 'image-history' }
if (Want 'menu') {
    & "$here/input.ps1" -RightClick $card2[0],$card2[1] | Out-Null; Start-Sleep -Milliseconds 800
    Shot 'image-history-menu'; Popups 'image-history-menu'
    Key 'Escape'
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Start-Sleep -Milliseconds 300
    Shot 'image-history-selected'
    & "$here/input.ps1" -MoveTo $card1[0],$card1[1] | Out-Null; Start-Sleep -Milliseconds 400
    Shot 'image-history-hover'
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null
}
if (Want 'settings') { Invoke-Button 'Settings'; Shot 'image-history-settings'; Key 'Escape' }
if (Want 'stats') { Invoke-Button 'Stats'; Start-Sleep -Milliseconds 800; Shot 'image-history-stats'; Key 'Escape' }
if (Want 'delete') {
    & "$here/input.ps1" -RightClick $card2[0],$card2[1] | Out-Null; Start-Sleep -Milliseconds 800
    # "Delete from history..." is the 9th entry (after Open image, Open, Copy, Toggle favorite, Edit tag, Edit item, Rename).
    Popups 'image-history-menu-check'
    Key 'Escape'
}
Close
if (Want 'search') {
    Open
    & "$here/input.ps1" -Click ($cx + 130),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -Text 'zzz' | Out-Null; Start-Sleep -Milliseconds 1200
    & "$here/input.ps1" -Click ($cx + 900),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 600
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Start-Sleep -Milliseconds 300
    Shot 'image-history-nomatch'
    Close
}
