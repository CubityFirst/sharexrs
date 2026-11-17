//! `TaskHelpers.PlayNotificationSoundAsync`: ShareX's own sounds
//! (`ShareX/Resources/*.wav`), played without waiting for them.

/// `Resources.CaptureSound`, after a capture (`PlaySoundAfterCapture`).
pub fn play_capture() {
    play(include_bytes!("../assets/sounds/CaptureSound.wav"));
}

#[cfg(windows)]
fn play(wav: &'static [u8]) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    // SAFETY: the sound is static, so it outlives the asynchronous play.
    unsafe { PlaySoundW(wav.as_ptr().cast(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
}

/// Not ported outside Windows yet (snapr plays through `NSSound` on macOS).
#[cfg(not(windows))]
fn play(_wav: &'static [u8]) {}
