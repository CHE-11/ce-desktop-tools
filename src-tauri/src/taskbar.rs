//! Windows taskbar identity.
//!
//! The bundle id stays `com.ce.git` so this app still finds the settings
//! pointer that ce-git used. The taskbar id must be different, or pinning
//! this app replaces ce-git's taskbar icon.

pub fn set_process_app_id() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(
            windows::core::w!("com.ce.desktop-tools"),
        );
    }
}
