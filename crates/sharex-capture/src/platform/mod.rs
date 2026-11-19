//! What each OS does differently. Windows goes through Win32 as ShareX
//! does; macOS and Linux grab the screen through xcap as snapr does, and
//! macOS lists windows through CoreGraphics (snapr's `macos.rs`).

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
#[cfg(windows)]
pub mod windows_hdr;

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::*;

#[cfg(target_os = "macos")]
pub mod macos;
