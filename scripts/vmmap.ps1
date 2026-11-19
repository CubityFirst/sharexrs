<#
.SYNOPSIS
A process's committed memory by kind (VirtualQueryEx over its whole address
space): private, mapped and image, in MB, and the largest private
allocations (by allocation base) with their committed size and protection.
Reads only (PROCESS_QUERY_INFORMATION); nothing is sent to the process.

.EXAMPLE
powershell -File scripts/vmmap.ps1 -Id 1234 -Top 30
#>
param(
    [Parameter(Mandatory = $true)][int]$Id,
    [int]$Top = 25
)

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class VmMap {
    [StructLayout(LayoutKind.Sequential)]
    public struct MBI {
        public IntPtr BaseAddress; public IntPtr AllocationBase; public uint AllocationProtect;
        public ushort PartitionId; public IntPtr RegionSize; public uint State; public uint Protect; public uint Type;
    }
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern IntPtr VirtualQueryEx(IntPtr h, IntPtr addr, out MBI info, IntPtr len);
    public class Alloc { public long Base; public long Committed; public uint Type; public uint Protect; public int Regions; }
    public static List<Alloc> Walk(int pid) {
        var result = new Dictionary<long, Alloc>();
        IntPtr h = OpenProcess(0x0400 | 0x0010, false, pid);
        if (h == IntPtr.Zero) throw new Exception("OpenProcess failed");
        try {
            long addr = 0;
            MBI m;
            while (VirtualQueryEx(h, new IntPtr(addr), out m, new IntPtr(Marshal.SizeOf(typeof(MBI)))) != IntPtr.Zero) {
                long size = m.RegionSize.ToInt64();
                if (m.State == 0x1000) {
                    long b = m.AllocationBase.ToInt64();
                    Alloc a;
                    if (!result.TryGetValue(b, out a)) { a = new Alloc { Base = b, Type = m.Type, Protect = m.Protect }; result[b] = a; }
                    a.Committed += size; a.Regions++;
                }
                long next = m.BaseAddress.ToInt64() + size;
                if (next <= addr) break;
                addr = next;
            }
        } finally { CloseHandle(h); }
        return new List<Alloc>(result.Values);
    }
}
"@

$allocs = [VmMap]::Walk($Id)
$kinds = @{ 0x20000 = 'private'; 0x40000 = 'mapped'; 0x1000000 = 'image' }
$allocs | Group-Object { $kinds[[int]$_.Type] } | ForEach-Object {
    [pscustomobject]@{ Kind = $_.Name; CommittedMB = [math]::Round(($_.Group | Measure-Object Committed -Sum).Sum / 1MB, 1); Allocations = $_.Count }
} | Format-Table -AutoSize | Out-String -Width 200
"Largest private allocations:"
$allocs | Where-Object { $_.Type -eq 0x20000 } | Sort-Object Committed -Descending | Select-Object -First $Top | ForEach-Object {
    [pscustomobject]@{ Base = '0x{0:X}' -f $_.Base; CommittedMB = [math]::Round($_.Committed / 1MB, 2); Protect = '0x{0:X}' -f $_.Protect; Regions = $_.Regions }
} | Format-Table -AutoSize | Out-String -Width 200
"Private allocations by committed size bucket:"
$allocs | Where-Object { $_.Type -eq 0x20000 } | Group-Object { if ($_.Committed -ge 16MB) { '>=16MB' } elseif ($_.Committed -ge 1MB) { '1-16MB' } elseif ($_.Committed -ge 64KB) { '64KB-1MB' } else { '<64KB' } } | ForEach-Object {
    [pscustomobject]@{ Bucket = $_.Name; Count = $_.Count; MB = [math]::Round(($_.Group | Measure-Object Committed -Sum).Sum / 1MB, 1) }
} | Format-Table -AutoSize | Out-String -Width 200
