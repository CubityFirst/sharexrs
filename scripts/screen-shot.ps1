<#
.SYNOPSIS
  Saves what's on one monitor (physical pixels, read from the screen), for
  capturing capture overlays. Only use it while a test pattern
  (`sharexrs --test-pattern`) covers the monitor, so no other app's
  content ends up in the file.

.EXAMPLE
  scripts/screen-shot.ps1 -Out G:/scripts/sharexrs-ref/shots/sharex/region-rest.png
  scripts/screen-shot.ps1 -Monitor 1 -Out shots/x.png
#>
param(
    [Parameter(Mandatory)] [string] $Out,
    [int] $Monitor = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class ScreenShotDpi { [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v); }
'@
[void][ScreenShotDpi]::SetProcessDpiAwarenessContext([IntPtr](-4))
$b = ([System.Windows.Forms.Screen]::AllScreens)[$Monitor].Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
$g.Dispose()
$path = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path (Get-Location).Path $Out }
New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
'monitor {0}: {1}x{2} at {3},{4} -> {5}' -f $Monitor, $b.Width, $b.Height, $b.X, $b.Y, $Out
