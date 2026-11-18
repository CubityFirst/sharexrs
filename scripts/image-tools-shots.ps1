<#
.SYNOPSIS
  Opens an Images tool window in a running ShareX or ShareXrs (Tools menu),
  feeds it the synthetic inputs through its file dialogs, and captures each
  state the `--render` screens reproduce (docs/TOOLS.md, "The windows").

.EXAMPLE
  scripts/image-tools-shots.ps1 -Process ShareX -Tool combiner -Out G:\scripts\sharexrs-ref\shots\sharex
  scripts/image-tools-shots.ps1 -Process ShareX -Tool all -Suffix -light -Out ...

  Inputs: -In (default G:\Scripts\sharexrs-ref\tools-ui\in, made by
  scripts/image-tools-inputs.py); each window is closed after its shots.
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Tool,
    [Parameter(Mandatory)] [string] $Out,
    [string] $In = 'G:\Scripts\sharexrs-ref\tools-ui\in',
    [string] $Suffix = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
. "$PSScriptRoot/uia-lib.ps1"

function Open-Tool([string] $Entry, [string] $Title) {
    & "$PSScriptRoot/sharex-menu.ps1" -Process $Process -Menu Tools -Entry $Entry | Out-Null
    Start-Sleep -Milliseconds 900
    return Get-UiaWindow $Process $Title
}
function Shot([string] $Title, [string] $Name) {
    & "$PSScriptRoot/input.ps1" -MoveTo 1900, 1050
    Start-Sleep -Milliseconds 700
    & "$PSScriptRoot/winshot.ps1" -Process $Process -Title $Title -Out (Join-Path $Out "$Name$Suffix.png")
}
function Answer([string[]] $Files, [string] $Folder) {
    Start-Sleep -Milliseconds 500
    if ($Folder) { & "$PSScriptRoot/file-dialog.ps1" -Process $Process -Folder $Folder | Out-Null }
    else { & "$PSScriptRoot/file-dialog.ps1" -Process $Process -Files $Files | Out-Null }
    Start-Sleep -Milliseconds 1200
}
function Close-Tool([string] $Title) {
    & "$PSScriptRoot/close-window.ps1" -Process $Process -Title $Title | Out-Null
    Start-Sleep -Milliseconds 400
}
$three = "$In\photo.png", "$In\landscape.png", "$In\portrait.png"

$tools = if ($Tool -eq 'all') { 'combiner', 'splitter', 'thumbnailer', 'resizer', 'converter', 'watermark', 'comparer', 'icon' } else { @($Tool) }
foreach ($t in $tools) {
    switch ($t) {
        'combiner' {
            $title = 'Image Combiner'
            $w = Open-Tool 'Image combiner...' $title
            Shot $title 'combiner-empty'
            Invoke-UiaNamed $w 'Add...'; Answer $three
            Shot $title 'combiner-three'
            Invoke-UiaNamed $w "$In\landscape.png"
            Shot $title 'combiner-selected'
            Invoke-UiaNamed $w 'Horizontal'
            Shot $title 'combiner-horizontal'
            # Back to ShareX's defaults (the options are saved).
            Invoke-UiaNamed $w 'Vertical'
            Close-Tool $title
        }
        'splitter' {
            $title = 'Image Splitter'
            $w = Open-Tool 'Image splitter...' $title
            Shot $title 'splitter-empty'
            # The spinners first: the text box last set keeps its selection.
            Set-UiaSpinner $w 0 3
            Set-UiaSpinner $w 1 2
            Invoke-UiaNamed $w 'Browse...' 0; Answer @("$In\photo.png")
            Shot $title 'splitter-loaded'
            Close-Tool $title
        }
        'thumbnailer' {
            $title = 'Image Thumbnailer'
            $w = Open-Tool 'Image thumbnailer...' $title
            Shot $title 'thumbnailer-empty'
            Invoke-UiaNamed $w 'Add...'; Answer $three
            Shot $title 'thumbnailer-three'
            Close-Tool $title
        }
        'resizer' {
            $title = 'Image resizer'
            $w = Open-Tool 'Image resizer...' $title
            Shot $title 'resizer-empty'
            Invoke-UiaNamed $w 'Add...'; Answer $three
            Start-Sleep -Milliseconds 800
            Shot $title 'resizer-three'
            Select-UiaComboItem $w 1 'JPEG'
            Start-Sleep -Milliseconds 800
            Shot $title 'resizer-jpeg'
            Close-Tool $title
        }
        'converter' {
            $title = 'Image converter'
            $w = Open-Tool 'Image converter...' $title
            Shot $title 'converter-empty'
            Invoke-UiaNamed $w 'Add...'; Answer $three
            Start-Sleep -Milliseconds 800
            Shot $title 'converter-png'
            foreach ($f in 'JPEG', 'WebP', 'GIF', 'BMP') {
                Select-UiaComboItem $w 0 $f
                Start-Sleep -Milliseconds 1200
                Shot $title "converter-$($f.ToLower())"
            }
            Close-Tool $title
        }
        'watermark' {
            $title = 'Image watermark'
            $w = Open-Tool 'Image watermark...' $title
            Shot $title 'watermark-empty'
            Invoke-UiaNamed $w 'Add...'; Answer $three
            Start-Sleep -Milliseconds 800
            Shot $title 'watermark-text'
            Select-UiaComboItem $w 0 'Image'
            Invoke-UiaNamed $w 'Browse...' 0; Answer @("$In\stamp.png")
            Start-Sleep -Milliseconds 800
            Shot $title 'watermark-image'
            Close-Tool $title
        }
        'comparer' {
            $title = 'Image Comparer'
            $w = Open-Tool 'Image comparer...' $title
            Shot $title 'comparer-empty'
            Invoke-UiaNamed $w 'Choose...' 0; Answer @("$In\photo.png")
            Shot $title 'comparer-one'
            Invoke-UiaNamed $w 'Choose...' 1; Answer @("$In\photo-edited.png")
            Shot $title 'comparer-slider'
            Invoke-UiaNamed $w 'Diff View'
            Shot $title 'comparer-diff'
            Invoke-UiaNamed $w 'Slider'
            Invoke-UiaNamed $w 'Choose...' 1; Answer @("$In\photo.png")
            Shot $title 'comparer-identical'
            Close-Tool $title
        }
        'icon' {
            $title = 'Icon Converter'
            $w = Open-Tool 'Icon converter...' $title
            Shot $title 'icon-empty'
            Invoke-UiaNamed $w 'Browse...'; Answer @("$In\alpha.png")
            Shot $title 'icon-loaded'
            Close-Tool $title
        }
        default { throw "no tool $t" }
    }
}
