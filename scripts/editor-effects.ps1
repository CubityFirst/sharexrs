<#
.SYNOPSIS
  Drives the reference ShareX's image editor effect browser (the editor
  already open, e.g. with scripts/editor-open.ps1): opens the browser with
  its Image Effects button (UI Automation), types a search, optionally clicks
  the first result (which opens its dialog in the right sidebar) and takes a
  shot of the window. ShareXrs's states are rendered instead
  (`--render image-editor --state effects...`, scripts/compare-editor-effects.sh).
.EXAMPLE
  scripts/editor-effects.ps1 -Search 'colorize' -Open -Out target/shots/fx/sx-colorize-dark.png
  scripts/editor-effects.ps1 -Out target/shots/fx/sx-browser-dark.png     # the browser, no search
#>
param(
    [string] $Process = 'ShareX',
    [string] $Search = '',
    [switch] $Open,
    [string] $Out,
    # Where the first result sits (physical pixels) in the 1028x720 window
    # at 0,0 (the maximized editor on the 1024x768 monitor).
    [int[]] $First = @(180, 254)
)
$ErrorActionPreference = 'Stop'
& "$PSScriptRoot/uia.ps1" -Process $Process -Window 'Image Editor' -InvokeMatch '^Image Effects' | Out-Null
# `FocusSearchBox` focuses the cleared search box 50 ms later.
Start-Sleep -Milliseconds 700
if ($Search) {
    & "$PSScriptRoot/input.ps1" -Text $Search
    Start-Sleep -Milliseconds 600
}
if ($Open) {
    & "$PSScriptRoot/input.ps1" -Click $First
    Start-Sleep -Milliseconds 1200
}
& "$PSScriptRoot/input.ps1" -MoveTo 1010, 740
Start-Sleep -Milliseconds 400
if ($Out) {
    & "$PSScriptRoot/winshot.ps1" -Process $Process -Title 'Image Editor' -Out $Out
}
