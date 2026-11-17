<#
.SYNOPSIS
  Reference shots of the region capture's annotation tools, one short
  annotate-session.ps1 run per tool (test pattern first): the tool chosen,
  a shape mid-draw and the finished shape selected; text typed in place;
  a step placed. Prints the steps each shot used, for `--render
  region-* --steps` (the same verbs, `;`-separated).
.EXAMPLE
  scripts/annotate-tools.ps1 -Process ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/editor -Tools Ellipse,Line
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string[]] $Tools = @('Select', 'Rectangle', 'Ellipse', 'Line', 'Arrow', 'Freehand', 'Text', 'SpeechBalloon', 'Step',
        'Image', 'Clone', 'Emoji', 'Cursor', 'Highlight', 'SmartEraser', 'Blur', 'Pixelate', 'Magnify', 'Spotlight')
)
$ErrorActionPreference = 'Stop'
$order = @('Select', 'Rectangle', 'Ellipse', 'Line', 'Arrow', 'Freehand', 'Text', 'SpeechBalloon', 'Step',
    'Image', 'Clone', 'Emoji', 'Cursor', 'Highlight', 'SmartEraser', 'Blur', 'Pixelate', 'Magnify', 'Spotlight')
# Shapes drawn by dragging (the others only show the tool chosen).
$drawn = @('Rectangle', 'Ellipse', 'Line', 'Arrow', 'Freehand', 'SpeechBalloon', 'Highlight', 'Blur', 'Pixelate', 'Magnify', 'Spotlight')
foreach ($tool in $Tools) {
    $i = [array]::IndexOf($order, $tool)
    # The tool's button on the toolbar at the top centre of a 1024px monitor.
    $x = 150 + 44 * $i
    $key = $tool.ToLower()
    $steps = @("click:$x,25", 'move:600,600', "shot:$Out/$key-tool.png")
    if ($drawn -contains $tool) {
        $steps += @('down:200,200', 'to:450,400', "shot:$Out/$key-draw.png", 'up', 'move:600,600', "shot:$Out/$key-selected.png")
    } elseif ($tool -eq 'Step') {
        $steps += @('down:300,300', "shot:$Out/$key-draw.png", 'up', 'move:600,600', "shot:$Out/$key-selected.png")
    } elseif ($tool -eq 'Text') {
        $steps += @('click:200,200', 'type:Hello', 'move:600,600', "shot:$Out/$key-draw.png", 'key:Enter', "shot:$Out/$key-selected.png")
    }
    # A spotlight darkens the rest by its strength (30: 76/255).
    $dark = if ($tool -eq 'Spotlight') { 76 } else { -1 }
    & "$PSScriptRoot/annotate-session.ps1" -Process $Process -At 600,600 -Steps $steps -Dark $dark
    Write-Output ("$tool steps: " + (($steps | Where-Object { $_ -notlike 'shot:*' }) -join ';'))
}
