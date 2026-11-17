<#
.SYNOPSIS
  One short, safe region capture session with annotation steps, in ShareX
  or ShareXrs: covers the monitors with `sharexrs --test-pattern` first,
  opens Capture > Region (as region-session.ps1 does), runs the steps,
  and always ends with Tab (back to the region tool, dropping any
  annotation selection) and Escape, and closes the pattern. Grabs that aren't mostly the pattern are deleted.

  Steps (in order):
    move:X,Y        move the pointer
    click:X,Y       left click
    rclick:X,Y      right click
    dclick:X,Y      double click
    down:X,Y        press the left button
    to:X,Y          move with the button held (12 steps)
    up              release the left button
    key:NAME        a key: a letter, Tab, Escape, Enter, Delete, Left...
    ctrl:NAME       Ctrl + a key; shift:NAME likewise
    hold:NAME / release:NAME   a modifier (Shift, Ctrl) down / up
    type:TEXT       types TEXT
    wait:MS
    shot:PATH       grabs the screen

.EXAMPLE
  scripts/annotate-session.ps1 -Process ShareX -Steps 'key:R','down:200,150','to:500,400','up','shot:G:/x.png'
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [int[]] $At = @(300, 300),
    [string[]] $Steps = @(),
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    [int] $WaitMs = 1500,
    # Don't press Escape at the end (the steps captured, closing the overlay).
    [switch] $NoEscape,
    # Open the menu with clicks on the raised main window, not UI Automation.
    [switch] $Physical,
    # Also accept the pattern under black at this alpha (a spotlight).
    [int] $Dark = -1
)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class AInput {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
    [DllImport("user32.dll")] public static extern short VkKeyScan(char c);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    public struct POINT { public int X; public int Y; }
}
"@
[void][AInput]::SetProcessDpiAwarenessContext([IntPtr](-4))
$named = @{ Escape = 0x1B; Enter = 0x0D; Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28; Space = 0x20; Tab = 0x09;
    Delete = 0x2E; Back = 0x08; Home = 0x24; End = 0x23; Shift = 0x10; Ctrl = 0x11; Alt = 0x12 }
function Vk([string] $name) {
    if ($named.ContainsKey($name)) { return $named[$name] }
    if ($name.Length -eq 1) { return [byte][char]$name.ToUpper() }
    throw "unknown key $name"
}
function Press([int] $vk) {
    [AInput]::keybd_event([byte]$vk, 0, 0, [IntPtr]::Zero)
    [AInput]::keybd_event([byte]$vk, 0, 2, [IntPtr]::Zero)
}
function MoveTo([int] $x, [int] $y) {
    [void][AInput]::SetCursorPos($x, $y)
    [AInput]::mouse_event(0x1, 0, 0, 0, [IntPtr]::Zero)
}
function Xy([string] $v) { $p = $v -split ','; return @([int]$p[0], [int]$p[1]) }

# The overlay's window (by handle: ShareX keeps a hidden one around too).
function Overlays {
    @(& "$PSScriptRoot/winrects.ps1" -Process $Process 2>$null | Where-Object { $_ -match '"Region capture"' } |
        ForEach-Object { ($_ -split ' ')[0] })
}
$before = Overlays
$patternArgs = @('--test-pattern', '--seconds', '90')
$pattern = Start-Process -FilePath $Sharexrs -ArgumentList $patternArgs -PassThru
$held = @()
$buttonDown = $false
try {
    # Only once the pattern covers the screen: an overlay freezes what's on it.
    & "$PSScriptRoot/wait-pattern.ps1"
    if ($Process -eq 'sharexrs' -or $Physical) {
        $title = if ($Process -eq 'sharexrs') { '^ShareXrs [0-9]' } else { '^ShareX [0-9]' }
        $origin = (& "$PSScriptRoot/raise.ps1" -Process $Process -Title $title) -split ','
        $ox = [int]$origin[0]; $oy = [int]$origin[1]
        & "$PSScriptRoot/input.ps1" -Click ($ox + 60), ($oy + 60)
        Start-Sleep -Milliseconds 400
        & "$PSScriptRoot/input.ps1" -Click ($ox + 300), ($oy + 150)
        & "$PSScriptRoot/input.ps1" -MoveTo $At
    } else {
        & "$PSScriptRoot/input.ps1" -MoveTo $At
        & "$PSScriptRoot/sharex-menu.ps1" -Process $Process -Menu Capture -Entry "Region..."
    }
    Start-Sleep -Milliseconds $WaitMs
    # Above the pattern: a background process's new topmost window can open under it.
    [void](& "$PSScriptRoot/raise.ps1" -Process $Process -Title '^Region capture$')
    foreach ($step in $Steps) {
        $verb, $arg = $step -split ':', 2
        switch ($verb) {
            'move' { $p = Xy $arg; MoveTo $p[0] $p[1] }
            'click' { $p = Xy $arg; MoveTo $p[0] $p[1]; Start-Sleep -Milliseconds 50
                [AInput]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); [AInput]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero) }
            'rclick' { $p = Xy $arg; MoveTo $p[0] $p[1]; Start-Sleep -Milliseconds 50
                [AInput]::mouse_event(0x8, 0, 0, 0, [IntPtr]::Zero); [AInput]::mouse_event(0x10, 0, 0, 0, [IntPtr]::Zero) }
            'dclick' { $p = Xy $arg; MoveTo $p[0] $p[1]; Start-Sleep -Milliseconds 50
                [AInput]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); [AInput]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
                Start-Sleep -Milliseconds 60
                [AInput]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); [AInput]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero) }
            'down' { $p = Xy $arg; MoveTo $p[0] $p[1]; Start-Sleep -Milliseconds 50
                [AInput]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); $buttonDown = $true; Start-Sleep -Milliseconds 60 }
            'to' {
                $p = Xy $arg
                $from = New-Object AInput+POINT
                [void][AInput]::GetCursorPos([ref]$from)
                for ($i = 1; $i -le 12; $i++) {
                    MoveTo ([int]($from.X + ($p[0] - $from.X) * $i / 12)) ([int]($from.Y + ($p[1] - $from.Y) * $i / 12))
                    Start-Sleep -Milliseconds 25
                }
                Start-Sleep -Milliseconds 80
            }
            'up' { [AInput]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); $buttonDown = $false }
            'key' { Press (Vk $arg) }
            'ctrl' { [AInput]::keybd_event(0x11, 0, 0, [IntPtr]::Zero); Press (Vk $arg); [AInput]::keybd_event(0x11, 0, 2, [IntPtr]::Zero) }
            'shift' { [AInput]::keybd_event(0x10, 0, 0, [IntPtr]::Zero); Press (Vk $arg); [AInput]::keybd_event(0x10, 0, 2, [IntPtr]::Zero) }
            'hold' { $vk = Vk $arg; [AInput]::keybd_event([byte]$vk, 0, 0, [IntPtr]::Zero); $held += $vk }
            'release' { $vk = Vk $arg; [AInput]::keybd_event([byte]$vk, 0, 2, [IntPtr]::Zero); $held = @($held | Where-Object { $_ -ne $vk }) }
            'type' {
                foreach ($c in $arg.ToCharArray()) {
                    $s = [AInput]::VkKeyScan($c)
                    $vk = $s -band 0xFF
                    $shifted = ($s -band 0x100) -ne 0
                    if ($shifted) { [AInput]::keybd_event(0x10, 0, 0, [IntPtr]::Zero) }
                    Press $vk
                    if ($shifted) { [AInput]::keybd_event(0x10, 0, 2, [IntPtr]::Zero) }
                    Start-Sleep -Milliseconds 20
                }
            }
            'wait' { Start-Sleep -Milliseconds ([int]$arg) }
            'stats' {
                $f = Join-Path ([System.IO.Path]::GetTempPath()) 'stats-grab.png'
                & "$PSScriptRoot/screen-shot.ps1" -Out $f | Out-Null
                try {
                python -W ignore -c "from PIL import Image; im=Image.open(r'$f').convert('RGB'); px=list(im.getdata()); n=len(px); print('black', sum(1 for p in px if max(p)<16)/n, 'mean', tuple(sum(c[i] for c in px)//n for i in range(3)), 'toolbar', im.getpixel((70,30)))"
                } finally { Remove-Item $f -ErrorAction SilentlyContinue }
            }
            'windows' { & "$PSScriptRoot/zorder.ps1" }
            'shot' {
                Start-Sleep -Milliseconds 300
                & "$PSScriptRoot/screen-shot.ps1" -Out $arg
                $extra = if ($Dark -ge 0) { @('--dark', $Dark) } else { @() }
                python "$PSScriptRoot/pattern-check.py" $arg --min 0.5 --delete @extra
            }
            default { throw "unknown step $step" }
        }
        Start-Sleep -Milliseconds 120
    }
}
finally {
    foreach ($vk in $held) { [AInput]::keybd_event([byte]$vk, 0, 2, [IntPtr]::Zero) }
    if (-not $NoEscape) {
        # Escape before the button goes up, so a held drag captures nothing;
        # only while the overlay is open (Escape hides a main window).
        Press 0x09; Start-Sleep -Milliseconds 200
        for ($i = 0; $i -lt 4; $i++) {
            $open = @(Overlays | Where-Object { $before -notcontains $_ })
            if ($open.Count -eq 0) { break }
            Press 0x1B; Start-Sleep -Milliseconds 400
        }
    }
    if ($buttonDown) { [AInput]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero) }
    Start-Sleep -Milliseconds 400
    Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue
}





