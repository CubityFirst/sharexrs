<#
.SYNOPSIS
  Captures the History window's states in ShareX (or ShareXrs) over the
  synthetic history (scripts/history-fixture.py), for comparing with
  `sharexrs --render history-*`. Popups (context menus) are captured as
  their own windows with their screen position, then composited onto the
  window shot by scripts/history-composite.py.

.EXAMPLE
  scripts/history-shots.ps1 -Process ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/history -Theme dark
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Theme = 'dark',
    [string] $MainTitle = '^ShareX',
    [string] $Title = 'History \(',
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
    # Every popup (empty title), top first, with its screen rectangle.
    $rects = & "$here/winrects.ps1" -Process $Process | Where-Object { $_ -match '^\d+ "" window=(\d+),(\d+) (\d+)x(\d+)' }
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
    & "$here/uia.ps1" -Process $Process -Window $MainTitle -Invoke 'History...' | Out-Null
    Start-Sleep -Milliseconds 1800
    Shot '_tmp'
    Remove-Item "$Out/_tmp-$Theme.png" -ErrorAction SilentlyContinue
    & "$here/raise.ps1" -Process $Process -Title $Title | Out-Null
    Start-Sleep -Milliseconds 300
}
function Close() { & "$here/close-window.ps1" -Process $Process -Title $Title | Out-Null; Start-Sleep -Milliseconds 500 }
function Invoke-Button($name) { & "$here/uia.ps1" -Process $Process -Window $Title -Invoke $name | Out-Null; Start-Sleep -Milliseconds 700 }
function Key($k) { & "$here/input.ps1" -Key $k | Out-Null; Start-Sleep -Milliseconds 400 }

# The client area's origin once the window is at 4,4 1016x712: 12,35.
$cx = 12; $cy = 35
$row1 = $cy + 120; $row2 = $row1 + 34

Open
if (Want 'history') { Shot 'history' }
if (Want 'advanced') { Invoke-Button 'Advanced'; Shot 'history-advanced'; Invoke-Button 'Advanced' }
if (Want 'menu') {
    & "$here/input.ps1" -RightClick ($cx + 100),$row1 | Out-Null; Start-Sleep -Milliseconds 800
    Shot 'history-menu'; Popups 'history-menu'
    & "$here/input.ps1" -MoveTo ($cx + 148),($row1 + 58) | Out-Null; Start-Sleep -Milliseconds 900
    Popups 'history-menu-copy'
    Key 'Escape'
}
if (Want 'menu2') {
    & "$here/input.ps1" -Click ($cx + 100),$row2 | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -RightClick ($cx + 100),$row2 | Out-Null; Start-Sleep -Milliseconds 800
    Shot 'history-menu2'; Popups 'history-menu2'
    & "$here/input.ps1" -MoveTo ($cx + 148),($row2 + 24) | Out-Null; Start-Sleep -Milliseconds 900
    Popups 'history-menu2-open'
    & "$here/input.ps1" -MoveTo ($cx + 148),($row2 + 58) | Out-Null; Start-Sleep -Milliseconds 900
    Popups 'history-menu2-copy'
    Key 'Escape'
}
if (Want 'edit') {
    & "$here/input.ps1" -RightClick ($cx + 100),$row2 | Out-Null; Start-Sleep -Milliseconds 800
    & "$here/input.ps1" -Click ($cx + 180),($row2 + 161) | Out-Null; Start-Sleep -Milliseconds 700
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Start-Sleep -Milliseconds 300
    Shot 'history-edit'; Key 'Escape'
}
if (Want 'delete') {
    & "$here/input.ps1" -Click ($cx + 100),$row1 | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -RightClick ($cx + 100),$row1 | Out-Null; Start-Sleep -Milliseconds 800
    & "$here/input.ps1" -Click ($cx + 180),($row1 + 229) | Out-Null; Start-Sleep -Milliseconds 700
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Start-Sleep -Milliseconds 300
    Shot 'history-delete'; Key 'Escape'
}
if (Want 'stats') { Invoke-Button 'Stats'; Start-Sleep -Milliseconds 800; Shot 'history-stats'; Key 'Escape' }
if (Want 'settings') { Invoke-Button 'Settings'; Shot 'history-settings'; Key 'Escape' }
if (Want 'import') { Invoke-Button 'Import'; Shot 'history-import'; Key 'Escape' }
if (Want 'columns') {
    & "$here/input.ps1" -RightClick ($cx + 100),($cy + 85) | Out-Null; Start-Sleep -Milliseconds 800
    Shot 'history-columns'; Popups 'history-columns'
    Key 'Escape'
}
if (Want 'favorites') { Invoke-Button 'Favorites'; Start-Sleep -Milliseconds 500; Shot 'history-favorites'; Invoke-Button 'Favorites' }
if (Want 'search') {
    & "$here/input.ps1" -Click ($cx + 120),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -Text 'chrome' | Out-Null; Start-Sleep -Milliseconds 900
    & "$here/input.ps1" -MoveTo 1010,700 | Out-Null; Key 'Escape'
    Start-Sleep -Milliseconds 300
}
Close
if (Want 'search') {
    Open
    & "$here/input.ps1" -Click ($cx + 120),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -Text 'chrome' | Out-Null; Start-Sleep -Milliseconds 1200
    # The suggestions popup closes on a click elsewhere in the window.
    & "$here/input.ps1" -Click ($cx + 870),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 600
    Shot 'history-search'
    Close
    Open
    & "$here/input.ps1" -Click ($cx + 120),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 300
    & "$here/input.ps1" -Text 'zzz' | Out-Null; Start-Sleep -Milliseconds 1200
    & "$here/input.ps1" -Click ($cx + 870),($cy + 28) | Out-Null; Start-Sleep -Milliseconds 600
    Shot 'history-nomatch'
    Close
}
