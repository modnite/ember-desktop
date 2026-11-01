//! Ember for the desktop: a small native shell around the Ember web app.
//!
//! The window shows the web app straight from the person's own Hearth server, so every web change reaches the desktop app with nothing to
//! update here. The shell adds what a web page cannot do: a tray icon, keeping running when the window is closed, native notifications,
//! taskbar flashing, one running copy at a time and starting with the computer.

use std::{
    fs,
    path::PathBuf,
    sync::{atomic::{AtomicBool, Ordering}, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Url, UserAttentionType, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

const MAIN: &str = "main";

/// What the shell remembers between runs.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Settings {
    /// The server's address as "https://host" (no path), or nothing before the first connection.
    server: Option<String>,
    /// Closing the window keeps Ember running in the tray, so messages and calls still arrive.
    close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { server: None, close_to_tray: true }
    }
}

struct Shared {
    settings: Mutex<Settings>,
    quitting: AtomicBool,
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    // A debug build can keep its settings elsewhere, so trying things out never touches an installed copy's settings.
    #[cfg(debug_assertions)]
    if let Ok(dir) = std::env::var("EMBER_DESKTOP_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("settings.json"));
    }
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    settings_path(app)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_settings(app: &AppHandle) {
    let Some(path) = settings_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let settings = app.state::<Shared>().settings.lock().unwrap().clone();
    if let Ok(text) = serde_json::to_string_pretty(&settings) {
        let _ = fs::write(path, text);
    }
}

/// "ember.example.com", "https://ember.example.com/" and "localhost:18300" all become an address (scheme, host and port only).
/// Without a scheme, https is tried first and plain http second (a server on the same network often has no certificate).
fn candidates(input: &str) -> Vec<String> {
    let text = input.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let with = if text.contains("://") { vec![text.to_string()] } else { vec![format!("https://{text}"), format!("http://{text}")] };
    with.into_iter()
        .filter_map(|t| Url::parse(&t).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
        .map(|u| u.origin().ascii_serialization())
        .collect()
}

/// What is at an address. Told apart so the message can say what is actually wrong.
#[derive(PartialEq, PartialOrd, Clone, Copy)]
enum Probe {
    /// Nothing answered (a wrong name, no connection, a network that cannot reach it).
    Unreachable,
    /// Something answered, but it is not a Hearth server.
    NotHearth,
    /// A Hearth server, but this address does not offer the Ember web app (it may be the Hearth web app instead).
    NoEmber,
    Ok,
}

fn probe(origin: &str) -> Probe {
    let Ok(client) = reqwest::blocking::Client::builder().timeout(Duration::from_secs(6)).build() else { return Probe::Unreachable };
    // Every Hearth server tells how people may register at /api/auth/config.
    let Ok(config) = client.get(format!("{origin}/api/auth/config")).send() else { return Probe::Unreachable };
    let hearth = config
        .status()
        .is_success()
        .then(|| config.text().ok())
        .flatten()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|v| v.get("registration").is_some())
        .unwrap_or(false);
    if !hearth {
        return Probe::NotHearth;
    }
    // The Ember web app says who it is in its manifest.
    let ember = client
        .get(format!("{origin}/manifest.webmanifest"))
        .send()
        .ok()
        .filter(|r| r.status().is_success())
        .and_then(|r| r.text().ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|v| v.get("short_name").and_then(|n| n.as_str()) == Some("Ember"))
        .unwrap_or(false);
    if ember { Probe::Ok } else { Probe::NoEmber }
}

/// Where the Ember web app usually lives next to a server's own address: call.example.com becomes ember.example.com, example.com becomes
/// ember.example.com. Nothing for an IP address or a name with no dot.
fn ember_sibling(origin: &str) -> Option<String> {
    let mut url = Url::parse(origin).ok()?;
    let host = url.host_str()?.to_string();
    if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
        return None;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let rest = if labels.len() >= 3 { labels[1..].join(".") } else { host.clone() };
    let sibling = format!("ember.{rest}");
    if sibling == host {
        return None;
    }
    url.set_host(Some(&sibling)).ok()?;
    Some(url.origin().ascii_serialization())
}

fn resolve_server(input: &str) -> Result<String, String> {
    let options = candidates(input);
    if options.is_empty() {
        return Err("Type the address of your server, for example ember.example.com.".into());
    }
    let mut worst = Probe::Unreachable;
    for origin in &options {
        let found = probe(origin);
        if found == Probe::Ok {
            return Ok(origin.clone());
        }
        if found > worst {
            worst = found;
        }
    }
    // The same address Android uses is often the Hearth server's own (call.example.com), where the Ember web app lives under a name of its
    // own. Try that name, and only take it if it really is the Ember web app.
    let mut tried = None;
    if worst >= Probe::NoEmber || worst == Probe::Unreachable {
        if let Some(sibling) = options.iter().find_map(|o| ember_sibling(o)) {
            if probe(&sibling) == Probe::Ok {
                return Ok(sibling);
            }
            tried = Some(sibling);
        }
    }
    let host = Url::parse(&options[0]).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| input.trim().to_string());
    let also = tried.and_then(|t| Url::parse(&t).ok()).and_then(|u| u.host_str().map(str::to_string)).map(|h| format!(" {h} did not answer as the Ember web app either.")).unwrap_or_default();
    Err(match worst {
        Probe::NoEmber => format!("{host} is a Hearth server, but it does not offer the Ember web app.{also} Ember usually has its own address, for example ember.yourdomain.com."),
        Probe::NotHearth => format!("{host} answered, but it is not a Hearth server."),
        _ => format!("Could not reach {host}.{also} Check the address and your connection."),
    })
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default()
}

/// The shell's own page, where the server address is typed. It is shown in the one window, in place of the web app.
fn local_page() -> Url {
    #[cfg(windows)]
    let text = "http://tauri.localhost/index.html";
    #[cfg(not(windows))]
    let text = "tauri://localhost/index.html";
    Url::parse(text).expect("the shell's own page has a valid address")
}

fn is_local(url: &Url) -> bool {
    url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost")
}

/// The one window. It shows the person's own server (or the shell's page for choosing one) and nothing else: any other address opens in
/// the default browser instead.
fn build_main(app: &AppHandle, server: Option<&str>, visible: bool) -> tauri::Result<WebviewWindow> {
    let start = match server {
        Some(s) => WebviewUrl::External(Url::parse(s).expect("a saved server address is a valid address")),
        None => WebviewUrl::App("index.html".into()),
    };
    let guard_app = app.clone();
    let popup_app = app.clone();
    WebviewWindowBuilder::new(app, MAIN, start)
        .title("Ember")
        .inner_size(1100.0, 760.0)
        .min_inner_size(380.0, 480.0)
        .visible(visible)
        .on_navigation(move |url| {
            // Whichever server is chosen now (it can be changed while the window is open).
            let current = guard_app.state::<Shared>().settings.lock().unwrap().server.clone().and_then(|s| Url::parse(&s).ok());
            if is_local(url) || current.map(|c| same_origin(url, &c)).unwrap_or(false) || matches!(url.scheme(), "about" | "blob" | "data") {
                return true;
            }
            let _ = guard_app.opener().open_url(url.as_str(), None::<&str>);
            false
        })
        .on_new_window(move |url, _features| {
            let _ = popup_app.opener().open_url(url.as_str(), None::<&str>);
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
}

fn saved_server(app: &AppHandle) -> Option<String> {
    app.state::<Shared>().settings.lock().unwrap().server.clone()
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    } else {
        let server = saved_server(app);
        let _ = build_main(app, server.as_deref(), true);
    }
}

/// Show the page for choosing a server in the main window, in place of the web app.
fn open_setup(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.navigate(local_page());
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    } else {
        let _ = build_main(app, None, true);
    }
}

// ---------------------------------------------------------------- what the pages can ask for

/// Connect to a server: check it answers, remember it and show it.
#[tauri::command]
async fn set_server(app: AppHandle, input: String) -> Result<String, String> {
    let origin = tauri::async_runtime::spawn_blocking(move || resolve_server(&input)).await.map_err(|e| e.to_string())??;
    app.state::<Shared>().settings.lock().unwrap().server = Some(origin.clone());
    save_settings(&app);
    let url = Url::parse(&origin).map_err(|e| e.to_string())?;
    if let Some(main) = app.get_webview_window(MAIN) {
        main.navigate(url).map_err(|e| e.to_string())?;
        let _ = main.show();
        let _ = main.set_focus();
    } else {
        build_main(&app, Some(&origin), true).map_err(|e| e.to_string())?;
    }
    Ok(origin)
}

/// Leave the page for choosing a server and go back to the one already chosen.
#[tauri::command]
fn return_to_server(app: AppHandle) {
    if let (Some(main), Some(server)) = (app.get_webview_window(MAIN), saved_server(&app)) {
        if let Ok(url) = Url::parse(&server) {
            let _ = main.navigate(url);
        }
    }
}

/// The web app's own "Change server" button: opens the page where the address is typed.
#[tauri::command]
// Async, because a window made from a plain command can stall on Windows while the command waits for it.
async fn open_server_picker(app: AppHandle) {
    open_setup(&app);
}

#[tauri::command]
fn get_server(app: AppHandle) -> Option<String> {
    app.state::<Shared>().settings.lock().unwrap().server.clone()
}

/// A banner from the system, only when the window is not what the person is looking at.
#[tauri::command]
fn notify_desktop(app: AppHandle, title: String, body: String) {
    #[cfg(debug_assertions)]
    eprintln!("notify_desktop: {title} / {body}");
    if let Some(w) = app.get_webview_window(MAIN) {
        if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
            return;
        }
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Flash the taskbar button (a nudge, a call) until the window is looked at.
#[tauri::command]
fn attention(app: AppHandle, critical: bool) {
    #[cfg(debug_assertions)]
    eprintln!("attention: critical={critical}");
    if let Some(w) = app.get_webview_window(MAIN) {
        if w.is_focused().unwrap_or(false) {
            return;
        }
        let kind = if critical { UserAttentionType::Critical } else { UserAttentionType::Informational };
        let _ = w.request_user_attention(Some(kind));
    }
}

/// How many chats have unread messages: shown on the tray icon and in the window's title.
#[tauri::command]
fn set_unread(app: AppHandle, count: u32) {
    let tip = if count == 0 { "Ember".to_string() } else { format!("Ember ({count} unread)") };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&tip));
    }
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.set_title(&if count == 0 { "Ember".to_string() } else { format!("({count}) Ember") });
    }
}

// ---------------------------------------------------------------- the tray

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let close_to_tray = app.state::<Shared>().settings.lock().unwrap().close_to_tray;
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let open = MenuItem::with_id(app, "open", "Open Ember", true, None::<&str>)?;
    let server = MenuItem::with_id(app, "server", "Change server…", true, None::<&str>)?;
    let tray_item = CheckMenuItem::with_id(app, "close_to_tray", "Keep running in the tray when closed", true, close_to_tray, None::<&str>)?;
    let start_item = CheckMenuItem::with_id(app, "autostart", "Start when I sign in", true, autostart_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ember", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &server, &PredefinedMenuItem::separator(app)?, &tray_item, &start_item, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Ember")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "server" => open_setup(app),
            "close_to_tray" => {
                app.state::<Shared>().settings.lock().unwrap().close_to_tray = tray_item.is_checked().unwrap_or(true);
                save_settings(app);
            }
            "autostart" => {
                let on = start_item.is_checked().unwrap_or(false);
                let _ = if on { app.autolaunch().enable() } else { app.autolaunch().disable() };
            }
            "quit" => {
                app.state::<Shared>().quitting.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // A second copy only brings the first one to the front. (Not in a debug build, so one can be tried beside an installed copy.)
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)));
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .invoke_handler(tauri::generate_handler![set_server, get_server, return_to_server, open_server_picker, notify_desktop, attention, set_unread])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<Shared>();
                let keep = state.settings.lock().unwrap().close_to_tray;
                if window.label() == MAIN && keep && !state.quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(Shared { settings: Mutex::new(load_settings(&handle)), quitting: AtomicBool::new(false) });
            build_tray(&handle)?;
            let minimized = std::env::args().any(|a| a == "--minimized");
            let server = saved_server(&handle);
            // Starting minimized only hides the window when there is a server to show; the first run needs the page for choosing one.
            build_main(&handle, server.as_deref(), !minimized || server.is_none())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ember could not start");
}
