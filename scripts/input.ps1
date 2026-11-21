<#
.SYNOPSIS
  Moves the mouse, clicks or presses a key, for driving menus (which UI
  Automation can't hover). Coordinates are physical screen pixels.

.EXAMPLE
  scripts/input.ps1 -MoveTo 1000,760          # park the cursor out of the way
  scripts/input.ps1 -MoveTo 300,120           # hover a menu entry
  scripts/input.ps1 -Click 300,120
  scripts/input.ps1 -RightClick 1000,750      # a tray icon's menu
  scripts/input.ps1 -MiddleClick 1000,750
  scripts/input.ps1 -DoubleClick 1000,750
  scripts/input.ps1 -Key Escape
  scripts/input.ps1 -Text 'not a url'
  scripts/input.ps1 -Down 200,150 -DragTo 600,450 -Hold   # press, drag, keep pressed
  scripts/input.ps1 -Up                                   # release the left button
  scripts/input.ps1 -Chord Ctrl+C                         # a key with modifiers
  scripts/input.ps1 -Chord Shift+Right
  scripts/input.ps1 -RightDown 500,400                    # press (and later -RightUp) the right button
#>
param(
    [int[]] $MoveTo,
    [int[]] $Click,
    [int[]] $RightClick,
    [int[]] $MiddleClick,
    # Two left clicks, 60 ms apart (within the double-click time).
    [int[]] $DoubleClick,
    # Presses the left button there (after moving to it).
    [int[]] $Down,
    # Moves there in small steps (dragging, when the button is down).
    [int[]] $DragTo,
    # With -Down/-DragTo: keeps the button pressed (for a mid-drag shot).
    [switch] $Hold,
    # Releases the left button where the cursor is.
    [switch] $Up,
    [ValidateSet('Escape', 'Enter', 'Up', 'Down', 'Left', 'Right', 'Space', 'Tab')] [string] $Key,
    # Types this text into the focused window (SendKeys, special characters escaped).
    [string] $Text,
    # Turns the mouse wheel by this many notches where the cursor is (negative scrolls down).
    [int] $Wheel,
    # A key with modifiers held: Ctrl+C, Shift+Right, Alt+Left, H, Delete,
    # Add, Subtract, OemPlus, OemMinus, F1..F12, NumPad0..NumPad9, A..Z, 0..9
    # and -Key's names.
    [string] $Chord,
    # Presses (there) or releases the right or middle button.
    [int[]] $RightDown,
    [switch] $RightUp,
    [int[]] $MiddleDown,
    [switch] $MiddleUp
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
if ($RightClick) {
    Move-To $RightClick[0] $RightClick[1]
    [Input]::mouse_event(0x8, 0, 0, 0, [IntPtr]::Zero)
    [Input]::mouse_event(0x10, 0, 0, 0, [IntPtr]::Zero)
}
if ($MiddleClick) {
    Move-To $MiddleClick[0] $MiddleClick[1]
    [Input]::mouse_event(0x20, 0, 0, 0, [IntPtr]::Zero)
    [Input]::mouse_event(0x40, 0, 0, 0, [IntPtr]::Zero)
}
if ($DoubleClick) {
    Move-To $DoubleClick[0] $DoubleClick[1]
    [Input]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
    [Input]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [Input]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
    [Input]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
}
if ($Down) {
    Move-To $Down[0] $Down[1]
    [Input]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
}
if ($DragTo) {
    Add-Type -AssemblyName System.Windows.Forms
    $from = [System.Windows.Forms.Cursor]::Position
    $steps = 12
    for ($i = 1; $i -le $steps; $i++) {
        $x = [int]($from.X + ($DragTo[0] - $from.X) * $i / $steps)
        $y = [int]($from.Y + ($DragTo[1] - $from.Y) * $i / $steps)
        [void][Input]::SetCursorPos($x, $y)
        [Input]::mouse_event(0x1, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 80
}
if (($Down -or $DragTo) -and -not $Hold) {
    [Input]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
}
if ($Up) {
    [Input]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
}
if ($Key) {
    $vk = @{ Escape = 0x1B; Enter = 0x0D; Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28; Space = 0x20; Tab = 0x09 }[$Key]
    [Input]::keybd_event([byte]$vk, 0, 0, [IntPtr]::Zero)
    [Input]::keybd_event([byte]$vk, 0, 2, [IntPtr]::Zero)
}
if ($Wheel) {
    $notches = [Math]::Abs($Wheel)
    for ($i = 0; $i -lt $notches; $i++) {
        # MOUSEEVENTF_WHEEL, one notch (WHEEL_DELTA 120) each.
        $delta = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int](120 * [Math]::Sign($Wheel))), 0)
        [Input]::mouse_event(0x800, 0, 0, $delta, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
    }
}
if ($RightDown) {
    Move-To $RightDown[0] $RightDown[1]
    [Input]::mouse_event(0x8, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
}
if ($RightUp) {
    [Input]::mouse_event(0x10, 0, 0, 0, [IntPtr]::Zero)
}
if ($MiddleDown) {
    Move-To $MiddleDown[0] $MiddleDown[1]
    [Input]::mouse_event(0x20, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
}
if ($MiddleUp) {
    [Input]::mouse_event(0x40, 0, 0, 0, [IntPtr]::Zero)
}
if ($Chord) {
    $names = @{ Escape = 0x1B; Enter = 0x0D; Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28; Space = 0x20; Tab = 0x09;
        Delete = 0x2E; Add = 0x6B; Subtract = 0x6D; OemPlus = 0xBB; OemMinus = 0xBD; Home = 0x24; End = 0x23 }
    $mods = @{ Ctrl = 0x11; Shift = 0x10; Alt = 0x12 }
    $parts = $Chord -split '\+'
    $held = @()
    foreach ($m in $parts[0..($parts.Count - 2)]) { if ($parts.Count -gt 1) { $held += $mods[$m] } }
    $k = $parts[-1]
    $vk = if ($names.ContainsKey($k)) { $names[$k] } elseif ($k -match '^F([0-9]+)$') { 0x6F + [int]$Matches[1] } elseif ($k -match '^NumPad([0-9])$') { 0x60 + [int]$Matches[1] } else { [int][char]$k.ToUpperInvariant() }
    foreach ($m in $held) { [Input]::keybd_event([byte]$m, 0, 0, [IntPtr]::Zero) }
    # Arrows, Delete, Home and End are extended keys.
    $ext = if (@(0x25, 0x26, 0x27, 0x28, 0x2E, 0x24, 0x23) -contains $vk) { 1 } else { 0 }
    [Input]::keybd_event([byte]$vk, 0, $ext, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [Input]::keybd_event([byte]$vk, 0, 2 -bor $ext, [IntPtr]::Zero)
    [array]::Reverse($held)
    foreach ($m in $held) { [Input]::keybd_event([byte]$m, 0, 2, [IntPtr]::Zero) }
}
if ($Text) {
    Add-Type -AssemblyName System.Windows.Forms
    $escaped = [regex]::Replace($Text, '[+^%~(){}\[\]]', '{$0}')
    [System.Windows.Forms.SendKeys]::SendWait($escaped)
}
