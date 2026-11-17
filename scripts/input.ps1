<#
.SYNOPSIS
  Moves the mouse, clicks or presses a key, for driving menus (which UI
  Automation can't hover). Coordinates are physical screen pixels.

.EXAMPLE
  scripts/input.ps1 -MoveTo 1000,760          # park the cursor out of the way
  scripts/input.ps1 -MoveTo 300,120           # hover a menu entry
  scripts/input.ps1 -Click 300,120
  scripts/input.ps1 -Key Escape
#>
param(
    [int[]] $MoveTo,
    [int[]] $Click,
    [ValidateSet('Escape', 'Enter', 'Up', 'Down', 'Left', 'Right')] [string] $Key
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Input {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
}
'@
[void][Input]::SetProcessDpiAwarenessContext([IntPtr](-4))
# Moves the cursor as a mouse would: SetCursorPos alone doesn't always get a
# WM_MOUSEMOVE to the window before a following click, so the app would see
# the press at the old position. A relative zero move (MOUSEEVENTF_MOVE)
# does generate one.
function Move-To([int] $x, [int] $y) {
    [void][Input]::SetCursorPos($x + 1, $y + 1)
    [Input]::mouse_event(0x1, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [void][Input]::SetCursorPos($x, $y)
    [Input]::mouse_event(0x1, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
}
if ($MoveTo) {
    Move-To $MoveTo[0] $MoveTo[1]
}
if ($Click) {
    Move-To $Click[0] $Click[1]
    [Input]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
    [Input]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
}
if ($Key) {
    $vk = @{ Escape = 0x1B; Enter = 0x0D; Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28 }[$Key]
    [Input]::keybd_event([byte]$vk, 0, 0, [IntPtr]::Zero)
    [Input]::keybd_event([byte]$vk, 0, 2, [IntPtr]::Zero)
}
