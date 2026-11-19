//! Step timing for the region capture's path, on with
//! `SHAREXRS_TRACE_CAPTURE=1`: [`begin`] starts a capture's clock and each
//! [`mark`] logs the time since it and since the previous mark
//! (`capture trace: +12.34 ms (+3.21) label`), through the log (stderr).
//! Off, a mark is one check of a cached flag.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SHAREXRS_TRACE_CAPTURE").is_some_and(|v| v != "0"))
}

/// The capture's start and the previous mark.
static CLOCK: Mutex<Option<(Instant, Instant)>> = Mutex::new(None);

/// Starts a capture's clock (`label` logged at +0).
pub fn begin(label: &str) {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    *CLOCK.lock().expect("trace clock") = Some((now, now));
    eprintln!("capture trace: +0.00 ms begin {label}");
}

/// Logs `label` with the time since [`begin`] and since the last mark;
/// nothing before a `begin` or once [`end`] ran.
pub fn mark(label: &str) {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    let mut clock = CLOCK.lock().expect("trace clock");
    if let Some((start, last)) = clock.as_mut() {
        let total = (now - *start).as_secs_f64() * 1000.0;
        let step = (now - *last).as_secs_f64() * 1000.0;
        *last = now;
        eprintln!("capture trace: +{total:.2} ms (+{step:.2}) {label}");
    }
}

/// Logs `label` and stops the clock.
pub fn end(label: &str) {
    mark(label);
    if enabled() {
        *CLOCK.lock().expect("trace clock") = None;
    }
}

/// Whether a capture's clock is running.
pub fn running() -> bool {
    enabled() && CLOCK.lock().expect("trace clock").is_some()
}

/// [`mark`]s `label` on a running clock, else [`begin`]s one with it.
pub fn mark_or_begin(label: &str) {
    if running() { mark(label) } else { begin(label) }
}
