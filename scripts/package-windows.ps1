<#
.SYNOPSIS
Packs a Windows build of ShareXrs as the release's two Windows files, as
ShareX.Setup does for ShareX: a portable zip and an Inno Setup installer.

.DESCRIPTION
  scripts/package-windows.ps1 -Exe <sharexrs.exe> -Payload <dir> -Version <ver>
      -Platform x64|arm64 -Out <dir> [-SkipInstaller] [-Iscc <ISCC.exe>]
      [-NativeMessagingHost <sharexrs-native-messaging.exe>]

-Payload is scripts/fetch-bundled.py's output for the platform (ffmpeg.exe,
onnxruntime.dll, Models, Licenses). Writes, in -Out:

  ShareXrs-<ver>-portable-<platform>.zip   the folder below plus an empty
                                           "Portable" file (ShareX's marker:
                                           settings go in ShareXrs\ beside the exe)
  ShareXrs-<ver>-setup-<platform>.exe      packaging/windows/ShareXrs-setup.iss

The folder (ShareX.Setup's CreateFolder, minus what ShareXrs doesn't have):

  sharexrs.exe, sharexrs-native-messaging.exe, host-manifest-chrome.json,
  host-manifest-firefox.json, ffmpeg.exe, onnxruntime.dll, Models\..., Licenses\*.txt

-NativeMessagingHost defaults to sharexrs-native-messaging.exe beside -Exe; the
manifests come from packaging\windows.

The file names end as ShareX's do (portable-x64.zip, setup-x64.exe), which is
what ShareX's update checker looks for in a GitHub release.

Runs in Windows PowerShell 5.1 and PowerShell 7.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Exe,
    [string] $NativeMessagingHost,
    [Parameter(Mandatory)] [string] $Payload,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [ValidateSet('x64', 'arm64')] [string] $Platform,
    [Parameter(Mandatory)] [string] $Out,
    [switch] $SkipInstaller,
    [string] $Iscc,
    # Where the folders are put together (kept, to look at); a temporary one by default.
    [string] $StageDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$root = Split-Path -Parent $PSScriptRoot
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Exe = (Resolve-Path $Exe).Path
if (-not $NativeMessagingHost) { $NativeMessagingHost = Join-Path (Split-Path -Parent $Exe) 'sharexrs-native-messaging.exe' }
$NativeMessagingHost = (Resolve-Path $NativeMessagingHost).Path
$Payload = (Resolve-Path $Payload).Path

# The folder every package is made from.
$stage = if ($StageDir) { $StageDir } else { Join-Path ([System.IO.Path]::GetTempPath()) "sharexrs-package-$Platform" }
$folder = Join-Path $stage 'ShareXrs'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item $Exe (Join-Path $folder 'sharexrs.exe')
# ShareX.NativeMessagingHost and its host manifests (ShareX's *.exe and *.json).
Copy-Item $NativeMessagingHost (Join-Path $folder 'sharexrs-native-messaging.exe')
Copy-Item (Join-Path $root 'packaging\windows\host-manifest-chrome.json') $folder
Copy-Item (Join-Path $root 'packaging\windows\host-manifest-firefox.json') $folder
Copy-Item (Join-Path $Payload '*') $folder -Recurse

$required = @(
    'sharexrs.exe', 'sharexrs-native-messaging.exe', 'host-manifest-chrome.json', 'host-manifest-firefox.json',
    'ffmpeg.exe', 'onnxruntime.dll',
    'Models\PP-OCRv6_small_det\inference.onnx', 'Models\PP-OCRv6_small_det\inference.yml',
    'Models\PP-OCRv6_small_rec\inference.onnx', 'Models\PP-OCRv6_small_rec\inference.yml',
    'Licenses\ShareXrs_license.txt', 'Licenses\FFmpeg_license.txt', 'Licenses\FFmpeg_source.txt',
    'Licenses\OnnxRuntime_license.txt', 'Licenses\PaddleOCR_license.txt'
)
foreach ($f in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $folder $f) -PathType Leaf)) {
        throw "Missing from the package: $f"
    }
}

# The portable zip: the folder's contents at the zip's root (ShareX's
# ZipManager.Compress, no base directory) and the Portable marker.
$portable = Join-Path $stage 'portable'
Copy-Item $folder $portable -Recurse
New-Item -ItemType File (Join-Path $portable 'Portable') | Out-Null
$zip = Join-Path $Out "ShareXrs-$Version-portable-$Platform.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
# Entry by entry, named with "/": ZipFile.CreateFromDirectory on Windows
# PowerShell 5.1's .NET Framework names them with "\", which other unzips
# take as part of the file name.
$archive = [System.IO.Compression.ZipFile]::Open($zip, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($file in Get-ChildItem -LiteralPath $portable -Recurse -File | Sort-Object FullName) {
        $name = $file.FullName.Substring($portable.Length + 1).Replace('\', '/')
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, $name, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally {
    $archive.Dispose()
}
Write-Host "Portable zip: $zip"

if ($SkipInstaller) {
    if (-not $StageDir) { Remove-Item -Recurse -Force $stage }
    return
}

if (-not $Iscc) {
    $candidates = @(
        $env:ISCC,
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) }
    $Iscc = @($candidates)[0]
    if (-not $Iscc) {
        $command = Get-Command iscc -ErrorAction SilentlyContinue
        if ($command) { $Iscc = $command.Source }
    }
}
if (-not $Iscc) { throw 'Inno Setup 6 (ISCC.exe) not found: install it, pass -Iscc, or use -SkipInstaller' }

# VersionInfoVersion takes numbers only: 22.5.0-rc.1 -> 22.5.0.
$numeric = ($Version -split '-')[0]
$iss = Join-Path $root 'packaging\windows\ShareXrs-setup.iss'
& $Iscc /Q "/DPlatform=$Platform" "/DMyAppVersion=$Version" "/DMyAppVersionNumeric=$numeric" `
    "/DSourceDir=$folder" "/DOutputDir=$Out" "/DRootDir=$root" $iss
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }
Write-Host "Installer: $(Join-Path $Out "ShareXrs-$Version-setup-$Platform.exe")"
if (-not $StageDir) { Remove-Item -Recurse -Force $stage }
