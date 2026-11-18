<#
.SYNOPSIS
  UI Automation helpers for scripted side-by-side sessions, dot-sourced:
  find a window of a process, invoke/select/toggle an element by name (the
  n-th match), set a spinner's value, a text box's text or a combo box's
  item. Works on ShareX (Avalonia) and ShareXrs (AccessKit) alike.

.EXAMPLE
  . scripts/uia-lib.ps1
  $w = Get-UiaWindow ShareX 'Image Combiner'
  Invoke-UiaNamed $w 'Add...'
  Set-UiaSpinner $w 0 3
  Select-UiaComboItem $w 0 'JPEG'
#>
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$script:AE = [System.Windows.Automation.AutomationElement]
$script:Scope = [System.Windows.Automation.TreeScope]
$script:CT = [System.Windows.Automation.ControlType]

function Get-UiaWindow([string] $Process, [string] $Title, [int] $TimeoutMs = 8000) {
    $deadline = [DateTime]::Now.AddMilliseconds($TimeoutMs)
    while ([DateTime]::Now -lt $deadline) {
        $ids = @(Get-Process -Name $Process -ErrorAction SilentlyContinue | ForEach-Object Id)
        $w = $script:AE::RootElement.FindAll($script:Scope::Children, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $ids -contains $_.Current.ProcessId -and $_.Current.Name -match $Title } | Select-Object -First 1
        if ($w) { return $w }
        Start-Sleep -Milliseconds 150
    }
    throw "no window of $Process matching '$Title'"
}

function Get-UiaAll($Window, $ControlType) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($script:AE::ControlTypeProperty, $ControlType)
    return @($Window.FindAll($script:Scope::Descendants, $cond))
}

function Get-UiaNamed($Window, [string] $Name, [int] $Index = 0) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($script:AE::NameProperty, $Name)
    $all = @($Window.FindAll($script:Scope::Descendants, $cond))
    if ($all.Count -le $Index) { throw "no element '$Name' #$Index" }
    return $all[$Index]
}

function Invoke-UiaElement($el) {
    $p = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$p)) { $p.Invoke(); return }
    if ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$p)) { $p.Select(); return }
    if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { $p.Toggle(); return }
    if ($el.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$p)) { $p.Expand(); return }
    throw "'$($el.Current.Name)' has no invoke/select/toggle/expand pattern"
}

function Invoke-UiaNamed($Window, [string] $Name, [int] $Index = 0) {
    Invoke-UiaElement (Get-UiaNamed $Window $Name $Index)
}

# A list item added to the selection (Ctrl+click).
function Add-UiaSelection($Window, [string] $Name) {
    $el = Get-UiaNamed $Window $Name
    $el.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).AddToSelection()
}

function Set-UiaSpinner($Window, [int] $Index, [double] $Value) {
    $s = (Get-UiaAll $Window $script:CT::Spinner)[$Index]
    $s.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).SetValue($Value)
}

# The n-th text box that isn't part of a spinner.
function Set-UiaText($Window, [int] $Index, [string] $Text) {
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $edits = Get-UiaAll $Window $script:CT::Edit | Where-Object {
        $parent = $walker.GetParent($_)
        -not ($parent -and $parent.Current.ControlType -eq $script:CT::Spinner)
    }
    @($edits)[$Index].GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Text)
}

function Select-UiaComboItem($Window, [int] $Index, [string] $Item) {
    $c = (Get-UiaAll $Window $script:CT::ComboBox)[$Index]
    $ec = $c.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
    $ec.Expand()
    Start-Sleep -Milliseconds 300
    $cond = New-Object System.Windows.Automation.PropertyCondition($script:AE::NameProperty, $Item)
    $it = $c.FindFirst($script:Scope::Descendants, $cond)
    if (-not $it) {
        # The dropdown is a popup of its own.
        $pid0 = $Window.Current.ProcessId
        foreach ($top in $script:AE::RootElement.FindAll($script:Scope::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
            if ($top.Current.ProcessId -ne $pid0) { continue }
            $it = $top.FindFirst($script:Scope::Descendants, $cond)
            if ($it) { break }
        }
    }
    if (-not $it) { $ec.Collapse(); throw "no combo item '$Item'" }
    $it.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
    Start-Sleep -Milliseconds 150
    try { $ec.Collapse() } catch {}
}
