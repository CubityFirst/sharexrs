<#
.SYNOPSIS
  Waits until `sharexrs --test-pattern` covers the screen: grabs the screen
  (to a temporary file, deleted at once) until pattern-check.py finds it is
  the pattern. A capture overlay must not open before that, or it freezes
  whatever else is on screen. Throws after -Seconds.
#>
param([int] $Seconds = 20, [double] $Min = 0.9)
$ErrorActionPreference = 'Stop'
$f = Join-Path ([System.IO.Path]::GetTempPath()) 'wait-pattern-grab.png'
$deadline = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    try {
        & "$PSScriptRoot/screen-shot.ps1" -Out $f | Out-Null
        $out = python "$PSScriptRoot/pattern-check.py" $f --min $Min 2>$null
    } finally {
        Remove-Item $f -ErrorAction SilentlyContinue
    }
    $share = [double](($out | Select-String '([0-9.]+) of sampled').Matches[0].Groups[1].Value)
    if ($share -ge $Min) { return }
}
throw "the test pattern didn't cover the screen within $Seconds s"
