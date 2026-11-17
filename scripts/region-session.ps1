<#
.SYNOPSIS
  One short, safe region capture session in ShareX or ShareXrs, for
  reference shots and side-by-side checks: covers the monitors with
  `sharexrs --test-pattern` first (so the frozen screen is only the
  pattern), opens Capture > Region from the app's sidebar menu, runs the
  given steps, grabs the screen, and always ends the overlay with Escape
  and closes the pattern. A grab that isn't mostly the pattern is deleted.

.EXAMPLE
  # Overlay at rest, pointer at 300,300:
  scripts/region-session.ps1 -Process ShareX -At 300,300 -Out G:/scripts/sharexrs-ref/shots/sharex/region-rest.png
  # Mid-drag from 200,150 to 600,450:
  scripts/region-session.ps1 -Process ShareX -At 200,150 -DragTo 600,450 -Hold -Out .../region-drag.png
  # Capture a region (released, so the app saves and copies it):
  scripts/region-session.ps1 -Process ShareX -At 200,150 -DragTo 600,450
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    # Where the pointer is when the overlay opens.
    [int[]] $At = @(300, 300),
    # Press at -At and drag here.
    [int[]] $DragTo,
    # Keep the button pressed for the grab (mid-drag).
    [switch] $Hold,
    # Then move the pointer here (hover) before the grab.
    [int[]] $MoveTo,
    # A smaller test window on top of the pattern (X,Y,W,H), to snap to.
    [int[]] $Window,
    [string] $Out,
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    [int] $WaitMs = 1500
)
$ErrorActionPreference = 'Stop'
$patternArgs = @('--test-pattern', '--seconds', '60')
if ($Window) { $patternArgs += @('--window', ($Window -join ',')) }
$pattern = Start-Process -FilePath $Sharexrs -ArgumentList $patternArgs -PassThru
try {
    Start-Sleep -Seconds 2
    & "$PSScriptRoot/input.ps1" -MoveTo $At
    & "$PSScriptRoot/sharex-menu.ps1" -Process $Process -Menu Capture -Entry "Region..."
    Start-Sleep -Milliseconds $WaitMs
    if ($DragTo) {
        & "$PSScriptRoot/input.ps1" -Down $At -DragTo $DragTo -Hold:$Hold
        Start-Sleep -Milliseconds 300
    }
    if ($MoveTo) {
        & "$PSScriptRoot/input.ps1" -MoveTo $MoveTo
        Start-Sleep -Milliseconds 300
    }
    if ($Out) {
        & "$PSScriptRoot/screen-shot.ps1" -Out $Out
        python "$PSScriptRoot/pattern-check.py" $Out --min 0.5 --delete
    }
}
finally {
    # Ends the overlay unless a finished capture has closed it; mid-drag
    # it's cancelled before the button goes up, so nothing is captured.
    if (-not $DragTo -or $Hold) { & "$PSScriptRoot/input.ps1" -Key Escape; Start-Sleep -Milliseconds 200 }
    if ($Hold) { & "$PSScriptRoot/input.ps1" -Up }
    Start-Sleep -Milliseconds 400
    Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue
}
