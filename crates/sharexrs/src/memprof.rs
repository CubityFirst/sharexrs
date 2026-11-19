//! Memory profiling, behind the `memory-profile` feature (off in every
//! normal build). With `SHAREXRS_MEMPROF=<file>`, a thread appends one JSON
//! line a second: on Windows every heap of the process (`HeapSummary`:
//! allocated and committed bytes; the first is the process heap, which
//! Rust's allocator and the C runtime use, the rest are the drivers' and
//! system libraries'), and the wgpu allocator's report (what it reserved in
//! blocks and what's allocated in them, the largest allocations by label).
//! Swap chains aren't wgpu-allocator memory and don't show here.

use std::io::Write;

#[cfg(windows)]
fn heaps() -> String {
    use windows_sys::Win32::System::Memory::{GetProcessHeap, GetProcessHeaps, HEAP_SUMMARY, HeapSummary};
    let mut handles = vec![std::ptr::null_mut(); 256];
    // SAFETY: the buffer holds `handles.len()` handles.
    let n = unsafe { GetProcessHeaps(handles.len() as u32, handles.as_mut_ptr()) } as usize;
    handles.truncate(n.min(256));
    // SAFETY: no arguments.
    let process = unsafe { GetProcessHeap() };
    if let Some(i) = handles.iter().position(|&h| h == process) {
        handles.swap(0, i);
    }
    let list: Vec<String> = handles
        .iter()
        .filter_map(|&h| {
            let mut s = HEAP_SUMMARY { cb: size_of::<HEAP_SUMMARY>() as u32, cbAllocated: 0, cbCommitted: 0, cbReserved: 0, cbMaxReserve: 0 };
            // SAFETY: a heap of this process and a sized summary.
            (unsafe { HeapSummary(h, 0, &mut s) } != 0).then(|| format!("[{},{}]", s.cbAllocated, s.cbCommitted))
        })
        .collect();
    format!(",\"heaps\":[{}]", list.join(","))
}

/// Every busy block of every heap: a size histogram and the blocks of 64
/// KiB or more with their first 16 bytes (what they are: a font file, an
/// image...). Nothing is allocated while a heap is locked.
#[cfg(windows)]
fn walk() -> String {
    use windows_sys::Win32::System::Memory::{GetProcessHeaps, HeapLock, HeapUnlock, HeapWalk, PROCESS_HEAP_ENTRY};
    let mut handles = vec![std::ptr::null_mut(); 256];
    // SAFETY: the buffer holds `handles.len()` handles.
    let n = unsafe { GetProcessHeaps(handles.len() as u32, handles.as_mut_ptr()) } as usize;
    handles.truncate(n.min(256));
    let mut out = String::from(",\"walk\":[");
    for (i, &h) in handles.iter().enumerate() {
        const CLASSES: [usize; 7] = [256, 4096, 65536, 1 << 20, 4 << 20, 16 << 20, usize::MAX];
        let mut hist = [(0usize, 0usize); 7];
        let mut big: Vec<(usize, [u8; 16])> = Vec::with_capacity(20000);
        // SAFETY: a heap of this process, walked while locked; `big` has room
        // for every block recorded (nothing allocates during the walk).
        unsafe {
            if HeapLock(h) == 0 {
                continue;
            }
            let mut e: PROCESS_HEAP_ENTRY = Default::default();
            while HeapWalk(h, &mut e) != 0 {
                if e.wFlags & 0x0004 == 0 {
                    continue;
                }
                let size = e.cbData as usize;
                let c = CLASSES.iter().position(|&c| size < c).unwrap_or(6);
                hist[c].0 += 1;
                hist[c].1 += size;
                if size >= 65536 && big.len() < big.capacity() {
                    let mut head = [0u8; 16];
                    std::ptr::copy_nonoverlapping(e.lpData as *const u8, head.as_mut_ptr(), 16);
                    big.push((size, head));
                }
            }
            HeapUnlock(h);
        }
        big.sort_by_key(|b| std::cmp::Reverse(b.0));
        let hist: Vec<String> = hist.iter().map(|(n, b)| format!("[{n},{b}]")).collect();
        let big: Vec<String> = big
            .iter()
            .take(200)
            .map(|(size, head)| format!("[{size},\"{}\"]", head.iter().map(|b| format!("{b:02x}")).collect::<String>()))
            .collect();
        if i > 0 {
            out += ",";
        }
        out += &format!("{{\"hist\":[{}],\"big\":[{}]}}", hist.join(","), big.join(","));
    }
    out + "]"
}

#[cfg(not(windows))]
fn walk() -> String {
    String::new()
}

#[cfg(not(windows))]
fn heaps() -> String {
    String::new()
}

pub fn start() {
    let Some(path) = std::env::var_os("SHAREXRS_MEMPROF") else { return };
    let _ = std::thread::Builder::new().name("memprof".into()).spawn(move || {
        let start = std::time::Instant::now();
        let mut tick = 0u64;
        loop {
            let mut line = format!("{{\"t\":{:.1}", start.elapsed().as_secs_f64());
            line += &heaps();
            if tick % 10 == 9 && std::env::var_os("SHAREXRS_MEMPROF_WALK").is_some() {
                line += &walk();
            }
            tick += 1;
            if let Some(report) = sharex_ui::gpu::PROFILED_DEVICE.get().and_then(|d| d.generate_allocator_report()) {
                let blocks: Vec<String> = report.blocks.iter().map(|b| b.size.to_string()).collect();
                let mut allocs: Vec<_> = report.allocations.iter().collect();
                allocs.sort_by_key(|a| std::cmp::Reverse(a.size));
                let top: Vec<String> =
                    allocs.iter().take(25).map(|a| format!("[{:?},{}]", a.name, a.size)).collect();
                line += &format!(
                    ",\"gpu_allocated\":{},\"gpu_reserved\":{},\"gpu_blocks\":[{}],\"gpu_allocations\":{},\"gpu_top\":[{}]",
                    report.total_allocated_bytes,
                    report.total_reserved_bytes,
                    blocks.join(","),
                    report.allocations.len(),
                    top.join(",")
                );
            }
            line += "}\n";
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                let _ = f.write_all(line.as_bytes());
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}
