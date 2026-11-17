<#
.SYNOPSIS
  Puts a process's window above every other, topmost ones included (above
  `sharexrs --test-pattern`), so its menus can be clicked with input.ps1.
  Prints the window's client area origin (physical pixels).

.EXAMPLE
  scripts/raise.ps1 -Process sharexrs -Title '^ShareXrs [0-9]'
#>
param(
    [string] $Process,
    # A process id instead of a name (another ShareXrs may be running).
    [int] $Id,
    [string] $Title = '.'
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Raise {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hgt, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    public static IntPtr Find(HashSet<uint> pids, string title) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (found == IntPtr.Zero && pids.Contains(pid) && IsWindowVisible(h)) {
                var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
                if (sb.Length > 0 && System.Text.RegularExpressions.Regex.IsMatch(sb.ToString(), title)) found = h;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
[void][Raise]::SetProcessDpiAwarenessContext([IntPtr](-4))
$pids = [System.Collections.Generic.HashSet[uint32]]::new()
if ($Id) { [void]$pids.Add([uint32]$Id) } else { Get-Process -Name $Process | ForEach-Object { [void]$pids.Add([uint32]$_.Id) } }
$h = [Raise]::Find($pids, $Title)
if ($h -eq [IntPtr]::Zero) { throw "no window of $Process matching '$Title'" }
# HWND_TOPMOST, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW
[void][Raise]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43)
[void][Raise]::SetForegroundWindow($h)
$p = New-Object Raise+POINT
[void][Raise]::ClientToScreen($h, [ref]$p)
'{0},{1}' -f $p.X, $p.Y
