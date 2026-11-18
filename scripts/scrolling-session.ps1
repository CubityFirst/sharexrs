<#
.SYNOPSIS
  One safe session of Capture › Scrolling capture or Auto capture in ShareX
  or ShareXrs. Covers every monitor with `sharexrs --test-pattern` and puts
  its scrollable page (`--scroll X,Y,W,H`) on top, waits until the pattern
  is up (both tools grab the screen), opens the tool, runs the steps and
  always ends by closing the pattern. Screen grabs (`shot:`) are kept only
  if they're mostly the pattern; window shots (`win:`) use PrintWindow on
  the app's own window.

  ShareX's tool opens through its Capture menu (UI Automation); ShareXrs's
  through its command line (`sharexrs -<HotkeyType>`, which the running
  ShareXrs takes over).

.EXAMPLE
  scripts/scrolling-session.ps1 -Process ShareX -Entry 'Scrolling capture...' -Steps 'sleep:800','click:500,300','sleep:12000','win:Scrolling capture=G:/x.png'
  scripts/scrolling-session.ps1 -Process sharexrs -Job ScrollingCapture -Steps ...

  Steps: move:X,Y  click:X,Y  key:Chord  wheel:N  sleep:MS  shot:PATH
  win:TitleRegex=PATH  clip:PATH  invoke:TitleRegex=Name (UI Automation)
  cli:Job  wait:TitleRegex (until such a window of the app shows, 30 s)
#>
param(
    [Parameter(Mandatory)] [ValidateSet('ShareX', 'sharexrs')] [string] $Process,
    [string] $Entry,
    [string] $Job,
    [switch] $NoOpen,
    [string[]] $Steps = @(),
    [int[]] $At = @(512, 384),
    [int[]] $Scroll = @(300, 120, 420, 480),
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    [string] $ShareX = 'G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe',
    [string] $Pattern = "$env:TEMP/sharexrs-pattern/sharexrs.exe",
    [int] $PatternSeconds = 120
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Pattern)) { $Pattern = $Sharexrs }
$patternArgs = @('--test-pattern', '--seconds', "$PatternSeconds", '--scroll', ($Scroll -join ','))
$patternProc = Start-Process -FilePath $Pattern -ArgumentList $patternArgs -PassThru
$patternId = $patternProc.Id
$appName = if ($Process -eq 'ShareX') { 'ShareX' } else { 'sharexrs' }
function XY([string] $v) { $p = $v -split ','; return @([int]$p[0], [int]$p[1]) }
try {
    & "$PSScriptRoot/wait-pattern.ps1" -Min 0.6
    Start-Sleep -Milliseconds 400
    & "$PSScriptRoot/input.ps1" -MoveTo $At
    if (-not $NoOpen) {
        if ($Process -eq 'sharexrs') {
            $fwd = Start-Process -FilePath $Sharexrs -ArgumentList @("-$Job") -PassThru; $fwd | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
        } else {
            & "$PSScriptRoot/sharex-menu.ps1" -Process ShareX -Menu Capture -Entry $Entry | Out-Null
            & "$PSScriptRoot/input.ps1" -MoveTo $At
        }
    }
    foreach ($s in $Steps) {
        $name, $arg = $s -split ':', 2
        switch ($name) {
            'move' { & "$PSScriptRoot/input.ps1" -MoveTo (XY $arg) }
            'click' { & "$PSScriptRoot/input.ps1" -Click (XY $arg) }
            'wheel' { & "$PSScriptRoot/input.ps1" -Wheel ([int]$arg) }
            'key' { & "$PSScriptRoot/input.ps1" -Chord $arg }
            'sleep' { Start-Sleep -Milliseconds ([int]$arg) }
            'shot' {
                Start-Sleep -Milliseconds 300
                & "$PSScriptRoot/screen-shot.ps1" -Out $arg | Out-Null
                python "$PSScriptRoot/pattern-check.py" $arg --min 0.3 --delete
            }
            'win' {
                $t, $out = $arg -split '=', 2
                & "$PSScriptRoot/winshot.ps1" -Process $appName -Title $t -Out $out | Out-Null
            }
            'clip' { & "$PSScriptRoot/clipboard-dump.ps1" -Out $arg }
            'invoke' {
                $t, $n = $arg -split '=', 2
                & "$PSScriptRoot/uia.ps1" -Process $appName -Window $t -Invoke $n | Out-Null
            }
            'cli' {
                $exe = if ($Process -eq 'ShareX') { $ShareX } else { $Sharexrs }
                $fwd = Start-Process -FilePath $exe -ArgumentList @("-$arg") -PassThru
                $fwd | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
            }
            'wait' {
                $deadline = (Get-Date).AddSeconds(30)
                while ((Get-Date) -lt $deadline) {
                    $l = & "$PSScriptRoot/winshot.ps1" -Process $appName -List
                    if ($l -match $arg) { break }
                    Start-Sleep -Milliseconds 300
                }
            }
            default { throw "unknown step $s" }
        }
        Start-Sleep -Milliseconds 150
    }
}
finally {
    Start-Sleep -Milliseconds 300
    Stop-Process -Id $patternId -ErrorAction SilentlyContinue
}
