#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Fix WebKit EGL crash on Intel GPUs
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    if std::env::var("GDK_BACKEND").is_err() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    sonos_controller_lib::run();
}
