<#
.SYNOPSIS
  Screenshots one top-level window of a running process, for comparing
  ShareX and ShareXrs side by side.

.EXAMPLE
  scripts/winshot.ps1 -Process ShareX -Title 'ShareX' -Out shots/sharex/main.png -Width 900 -Height 600
  scripts/winshot.ps1 -Process sharexrs -Title 'ShareX' -Out shots/sharexrs/main.png -Width 900 -Height 600

  -Title is a regex matched against visible window titles of that process;
  the first match wins. -Width/-Height (outer size, physical pixels) resize
  the window first so both apps are captured at the same size. -List prints
  the process's visible windows and exits.
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string] $Title = '.',
    [string] $Out,
    [int] $Width = 0,
    [int] $Height = 0,
    [int] $X = 40,
    [int] $Y = 40,
    [switch] $List,
    # The whole visible frame instead of just the client area (the default,
    # which is the window's own pixels without the 1px border DWM draws
    # around borderless windows).
    [switch] $Frame,
    # Which of the matching windows, in z-order from the top (popups such as
    # a menu and its submenu share an empty title).
    [int] $Index = 0
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class WinShot {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hgt, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static List<Tuple<IntPtr,string>> Windows(HashSet<uint> pids) {
        var list = new List<Tuple<IntPtr,string>>();
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pids.Contains(pid) && IsWindowVisible(h)) {
                var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
                list.Add(Tuple.Create(h, sb.ToString()));
            }
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
'@
# Per-monitor-v2 awareness, so sizes and captures are in physical pixels.
[void][WinShot]::SetProcessDpiAwarenessContext([IntPtr](-4))

$pids = [System.Collections.Generic.HashSet[uint32]]::new()
Get-Process -Name $Process -ErrorAction SilentlyContinue | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
if ($pids.Count -eq 0) { throw "no process named $Process" }
$wins = [WinShot]::Windows($pids)
if ($List) { $wins | ForEach-Object { '{0}  {1}' -f $_.Item1, $_.Item2 }; return }
$win = $wins | Where-Object { $_.Item2 -match $Title } | Select-Object -Skip $Index -First 1
if (-not $win) { throw "no visible window of $Process matching '$Title'" }
$h = $win.Item1

[void][WinShot]::ShowWindow($h, 9) # SW_RESTORE
if ($Width -gt 0 -and $Height -gt 0) {
    [void][WinShot]::SetWindowPos($h, [IntPtr]::Zero, $X, $Y, $Width, $Height, 0x0014) # NOZORDER|NOACTIVATE
}
Start-Sleep -Milliseconds 700

# The client area, or with -Frame the visible frame
# (DWMWA_EXTENDED_FRAME_BOUNDS, without the invisible resize borders).
$r = New-Object WinShot+RECT
[void][WinShot]::DwmGetWindowAttribute($h, 9, [ref]$r, 16)
if (-not $Frame) {
    $c = New-Object WinShot+RECT
    [void][WinShot]::GetClientRect($h, [ref]$c)
    $pt = New-Object WinShot+POINT
    [void][WinShot]::ClientToScreen($h, [ref]$pt)
    $r.L = $pt.X; $r.T = $pt.Y; $r.R = $pt.X + $c.R; $r.B = $pt.Y + $c.B
}
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
# PrintWindow with PW_RENDERFULLCONTENT renders the window itself, so other
# windows overlapping it never end up in the shot and focus isn't stolen.
# It draws the whole window rect (with the invisible resize borders), so
# crop to the visible frame afterwards.
$wr = New-Object WinShot+RECT
[void][WinShot]::GetWindowRect($h, [ref]$wr)
$full = New-Object System.Drawing.Bitmap ($wr.R - $wr.L), ($wr.B - $wr.T)
$g = [System.Drawing.Graphics]::FromImage($full)
$hdc = $g.GetHdc()
$ok = [WinShot]::PrintWindow($h, $hdc, 2)
$g.ReleaseHdc($hdc); $g.Dispose()
if (-not $ok) { throw 'PrintWindow failed' }
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.DrawImage($full, (New-Object System.Drawing.Rectangle 0, 0, $w, $hh), ($r.L - $wr.L), ($r.T - $wr.T), $w, $hh, [System.Drawing.GraphicsUnit]::Pixel)
$g.Dispose(); $full.Dispose()
if (-not $Out) { $Out = "$Process.png" }
$path = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path (Get-Location).Path $Out }
New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"{0} '{1}' {2}x{3} -> {4}" -f $Process, $win.Item2, $w, $hh, $Out
