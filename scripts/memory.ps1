<#
.SYNOPSIS
Prints a process's memory, as docs/PORTING.md's memory tables give it:
working set / private bytes / peak working set (Get-Process) and the GPU
memory it holds (the `GPU Process Memory` counters, dedicated and shared),
in MB. Reads only; nothing is sent to the process.

.EXAMPLE
powershell -File scripts/memory.ps1 -Id 1234 -Label idle
powershell -File scripts/memory.ps1 -Id 1234 -Samples 30 -Interval 1   # one line a second
#>
param(
    [Parameter(Mandatory = $true)][int]$Id,
    [string]$Label = "",
    [int]$Samples = 1,
    [double]$Interval = 1
)

function Get-Mb([double]$bytes) { [math]::Round($bytes / 1MB, 1) }

for ($i = 0; $i -lt $Samples; $i++) {
    $p = Get-Process -Id $Id -ErrorAction Stop
    $dedicated = 0.0
    $shared = 0.0
    try {
        $set = Get-Counter -Counter "\GPU Process Memory(pid_$($Id)_*)\Dedicated Usage", "\GPU Process Memory(pid_$($Id)_*)\Shared Usage" -ErrorAction Stop
        foreach ($s in $set.CounterSamples) {
            if ($s.Path -like '*dedicated usage') { $dedicated += $s.CookedValue } else { $shared += $s.CookedValue }
        }
    } catch {}
    [pscustomobject]@{
        Label      = $Label
        Time       = (Get-Date).ToString("HH:mm:ss")
        WorkingSet = Get-Mb $p.WorkingSet64
        Private    = Get-Mb $p.PrivateMemorySize64
        PeakWS     = Get-Mb $p.PeakWorkingSet64
        GpuDedicated = Get-Mb $dedicated
        GpuShared  = Get-Mb $shared
        Handles    = $p.HandleCount
        Threads    = $p.Threads.Count
    } | ConvertTo-Json -Compress
    if ($i -lt $Samples - 1) { Start-Sleep -Milliseconds ([int]($Interval * 1000)) }
}
