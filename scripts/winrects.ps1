<#
.SYNOPSIS
  Prints the screen rectangles (physical pixels) of a process's visible
  top-level windows: the window rect, DWM's visible frame and the client
  area, plus the extended style. For finding where a popup (a menu or
  flyout, which Avalonia opens as its own window) sits relative to its owner.

.EXAMPLE
  scripts/winrects.ps1 -Process ShareX
#>
param([Parameter(Mandatory)] [string] $Process)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class WinRects {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static List<IntPtr> Windows(HashSet<uint> pids) {
        var list = new List<IntPtr>();
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pids.Contains(pid) && IsWindowVisible(h)) list.Add(h);
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
'@
[void][WinRects]::SetProcessDpiAwarenessContext([IntPtr](-4))
$pids = [System.Collections.Generic.HashSet[uint32]]::new()
Get-Process -Name $Process | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
foreach ($h in [WinRects]::Windows($pids)) {
    $sb = New-Object System.Text.StringBuilder 256
    [void][WinRects]::GetWindowTextW($h, $sb, 256)
    $w = New-Object WinRects+RECT; [void][WinRects]::GetWindowRect($h, [ref]$w)
    $f = New-Object WinRects+RECT; [void][WinRects]::DwmGetWindowAttribute($h, 9, [ref]$f, 16)
    $c = New-Object WinRects+RECT; [void][WinRects]::GetClientRect($h, [ref]$c)
    $p = New-Object WinRects+POINT; [void][WinRects]::ClientToScreen($h, [ref]$p)
    $ex = [WinRects]::GetWindowLong($h, -20)
    '{0} "{1}" window={2},{3} {4}x{5} frame={6},{7} {8}x{9} client={10},{11} {12}x{13} exstyle=0x{14:X}' -f `
        $h, $sb, $w.L, $w.T, ($w.R - $w.L), ($w.B - $w.T), $f.L, $f.T, ($f.R - $f.L), ($f.B - $f.T), `
        $p.X, $p.Y, $c.R, $c.B, $ex
}
