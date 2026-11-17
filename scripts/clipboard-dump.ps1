<#
.SYNOPSIS
  Lists the clipboard's formats and saves its image ones, for checking
  that ShareX and ShareXrs copy the same: "PNG" to <Out>.png and CF_DIB to
  <Out>.dib (the raw bytes) and <Out>-dib.png (decoded).

.EXAMPLE
  scripts/clipboard-dump.ps1 -Out shots/clip-sharex
#>
param([Parameter(Mandatory)] [string] $Out)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$data = [System.Windows.Forms.Clipboard]::GetDataObject()
if (-not $data) { throw 'the clipboard is empty' }
'formats: ' + (($data.GetFormats($false)) -join ', ')
$dir = Split-Path -Parent $Out
if ($dir) { New-Item -ItemType Directory -Force $dir | Out-Null }
$png = $data.GetData('PNG', $false)
if ($png -is [System.IO.MemoryStream]) {
    [System.IO.File]::WriteAllBytes("$Out.png", $png.ToArray())
    "PNG: $($png.Length) bytes -> $Out.png"
}
$dib = $data.GetData([System.Windows.Forms.DataFormats]::Dib, $false)
if ($dib -is [System.IO.MemoryStream]) {
    $bytes = $dib.ToArray()
    [System.IO.File]::WriteAllBytes("$Out.dib", $bytes)
    $w = [BitConverter]::ToInt32($bytes, 4); $h = [BitConverter]::ToInt32($bytes, 8)
    $bpp = [BitConverter]::ToInt16($bytes, 14); $size = [BitConverter]::ToInt32($bytes, 20)
    $xppm = [BitConverter]::ToInt32($bytes, 24)
    "CF_DIB: $($bytes.Length) bytes, header $([BitConverter]::ToInt32($bytes, 0)), ${w}x${h}, $bpp bpp, image size $size, x ppm $xppm -> $Out.dib"
    # A BMP file around it, decoded to PNG.
    $file = New-Object byte[] (14 + $bytes.Length)
    $file[0] = 0x42; $file[1] = 0x4D
    [BitConverter]::GetBytes([int](14 + $bytes.Length)).CopyTo($file, 2)
    $offset = 14 + [BitConverter]::ToInt32($bytes, 0)
    [BitConverter]::GetBytes([int]$offset).CopyTo($file, 10)
    $bytes.CopyTo($file, 14)
    $bmp = New-Object System.Drawing.Bitmap (New-Object System.IO.MemoryStream (, $file))
    $bmp.Save("$Out-dib.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
