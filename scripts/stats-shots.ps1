<#
.SYNOPSIS
  Captures the Stats window in ShareX (or ShareXrs) over the synthetic
  history: the top of the page, the bottom (scrolled to the end), and the
  bottom again with the hour chart's Images filter.

.EXAMPLE
  scripts/stats-shots.ps1 -Process ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/history -Theme dark
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Theme = 'dark',
    [string] $MainTitle = '^ShareX',
    [string] $Title = 'Stats$'
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
function Shot($name) {
    & "$here/winshot.ps1" -Process $Process -Title $Title -Out "$Out/$name-$Theme.png" -Width 1016 -Height 712 -X 4 -Y 4 | Out-Null
}
# Parked right of the window (at 4,4 1016x712), so no scroll bar expands.
function Park() { & "$here/input.ps1" -MoveTo 1022,300 | Out-Null; Start-Sleep -Milliseconds 1200 }
& "$here/uia.ps1" -Process $Process -Window $MainTitle -Invoke 'Stats...' | Out-Null
Start-Sleep -Milliseconds 1500
Shot '_tmp'; Remove-Item "$Out/_tmp-$Theme.png" -ErrorAction SilentlyContinue
& "$here/raise.ps1" -Process $Process -Title $Title | Out-Null
Park
Shot 'stats'
& "$here/input.ps1" -MoveTo 500,400 -Wheel -40 | Out-Null
Park
Shot 'stats-bottom'
& "$here/input.ps1" -MoveTo 500,400 -Wheel 40 | Out-Null
# The hour chart's Images filter (client 826,541 at the top of the page).
& "$here/input.ps1" -Click 838,576 | Out-Null
& "$here/input.ps1" -MoveTo 500,400 -Wheel -40 | Out-Null
Park
Shot 'stats-bottom-images'
& "$here/close-window.ps1" -Process $Process -Title $Title | Out-Null
