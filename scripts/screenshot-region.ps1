<#
.SYNOPSIS
  Captures what's on screen over one window's client area, popups included:
  for menus and flyouts, which Avalonia opens as separate top-level windows
  that PrintWindow on the owner doesn't see. Unlike winshot.ps1 this reads
  the screen, so make sure nothing else covers the window.

.EXAMPLE
  scripts/screenshot-region.ps1 -Process ShareX -Title 'ShareX 22' -Out shots/main-capture-menu.png
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string] $Title = '.',
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class RegionShot {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static IntPtr Find(HashSet<uint> pids, string title) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (found == IntPtr.Zero && pids.Contains(pid) && IsWindowVisible(h)) {
                var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
                if (System.Text.RegularExpressions.Regex.IsMatch(sb.ToString(), title) && sb.Length > 0) found = h;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
[void][RegionShot]::SetProcessDpiAwarenessContext([IntPtr](-4))
$pids = [System.Collections.Generic.HashSet[uint32]]::new()
Get-Process -Name $Process | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
$h = [RegionShot]::Find($pids, $Title)
if ($h -eq [IntPtr]::Zero) { throw "no window of $Process matching '$Title'" }
$c = New-Object RegionShot+RECT; [void][RegionShot]::GetClientRect($h, [ref]$c)
$p = New-Object RegionShot+POINT; [void][RegionShot]::ClientToScreen($h, [ref]$p)
$bmp = New-Object System.Drawing.Bitmap $c.R, $c.B
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($p.X, $p.Y, 0, 0, $bmp.Size)
$g.Dispose()
$path = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path (Get-Location).Path $Out }
New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
'{0} {1}x{2} at {3},{4} -> {5}' -f $Process, $c.R, $c.B, $p.X, $p.Y, $Out
