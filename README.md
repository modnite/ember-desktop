# Ember for the desktop

Ember on Windows, macOS and Linux: a small native shell (Tauri 2) around the Ember web app. The window shows the web app straight from your own Hearth server, so every web change reaches the desktop app with nothing to update here.

Windows needs Windows 10 or later (the shell uses Microsoft's WebView2). Older systems keep using the native Hearth client or a browser.

## What the shell adds

- **A tray icon.** Left click opens Ember. The menu has: Change server, keep running in the tray when closed, start when I sign in, Quit.
- **Closing the window keeps Ember running** (messages and calls still arrive), unless you switch that off in the tray menu.
- **System notifications** for new messages, nudges and calls, only when the window is not the one in use. Switch them off in Settings, Desktop alerts.
- **A flashing taskbar button** for messages, and a longer flash for nudges and calls.
- **The unread count** in the tray tooltip and the window title.
- **One copy at a time.** Starting Ember again brings the running one to the front.
- **Links open in your browser**, not inside Ember. The window can only show your own server.

## First run

Type your server's address (`ember.example.com`; a bare address tries https, then http). The shell checks that a Hearth server answers, remembers it and opens it. Change server does it again: in Settings (Server), on the sign-in page, or in the tray menu.

Use the address of the **Ember web app** (for example `ember.example.com`), not the Hearth web address (`call.example.com`). The shell opens that address as a page, so it has to be the one that serves Ember. Android only talks to the server's API, which is why either address works there.

Settings are in `%APPDATA%\org.hearth.ember.desktop\settings.json` (the matching folder on macOS and Linux).

## What the web app may ask the shell

The page from your server can only ask for four things: show a notification, flash the taskbar, set the unread count, and open the shell's page for choosing a server. Choosing the server itself is only possible from that shell page. See `src-tauri/capabilities`.

## Build

You need Node, Rust (stable, MSVC on Windows) and the Visual Studio C++ build tools. On Linux also `libwebkit2gtk-4.1-dev` and friends (see the Tauri prerequisites).

```
npm install
npx tauri dev        # run it
npx tauri build      # installer in src-tauri/target/release/bundle
```

Windows builds an NSIS installer that installs for the current user (no admin rights). `.github/workflows/build.yml` builds the Windows, macOS and Linux installers on GitHub; only the Windows one has been run and tried so far.

## Testing without clicking

Run the debug build with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, then drive the page over the Chrome DevTools protocol. A debug build prints each notification and flash request it receives.

## Calls and microphone

Calls use the web app's own WebRTC, so they work wherever the web view supports it. On Linux the web view (WebKitGTK) is the weak spot for calls.
