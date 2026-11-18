<#
.SYNOPSIS
  Opens the image editor of the reference ShareX (or a portable ShareXrs)
  through its command line (`-ImageEditor [file]`), waits for its window,
  raises it and prints its client origin, for the editor dialogs' shots
  (scripts/editor-shots.ps1).
.EXAMPLE
  scripts/editor-open.ps1 -Process ShareX -Image target/edtest/test-640x400.png
  scripts/editor-open.ps1 -Process ShareX            # the start screen
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string] $Image,
    [string] $Exe
)
$ErrorActionPreference = 'Stop'
if (-not $Exe) {
    $Exe = if ($Process -eq 'ShareX') { 'G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe' } else { "$PSScriptRoot/../target/debug/sharexrs.exe" }
}
$args = @('-ImageEditor')
if ($Image) { $args += '"' + (Resolve-Path $Image).Path + '"' }
Start-Process -FilePath $Exe -ArgumentList $args -Wait
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 250
    $list = & "$PSScriptRoot/winshot.ps1" -Process $Process -List
    if ($list -match 'Image Editor') { break }
}
Start-Sleep -Milliseconds 800
& "$PSScriptRoot/raise.ps1" -Process $Process -Title 'Image Editor'
