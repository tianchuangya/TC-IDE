mod ai_core;
mod background_core;
mod dap_core;
mod editor_core;
mod editor_session;
mod extension_core;
mod graph_core;
mod index_core;
#[cfg(target_os = "linux")]
mod linux_app;
mod lsp_core;
#[cfg(target_os = "macos")]
mod macos_app;
mod motion_core;
mod package_motion;
mod search_core;
mod session_core;
mod syntax_core;
mod terminal_core;
mod text_buffer;
mod theme_core;
#[cfg(target_os = "windows")]
mod windows_app;
mod workspace_core;

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
