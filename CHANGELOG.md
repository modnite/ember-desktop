# Changelog

Version numbers follow the other Ember apps: the last number for small fixes, the middle number for features and major fixes.

## 0.4.2

- Fixed: the other person could not hear you in a call. The window asked for permission to use the microphone, the question never showed, and the call stayed silent. On Windows the microphone and camera now work in calls without asking, for your own server's pages only.

## 0.4.1

- A nudge shakes the window the chat is in, sent or received. It is the real window that shakes, and the system's reduced motion setting does not switch it off.
- Switching Chats in their own windows off closes the chat windows and brings their chats into the one window (the one in use is shown). Switching it on pops the chat you were in out into a window of its own.

## 0.4.0

- The window is thin. The sign-in page and the chat list (when chats have windows of their own) open at phone width, and the empty "Pick a chat" pane is gone in that mode. Switching Chats in their own windows off makes the window wide again.
- Windows open cleanly. They stay hidden until their page has drawn itself, so there is no blank flash, and each new chat window opens a little down and to the right of the last one.

## 0.3.0

- Each chat opens in a window of its own, by default, so every chat has its own taskbar button. Settings, Chats in their own windows, switches it off for the one-window look. Right click a chat for Open in its own window. A chat that already has a window comes to the front instead of opening another.
- A chat window shows only its chat and is named after it. The main window keeps the chat list, the sounds, the notices and the unread count, so nothing is announced twice. Notices stay quiet while any Ember window is the one in use.

## 0.2.2

- Change server no longer opens a second window. The page for choosing a server replaces the web app inside the one window, with the current server filled in and a Cancel link to go back. The first run uses the same page.

## 0.2.1

- The same server address as Android works: type `call.example.com` and the app finds the Ember web app next to it (`ember.example.com`) and uses it, but only when that address really is the Ember web app. If neither answers as Ember, the message says what was tried.
- A debug build can run beside an installed copy.

## 0.2.0

- The app icon is the speech bubble and flame on their own, with no tile or box around it. The tray icon is the same.
- A visible way to change the server: a Server row in Settings and a Change server link on the sign-in page, as well as the tray menu.
- Clearer messages when a server address does not work: it says whether nothing answered, whether something answered that is not a Hearth server, or whether it is a Hearth server without the Ember web app (the Hearth web address is not the Ember address).
- Fixed: changing the server while signed in left no main window.
- Debug builds can keep their settings apart from an installed copy (EMBER_DESKTOP_CONFIG_DIR).

## 0.1.0

- First version: a window with the Ember web app from your own server, a server picker on first run, a tray icon, closing to the tray, system notifications and a flashing taskbar for messages, nudges and calls, the unread count, one copy at a time, starting with the computer, and links that open in your browser.
- Built and tried on Windows 10 and later. macOS and Linux are set up to build but not yet tried.
