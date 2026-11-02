fn main() {
    // Each command the pages may call is its own permission, so the web app (loaded from the person's server) can be given only the few it needs.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&["set_server", "get_server", "return_to_server", "open_server_picker", "open_chat_window", "set_window_title", "close_window", "window_ready", "app_info", "check_update", "install_update", "allow_servers", "shake_chat", "close_chat_windows", "set_window_layout", "notify_desktop", "attention", "set_unread"]),
        ),
    )
    .expect("could not build the Tauri app");
}
