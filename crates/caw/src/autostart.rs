//! Launch at Login via SMAppService (macOS 13+).
//!
//! Only works when running from a .app bundle.

#[cfg(target_os = "macos")]
use objc2_service_management::SMAppService;

/// Check if the app is running from a .app bundle.
pub fn is_app_bundle() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS"))
        .unwrap_or(false)
}

/// Check if Launch at Login is enabled.
#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    if !is_app_bundle() {
        return false;
    }
    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };
    // SMAppServiceStatusEnabled = 1
    status.0 == 1
}

#[cfg(not(target_os = "macos"))]
pub fn is_enabled() -> bool {
    false
}

/// Enable or disable Launch at Login.
#[cfg(target_os = "macos")]
pub fn set_enabled(enabled: bool) {
    if !is_app_bundle() {
        tracing::warn!("Launch at Login requires running from a .app bundle");
        return;
    }
    let service = unsafe { SMAppService::mainAppService() };
    if enabled {
        if let Err(e) = unsafe { service.registerAndReturnError() } {
            tracing::error!("Failed to register login item: {:?}", e);
        }
    } else {
        if let Err(e) = unsafe { service.unregisterAndReturnError() } {
            tracing::error!("Failed to unregister login item: {:?}", e);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_enabled(_enabled: bool) {}
