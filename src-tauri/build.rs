fn main() {
    // Each command the pages may call is its own permission, so the web app (loaded from the person's server) can be given only the few it needs.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&["set_server", "get_server", "return_to_server", "open_server_picker", "notify_desktop", "attention", "set_unread"]),
        ),
    )
    .expect("could not build the Tauri app");
}
