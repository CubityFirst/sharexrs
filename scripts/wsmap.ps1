<#
.SYNOPSIS
A process's working set by owner (QueryWorkingSet): each loaded module's
resident pages (and how many of them are shared), and the rest by kind
(private, mapped). Reads only (PROCESS_QUERY_INFORMATION | PROCESS_VM_READ);
nothing is sent to the process.

.EXAMPLE
powershell -File scripts/wsmap.ps1 -Id 1234 -Top 20
#>
param(
    [Parameter(Mandatory = $true)][int]$Id,
    [int]$Top = 20
)

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class WsMap {
    [StructLayout(LayoutKind.Sequential)]
    public struct MBI {
        public IntPtr BaseAddress; public IntPtr AllocationBase; public uint AllocationProtect;
        public ushort PartitionId; public IntPtr RegionSize; public uint State; public uint Protect; public uint Type;
    }
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern IntPtr VirtualQueryEx(IntPtr h, IntPtr addr, out MBI info, IntPtr len);
    [DllImport("psapi.dll")] static extern bool QueryWorkingSet(IntPtr h, IntPtr buf, int size);
    [DllImport("psapi.dll", CharSet = CharSet.Unicode)] static extern uint GetMappedFileNameW(IntPtr h, IntPtr addr, System.Text.StringBuilder name, uint size);
    public class Row { public string Owner; public long Pages; public long Shared; }
    // owner of an allocation base: a module name (from the caller) or the kind
    public static List<Row> Walk(int pid, Dictionary<long, string> modules) {
        IntPtr h = OpenProcess(0x0400 | 0x0010, false, pid);
        if (h == IntPtr.Zero) throw new Exception("OpenProcess failed");
        var rows = new Dictionary<string, Row>();
        try {
            int n = 1 << 20;
            IntPtr buf = Marshal.AllocHGlobal(8 * (n + 1));
            try {
                if (!QueryWorkingSet(h, buf, 8 * (n + 1))) throw new Exception("QueryWorkingSet failed");
                long count = Marshal.ReadInt64(buf);
                var cache = new Dictionary<long, string>();
                for (long i = 0; i < count; i++) {
                    ulong e = (ulong)Marshal.ReadInt64(buf, (int)(8 + i * 8));
                    long va = (long)(e & ~0xFFFUL);
                    bool shared = ((e >> 8) & 1) == 1;
                    MBI m;
                    string owner = "?";
                    if (VirtualQueryEx(h, new IntPtr(va), out m, new IntPtr(Marshal.SizeOf(typeof(MBI)))) != IntPtr.Zero) {
                        long b = m.AllocationBase.ToInt64();
                        if (!cache.TryGetValue(b, out owner)) {
                            if (!modules.TryGetValue(b, out owner)) {
                                owner = m.Type == 0x20000 ? (m.Protect == 0x404 ? "(private, write-combined)" : "(private)") : m.Type == 0x40000 ? "(mapped)" : "(image, other)";
                                if (m.Type == 0x40000) {
                                    var sb = new System.Text.StringBuilder(1024);
                                    if (GetMappedFileNameW(h, m.AllocationBase, sb, 1024) > 0) {
                                        string f = sb.ToString();
                                        owner = "(mapped) " + f.Substring(f.LastIndexOf('\\') + 1);
                                    }
                                }
                            }
                            cache[b] = owner;
                        }
                    }
                    Row r;
                    if (!rows.TryGetValue(owner, out r)) { r = new Row { Owner = owner }; rows[owner] = r; }
                    r.Pages++;
                    if (shared) r.Shared++;
                }
            } finally { Marshal.FreeHGlobal(buf); }
        } finally { CloseHandle(h); }
        return new List<Row>(rows.Values);
    }
}
"@

$mods = New-Object 'System.Collections.Generic.Dictionary[long,string]'
foreach ($m in (Get-Process -Id $Id).Modules) { $mods[$m.BaseAddress.ToInt64()] = $m.ModuleName }
$rows = [WsMap]::Walk($Id, $mods)
$total = ($rows | Measure-Object Pages -Sum).Sum
"Working set: {0} MB" -f [math]::Round($total * 4KB / 1MB, 1)
$rows | Sort-Object Pages -Descending | Select-Object -First $Top | ForEach-Object {
    [pscustomobject]@{ Owner = $_.Owner; MB = [math]::Round($_.Pages * 4KB / 1MB, 1); SharedMB = [math]::Round($_.Shared * 4KB / 1MB, 1) }
} | Format-Table -AutoSize | Out-String -Width 200
