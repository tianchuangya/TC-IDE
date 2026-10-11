mod editor_core;
#[cfg(target_os = "linux")]
mod linux_app;
#[cfg(target_os = "macos")]
mod macos_app;
#[cfg(target_os = "windows")]
mod windows_app;

#[cfg(target_os = "linux")]
fn main() {
    linux_app::run();
}

#[cfg(target_os = "windows")]
fn main() {
    windows_app::run();
}

#[cfg(target_os = "macos")]
fn main() {
    macos_app::run();
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
compile_error!("TC-IDE currently supports Linux, Windows, and macOS targets only");
