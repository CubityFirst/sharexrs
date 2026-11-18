<#
.SYNOPSIS
  One safe session of a Screen tool (screen color picker, measure, ruler,
  mouse highlighter, pin to screen) in ShareX or ShareXrs, for reference
  shots and side-by-side checks. Covers the monitors with
  `sharexrs --test-pattern` first and waits until it does (these tools
  sample, freeze or draw over the real screen), opens the tool, runs the
  steps, grabs the screen only while the pattern is up (a grab that isn't
  mostly the pattern is deleted), and always ends with Escape and closes the
  pattern.

  ShareX's tool opens through its Tools menu (UI Automation); ShareXrs's
  through its command line (`sharexrs -<HotkeyType>`, which the running
  ShareXrs takes over, as ShareX's single instance does).

.EXAMPLE
  scripts/screen-tools-session.ps1 -Process ShareX -Entry 'Screen color picker...' -Steps 'move:300,300','shot:G:/scripts/sharexrs-ref/shots/sharex/screen-picker.png'
  scripts/screen-tools-session.ps1 -Process sharexrs -Job ScreenColorPicker -Steps 'move:300,300','shot:shots/x.png'

  Steps: move:X,Y  click:X,Y  rclick:X,Y  mclick:X,Y  down:X,Y  drag:X,Y
  up  rdown:X,Y  rup  mdown:X,Y  mup  wheel:N  key:Chord (Ctrl+C, H,
  Escape...)  sleep:MS  shot:PATH  clip:PATH (dump the clipboard)
  setclip:PNG (an image on the clipboard)  cli:Job (the app's -Job)
  raise:TitleRegex (lift a non-topmost window above the pattern)
#>
param(
    [Parameter(Mandatory)] [ValidateSet('ShareX', 'sharexrs')] [string] $Process,
    # ShareX: the Tools menu entry.
    [string] $Entry,
    # ShareXrs: the job (`HotkeyType`) on its command line.
    [string] $Job,
    # Opens nothing (a tool already open, or the steps open it).
    [switch] $NoOpen,
    [string[]] $Steps = @(),
    # Where the pointer is before the tool opens.
    [int[]] $At = @(300, 300),
    # Escape at the end (unless the steps ended the tool themselves).
    [switch] $NoEscape,
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    [string] $ShareX = 'G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe',
    [string] $Pattern = "$env:TEMP/sharexrs-pattern/sharexrs.exe",
    [int] $WaitMs = 1200,
    [int] $PatternSeconds = 90,
    # A smaller test window on top of the pattern (X,Y,W,H).
    [int[]] $Window
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Pattern)) { $Pattern = $Sharexrs }
$patternArgs = @('--test-pattern', '--seconds', "$PatternSeconds")
if ($Window) { $patternArgs += @('--window', ($Window -join ',')) }
$patternProc = Start-Process -FilePath $Pattern -ArgumentList $patternArgs -PassThru
$patternId = $patternProc.Id
function XY([string] $v) { $p = $v -split ','; return @([int]$p[0], [int]$p[1]) }
try {
    & "$PSScriptRoot/wait-pattern.ps1"
    & "$PSScriptRoot/input.ps1" -MoveTo $At
    if (-not $NoOpen) {
        if ($Process -eq 'sharexrs') {
            $fwd = Start-Process -FilePath $Sharexrs -ArgumentList @("-$Job") -PassThru; $fwd | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
        } else {
            & "$PSScriptRoot/sharex-menu.ps1" -Process ShareX -Menu Tools -Entry $Entry | Out-Null
            & "$PSScriptRoot/input.ps1" -MoveTo $At
        }
        Start-Sleep -Milliseconds $WaitMs
    }
    foreach ($s in $Steps) {
        $name, $arg = $s -split ':', 2
        switch ($name) {
            'move' { & "$PSScriptRoot/input.ps1" -MoveTo (XY $arg) }
            'click' { & "$PSScriptRoot/input.ps1" -Click (XY $arg) }
            'rclick' { & "$PSScriptRoot/input.ps1" -RightClick (XY $arg) }
            'mclick' { & "$PSScriptRoot/input.ps1" -MiddleClick (XY $arg) }
            'down' { & "$PSScriptRoot/input.ps1" -Down (XY $arg) -Hold }
            'drag' { & "$PSScriptRoot/input.ps1" -DragTo (XY $arg) -Hold }
            'up' { & "$PSScriptRoot/input.ps1" -Up }
            'rdown' { & "$PSScriptRoot/input.ps1" -RightDown (XY $arg) }
            'rup' { & "$PSScriptRoot/input.ps1" -RightUp }
            'mdown' { & "$PSScriptRoot/input.ps1" -MiddleDown (XY $arg) }
            'mup' { & "$PSScriptRoot/input.ps1" -MiddleUp }
            'wheel' { & "$PSScriptRoot/input.ps1" -Wheel ([int]$arg) }
            'key' { & "$PSScriptRoot/input.ps1" -Chord $arg }
            'sleep' { Start-Sleep -Milliseconds ([int]$arg) }
            'shot' {
                Start-Sleep -Milliseconds 300
                & "$PSScriptRoot/screen-shot.ps1" -Out $arg | Out-Null
                python "$PSScriptRoot/pattern-check.py" $arg --min 0.3 --delete
            }
            # A window that isn't topmost (the Measure window) above the
            # pattern: the app's window whose title matches.
            'raise' {
                $name = if ($Process -eq 'ShareX') { 'ShareX' } else { 'sharexrs' }
                & "$PSScriptRoot/raise.ps1" -Process $name -Title $arg | Out-Null
            }
            'clip' { & "$PSScriptRoot/clipboard-dump.ps1" -Out $arg }
            'setclip' {
                Add-Type -AssemblyName System.Windows.Forms, System.Drawing
                $img = [System.Drawing.Image]::FromFile((Resolve-Path $arg))
                [System.Windows.Forms.Clipboard]::SetImage($img)
                $img.Dispose()
            }
            'cli' {
                # The app's own command line (`-<HotkeyType>`), which the
                # running instance takes over.
                $exe = if ($Process -eq 'ShareX') { $ShareX } else { $Sharexrs }
                $fwd = Start-Process -FilePath $exe -ArgumentList @("-$arg") -PassThru
                $fwd | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
            }
            default { throw "unknown step $s" }
        }
        Start-Sleep -Milliseconds 150
    }
}
finally {
    if (-not $NoEscape) { & "$PSScriptRoot/input.ps1" -Key Escape; Start-Sleep -Milliseconds 300 }
    Start-Sleep -Milliseconds 300
    Stop-Process -Id $patternId -ErrorAction SilentlyContinue
}


