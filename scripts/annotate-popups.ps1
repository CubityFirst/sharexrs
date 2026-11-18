<#
.SYNOPSIS
  Reference shots of the annotation tool options' popups in region capture
  (ShareX or ShareXrs over the test pattern, through annotate-session.ps1):
  each dropdown opened from the options row, the shadow options popup
  (right click), with the pointer left on the button.
.EXAMPLE
  scripts/annotate-popups.ps1 -Process ShareX -Out G:/scripts/sharexrs-ref/shots/sharex/editor-popups -Suffix dark
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Suffix = 'dark',
    [string[]] $Only = @()
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
# name, tool index on the toolbar, verb, option x (the row is at y 70 on a
# 1024x768 monitor).
$shots = @(
    @('rect-bordercolor', 1, 'click', 355),
    @('rect-fillcolor', 1, 'click', 420),
    @('rect-thickness', 1, 'click', 485),
    @('rect-borderstyle', 1, 'click', 578),
    @('rect-cornerradius', 1, 'click', 670),
    @('rect-shadow', 1, 'rclick', 726),
    @('arrow-arrowstyle', 4, 'click', 550),
    @('text-textcolor', 6, 'click', 307),
    @('text-fontfamily', 6, 'click', 500),
    @('text-alignment', 6, 'click', 622),
    @('text-fontsize', 6, 'click', 687),
    @('step-steptype', 8, 'click', 640),
    @('step-startnumber', 8, 'click', 725),
    @('cursor-cursortype', 12, 'click', 535)
)
foreach ($s in $shots) {
    $name, $tool, $verb, $x = $s
    if ($Only.Count -gt 0 -and -not ($Only -contains $name)) { continue }
    $tx = 150 + 44 * $tool
    $steps = @("click:$tx,25", 'move:600,600', "${verb}:$x,70", 'wait:500', "shot:$Out/$name-$Suffix.png", 'key:Escape')
    & "$PSScriptRoot/annotate-session.ps1" -Process $Process -At 600,600 -Steps $steps
    Write-Output ("$name steps: " + (($steps | Where-Object { $_ -notlike 'shot:*' }) -join ';'))
}
